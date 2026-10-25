use crate::{
    domain::{AssemblyStep, CustomFieldDefinition, ProjectBomItem, ProjectCustomValue},
    indexing, project_docs,
    state::AppState,
};
use chrono::Utc;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

type CommandResult<T> = Result<T, String>;

const PACKAGE_FORMAT: &str = "volum-project";
const PACKAGE_FORMAT_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 1_204;
const MAX_PROJECT_FILES: usize = 1_000;
const MAX_PROJECT_PHOTOS: usize = 200;
const SUPPORTED_FILES: &[&str] = &["stl", "3mf", "obj", "step", "stp"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPackageManifest {
    format: String,
    format_version: u32,
    app_version: String,
    package_export_id: String,
    exported_at: String,
    project: PackageProject,
    files: Vec<PackageAsset>,
    photos: Vec<PackagePhoto>,
    custom_fields: Vec<CustomFieldDefinition>,
    custom_values: Vec<ProjectCustomValue>,
    bom_items: Vec<ProjectBomItem>,
    assembly_steps: Vec<AssemblyStep>,
    tags: Vec<PackageTag>,
    #[serde(default)]
    web_sources: Vec<PackageWebSource>,
    estimate: Option<PackageEstimate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageProject {
    id: String,
    name: String,
    notes: String,
    favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageAsset {
    id: String,
    archive_path: String,
    filename: String,
    extension: String,
    role: String,
    primary: bool,
    byte_size: u64,
    blake3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagePhoto {
    id: String,
    archive_path: String,
    original_filename: String,
    mime_type: String,
    caption: String,
    sort_order: f64,
    is_cover: bool,
    byte_size: u64,
    blake3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageTag {
    name: String,
    color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageWebSource {
    provider: String,
    remote_id: Option<String>,
    canonical_url: String,
    title: String,
    creator: Option<String>,
    description: Option<String>,
    license: Option<String>,
    image_url: Option<String>,
    filament_grams: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageMaterial {
    name: String,
    material_type: String,
    color_name: Option<String>,
    color_hex: Option<String>,
    spool_price_minor: i64,
    currency: String,
    spool_weight_g: f64,
    density: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageEstimate {
    asset_id: Option<String>,
    plastic_g: f64,
    quantity: i64,
    source: String,
    material: Option<PackageMaterial>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPackagePreview {
    format_version: u32,
    app_version: String,
    package_project_id: String,
    package_export_id: String,
    project_name: String,
    exported_at: String,
    file_count: usize,
    photo_count: usize,
    bom_item_count: usize,
    assembly_step_count: usize,
    total_bytes: u64,
    already_imported: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPackageImportInput {
    path: String,
    root_id: String,
    relative_folder: String,
}

#[derive(Debug)]
struct ArchiveSource {
    path: PathBuf,
    archive_path: String,
}

#[tauri::command]
pub async fn inspect_project_package(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectPackagePreview> {
    let path = PathBuf::from(path);
    let manifest = tokio::task::spawn_blocking(move || validate_package(&path))
        .await
        .map_err(|error| error.to_string())??;
    preview_for_manifest(&manifest, &state).await
}

#[tauri::command]
pub async fn export_project_package(
    model_id: String,
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    let (manifest, sources) = build_package(&model_id, &state).await?;
    let destination = PathBuf::from(path);
    tokio::task::spawn_blocking(move || write_package(&destination, &manifest, &sources))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn import_project_package(
    input: ProjectPackageImportInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<String> {
    let _guard = state.file_operation_lock.lock().await;
    let package_path = PathBuf::from(&input.path);
    let package_for_validation = package_path.clone();
    let manifest = tokio::task::spawn_blocking(move || validate_package(&package_for_validation))
        .await
        .map_err(|error| error.to_string())??;
    let root_row = sqlx::query(
        "SELECT path, root_kind FROM library_roots WHERE id = ? AND status != 'removed'",
    )
    .bind(&input.root_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| "Destination library not found".to_string())?;
    let root_path = PathBuf::from(root_row.get::<String, _>("path"));
    let canonical_root = root_path
        .canonicalize()
        .map_err(|_| "The destination library is unavailable".to_string())?;
    let root_kind: String = root_row.get("root_kind");
    if root_kind == "managed" {
        let project_id =
            import_managed_project_package(&package_path, &manifest, &canonical_root, &state)
                .await?;
        let _ = app.emit("models-changed", &input.root_id);
        return Ok(project_id);
    }
    let requested_folder = if input.relative_folder.trim().is_empty() {
        let name = safe_name(&manifest.project.name);
        format!(
            "Volum Imports/{}",
            if name.is_empty() {
                "Imported project"
            } else {
                &name
            }
        )
    } else {
        normalize_relative_folder(&input.relative_folder)?
    };
    let (target_folder, target_path) = unique_destination(&canonical_root, &requested_folder)?;
    let photo_staging = state
        .data_dir
        .join("project-package-staging")
        .join(Uuid::new_v4().to_string());
    create_destination_without_symlinks(&canonical_root, &target_folder)?;
    if let Err(error) = std::fs::create_dir_all(&photo_staging) {
        let _ = std::fs::remove_dir(&target_path);
        return Err(error.to_string());
    }
    let extraction_manifest = manifest.clone();
    let extraction_package = package_path.clone();
    let extraction_target = target_path.clone();
    let extraction_photos = photo_staging.clone();
    let extraction_folder = target_folder.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_package_payloads(
            &extraction_package,
            &extraction_manifest,
            &extraction_target,
            &extraction_photos,
            &extraction_folder,
        )
    })
    .await
    .map_err(|error| error.to_string())?;
    let (relative_paths, photo_paths) = match extraction {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&target_path);
            let _ = std::fs::remove_dir_all(&photo_staging);
            return Err(error);
        }
    };
    let asset_ids = match indexing::index_imported_files(
        &input.root_id,
        &canonical_root,
        &relative_paths,
        &state.pool,
    )
    .await
    {
        Ok(ids) => ids,
        Err(error) => {
            let partial_ids = imported_asset_ids(&input.root_id, &relative_paths, &state.pool)
                .await
                .unwrap_or_default();
            let _ = cleanup_import(&partial_ids, None, &target_path, &state).await;
            let _ = std::fs::remove_dir_all(&photo_staging);
            return Err(error);
        }
    };
    let project_id = match create_imported_project(
        &manifest,
        &asset_ids,
        &input.root_id,
        &target_folder,
        &state,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => {
            let _ = cleanup_import(&asset_ids, None, &target_path, &state).await;
            let _ = std::fs::remove_dir_all(&photo_staging);
            return Err(error);
        }
    };
    let result =
        apply_imported_documentation(&manifest, &project_id, &asset_ids, &photo_paths, &state)
            .await;
    let _ = std::fs::remove_dir_all(&photo_staging);
    if let Err(error) = result {
        let _ = cleanup_import(&asset_ids, Some(&project_id), &target_path, &state).await;
        return Err(error);
    }
    indexing::refresh_search(&project_id, &state.pool).await?;
    let _ = app.emit("models-changed", &input.root_id);
    Ok(project_id)
}

async fn import_managed_project_package(
    package_path: &Path,
    manifest: &ProjectPackageManifest,
    root_path: &Path,
    state: &AppState,
) -> CommandResult<String> {
    let job_id = Uuid::new_v4().to_string();
    let project_id = Uuid::new_v4().to_string();
    let staging = root_path.join(".staging").join(format!("package-{job_id}"));
    let timestamp = now();
    sqlx::query("INSERT INTO file_operation_jobs (id, operation, state, payload_json, created_at, updated_at) VALUES (?, 'import-package', 'staging', ?, ?, ?)")
        .bind(&job_id)
        .bind(
            serde_json::json!({
                "projectId": project_id,
                "staging": staging.to_string_lossy(),
            })
            .to_string(),
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let model_staging = staging.join("Models");
    let photo_staging = staging.join("Photos");
    if let Err(error) = std::fs::create_dir_all(&model_staging)
        .and_then(|_| std::fs::create_dir_all(&photo_staging))
    {
        fail_package_job(&job_id, &error.to_string(), &state.pool).await;
        return Err(format!("Could not prepare the package import: {error}"));
    }
    let extraction_manifest = manifest.clone();
    let extraction_package = package_path.to_path_buf();
    let extraction_models = model_staging.clone();
    let extraction_photos = photo_staging.clone();
    let extraction = tokio::task::spawn_blocking(move || {
        extract_package_payloads(
            &extraction_package,
            &extraction_manifest,
            &extraction_models,
            &extraction_photos,
            "Models",
        )
    })
    .await
    .map_err(|error| error.to_string())?;
    let (staged_relative_paths, photo_paths) = match extraction {
        Ok(extraction) => extraction,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            fail_package_job(&job_id, &error, &state.pool).await;
            return Err(error);
        }
    };
    let staged_paths = staged_relative_paths
        .iter()
        .map(|relative| staging.join(relative).to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let input = crate::project_storage::CreateManagedProjectInput {
        name: manifest.project.name.clone(),
        file_paths: staged_paths,
        asset_ids: Vec::new(),
        transfer_mode: "move".into(),
    };
    let (project_id, _) = match crate::project_storage::create_managed_project_internal_with_id(
        &input, project_id, state,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            fail_package_job(&job_id, &error, &state.pool).await;
            return Err(error);
        }
    };
    if let Err(error) = sqlx::query(
        "UPDATE file_operation_jobs SET state = 'committing', updated_at = ? WHERE id = ?",
    )
    .bind(now())
    .bind(&job_id)
    .execute(&state.pool)
    .await
    .map_err(db_error)
    {
        defer_package_recovery(&job_id, &error, &state.pool).await;
        return Err(error);
    }
    let project = match sqlx::query(
        "SELECT f.relative_path FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?",
    )
    .bind(&project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)
    {
        Ok(project) => project,
        Err(error) => {
            defer_package_recovery(&job_id, &error, &state.pool).await;
            return Err(error);
        }
    };
    let target_path = root_path.join(project.get::<String, _>("relative_path"));
    let asset_ids: Vec<String> = match sqlx::query_scalar(
        "SELECT asset_id FROM model_assets WHERE model_id = ? ORDER BY sort_order",
    )
    .bind(&project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)
    {
        Ok(asset_ids) => asset_ids,
        Err(error) => {
            defer_package_recovery(&job_id, &error, &state.pool).await;
            return Err(error);
        }
    };
    if let Err(error) =
        apply_imported_documentation(manifest, &project_id, &asset_ids, &photo_paths, state).await
    {
        let cleanup = cleanup_import(&asset_ids, Some(&project_id), &target_path, state).await;
        let _ = std::fs::remove_dir_all(&staging);
        return match cleanup {
            Ok(()) => {
                fail_package_job(&job_id, &error, &state.pool).await;
                Err(error)
            }
            Err(cleanup_error) => {
                defer_package_recovery(&job_id, &cleanup_error, &state.pool).await;
                Err(format!(
                    "{error}. The incomplete managed import also needs cleanup: {cleanup_error}"
                ))
            }
        };
    }
    if let Err(error) = indexing::refresh_search(&project_id, &state.pool).await {
        let cleanup = cleanup_import(&asset_ids, Some(&project_id), &target_path, state).await;
        let _ = std::fs::remove_dir_all(&staging);
        if let Err(cleanup_error) = cleanup {
            defer_package_recovery(&job_id, &cleanup_error, &state.pool).await;
        } else {
            fail_package_job(&job_id, &error, &state.pool).await;
        }
        return Err(error);
    }
    if let Err(error) = crate::project_storage::write_project_marker(&project_id, &state.pool).await
    {
        let cleanup = cleanup_import(&asset_ids, Some(&project_id), &target_path, state).await;
        let _ = std::fs::remove_dir_all(&staging);
        if let Err(cleanup_error) = cleanup {
            defer_package_recovery(&job_id, &cleanup_error, &state.pool).await;
        } else {
            fail_package_job(&job_id, &error, &state.pool).await;
        }
        return Err(error);
    }
    sqlx::query("UPDATE file_operation_jobs SET state = 'complete', error = NULL, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&job_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(project_id)
}

async fn fail_package_job(id: &str, error: &str, pool: &sqlx::SqlitePool) {
    let _ = sqlx::query(
        "UPDATE file_operation_jobs SET state = 'failed', error = ?, updated_at = ? WHERE id = ?",
    )
    .bind(error)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await;
}

async fn defer_package_recovery(id: &str, error: &str, pool: &sqlx::SqlitePool) {
    let _ = sqlx::query("UPDATE file_operation_jobs SET error = ?, updated_at = ? WHERE id = ?")
        .bind(error)
        .bind(now())
        .bind(id)
        .execute(pool)
        .await;
}

async fn imported_asset_ids(
    root_id: &str,
    relative_paths: &[String],
    pool: &sqlx::SqlitePool,
) -> CommandResult<Vec<String>> {
    let mut ids = Vec::new();
    for relative_path in relative_paths {
        if let Some(id) =
            sqlx::query_scalar("SELECT id FROM assets WHERE root_id = ? AND relative_path = ?")
                .bind(root_id)
                .bind(relative_path)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

async fn cleanup_import(
    asset_ids: &[String],
    project_id: Option<&str>,
    target_path: &Path,
    state: &AppState,
) -> CommandResult<()> {
    let model_ids: Vec<String> = if asset_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar("SELECT DISTINCT model_id FROM model_assets WHERE asset_id IN (SELECT value FROM json_each(?))")
            .bind(serde_json::to_string(asset_ids).map_err(|error| error.to_string())?)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
    };
    let folder_ids: Vec<String> = if asset_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar(
            "SELECT DISTINCT folder_id FROM assets WHERE id IN (SELECT value FROM json_each(?))",
        )
        .bind(serde_json::to_string(asset_ids).map_err(|error| error.to_string())?)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?
    };
    let photo_paths = if let Some(project_id) = project_id {
        sqlx::query("SELECT media_path, thumbnail_path FROM project_photos WHERE model_id = ?")
            .bind(project_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
    } else {
        Vec::new()
    };
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    if let Some(project_id) = project_id {
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    if !asset_ids.is_empty() {
        sqlx::query("DELETE FROM assets WHERE id IN (SELECT value FROM json_each(?))")
            .bind(serde_json::to_string(asset_ids).map_err(|error| error.to_string())?)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        for model_id in model_ids {
            sqlx::query("DELETE FROM models WHERE id = ? AND NOT EXISTS (SELECT 1 FROM model_assets WHERE model_id = ?)")
                .bind(&model_id)
                .bind(&model_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        for folder_id in folder_ids {
            sqlx::query("DELETE FROM folders WHERE id = ? AND NOT EXISTS (SELECT 1 FROM assets WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM models WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM folders WHERE parent_id = ?)")
                .bind(&folder_id)
                .bind(&folder_id)
                .bind(&folder_id)
                .bind(&folder_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
    }
    transaction.commit().await.map_err(db_error)?;
    for row in photo_paths {
        for relative in [
            row.get::<String, _>("media_path"),
            row.get::<String, _>("thumbnail_path"),
        ] {
            if let Ok(path) = safe_media_path(&state.data_dir, &relative) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _ = std::fs::remove_dir_all(target_path);
    Ok(())
}

async fn build_package(
    model_id: &str,
    state: &AppState,
) -> CommandResult<(ProjectPackageManifest, Vec<ArchiveSource>)> {
    let project = sqlx::query(
        "SELECT display_name, notes, favorite, primary_asset_id FROM models WHERE id = ?",
    )
    .bind(model_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| "Project not found".to_string())?;
    let primary_asset_id: Option<String> = project.get("primary_asset_id");
    let asset_rows = sqlx::query("SELECT a.id, a.filename, a.extension, a.relative_path, a.byte_size, a.missing_since, ma.role, r.path root_path FROM model_assets ma JOIN assets a ON a.id = ma.asset_id JOIN library_roots r ON r.id = a.root_id WHERE ma.model_id = ? ORDER BY ma.sort_order, a.filename")
        .bind(model_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    if asset_rows.len() > MAX_PROJECT_FILES {
        return Err("Portable packages support up to 1,000 project files".into());
    }
    let mut files = Vec::with_capacity(asset_rows.len());
    let mut sources = Vec::new();
    for row in asset_rows {
        let filename: String = row.get("filename");
        if row.get::<Option<String>, _>("missing_since").is_some() {
            return Err(format!("Cannot export while {filename} is unavailable"));
        }
        let root = PathBuf::from(row.get::<String, _>("root_path"));
        let canonical_root = root
            .canonicalize()
            .map_err(|_| "A project library is offline".to_string())?;
        let path = root.join(row.get::<String, _>("relative_path"));
        let canonical = path
            .canonicalize()
            .map_err(|_| format!("Cannot export unavailable file: {filename}"))?;
        if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
            return Err("A project file is outside its approved library".into());
        }
        let id: String = row.get("id");
        let archive_path = format!("assets/{id}/{}", archive_filename(&filename));
        let hash_path = canonical.clone();
        let (byte_size, hash) = tokio::task::spawn_blocking(move || hash_file(&hash_path))
            .await
            .map_err(|error| error.to_string())??;
        files.push(PackageAsset {
            id: id.clone(),
            archive_path: archive_path.clone(),
            filename,
            extension: row.get("extension"),
            role: row.get("role"),
            primary: primary_asset_id.as_deref() == Some(id.as_str()),
            byte_size,
            blake3: hash,
        });
        sources.push(ArchiveSource {
            path: canonical,
            archive_path,
        });
    }
    let documentation = project_docs::documentation_for_project(model_id, &state.pool).await?;
    if documentation.photos.len() > MAX_PROJECT_PHOTOS {
        return Err("Portable packages cannot contain more than 200 photos".into());
    }
    let photo_rows = sqlx::query("SELECT id, media_path FROM project_photos WHERE model_id = ? ORDER BY sort_order, created_at")
        .bind(model_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let photo_details = documentation
        .photos
        .iter()
        .map(|photo| (photo.id.as_str(), photo))
        .collect::<HashMap<_, _>>();
    let mut photos = Vec::with_capacity(photo_rows.len());
    for row in photo_rows {
        let id: String = row.get("id");
        let detail = photo_details
            .get(id.as_str())
            .ok_or_else(|| "Project photo metadata is incomplete".to_string())?;
        let relative: String = row.get("media_path");
        let path = project_docs::resolve_project_photo_path(model_id, &relative, state).await?;
        if !path.is_file() {
            return Err(format!(
                "Cannot export unavailable photo: {}",
                detail.original_filename
            ));
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("jpg");
        let archive_path = format!("photos/{id}.{extension}");
        let hash_path = path.clone();
        let (byte_size, hash) = tokio::task::spawn_blocking(move || hash_file(&hash_path))
            .await
            .map_err(|error| error.to_string())??;
        photos.push(PackagePhoto {
            id: id.clone(),
            archive_path: archive_path.clone(),
            original_filename: detail.original_filename.clone(),
            mime_type: detail.mime_type.clone(),
            caption: detail.caption.clone(),
            sort_order: detail.sort_order,
            is_cover: detail.is_cover,
            byte_size,
            blake3: hash,
        });
        sources.push(ArchiveSource { path, archive_path });
    }
    let all_fields = project_docs::custom_field_definitions(&state.pool).await?;
    let used_fields = documentation
        .custom_values
        .iter()
        .map(|value| value.field_id.as_str())
        .collect::<HashSet<_>>();
    let custom_fields = all_fields
        .into_iter()
        .filter(|field| used_fields.contains(field.id.as_str()))
        .collect();
    let tag_rows = sqlx::query("SELECT t.name, t.color FROM tags t JOIN model_tags mt ON mt.tag_id = t.id WHERE mt.model_id = ? ORDER BY lower(t.name)")
        .bind(model_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let tags = tag_rows
        .into_iter()
        .map(|row| PackageTag {
            name: row.get("name"),
            color: row.get("color"),
        })
        .collect();
    let web_sources = package_web_sources(model_id, &state.pool).await?;
    let estimate = package_estimate(model_id, &state.pool).await?;
    let manifest = ProjectPackageManifest {
        format: PACKAGE_FORMAT.into(),
        format_version: PACKAGE_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        package_export_id: Uuid::new_v4().to_string(),
        exported_at: now(),
        project: PackageProject {
            id: model_id.to_string(),
            name: project.get("display_name"),
            notes: project.get("notes"),
            favorite: project.get::<i64, _>("favorite") != 0,
        },
        files,
        photos,
        custom_fields,
        custom_values: documentation.custom_values,
        bom_items: documentation.bom_items,
        assembly_steps: documentation.assembly_steps,
        tags,
        web_sources,
        estimate,
    };
    validate_manifest(&manifest)?;
    let total = sources.iter().try_fold(0_u64, |total, source| {
        let size = source
            .path
            .metadata()
            .map_err(|error| error.to_string())?
            .len();
        total
            .checked_add(size)
            .ok_or_else(|| "Project package size is invalid".to_string())
    })?;
    if total > MAX_PACKAGE_BYTES {
        return Err("Portable project packages cannot exceed 8 GB".into());
    }
    Ok((manifest, sources))
}

fn write_package(
    destination: &Path,
    manifest: &ProjectPackageManifest,
    sources: &[ArchiveSource],
) -> CommandResult<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = destination.with_file_name(format!(
        "{}.partial-{}",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project.volumproject"),
        Uuid::new_v4()
    ));
    let checksums = manifest
        .files
        .iter()
        .map(|entry| (entry.archive_path.clone(), entry.blake3.clone()))
        .chain(
            manifest
                .photos
                .iter()
                .map(|entry| (entry.archive_path.clone(), entry.blake3.clone())),
        )
        .collect::<BTreeMap<_, _>>();
    let expected = manifest
        .files
        .iter()
        .map(|entry| {
            (
                entry.archive_path.as_str(),
                (entry.byte_size, entry.blake3.as_str()),
            )
        })
        .chain(manifest.photos.iter().map(|entry| {
            (
                entry.archive_path.as_str(),
                (entry.byte_size, entry.blake3.as_str()),
            )
        }))
        .collect::<HashMap<_, _>>();
    let result = (|| -> CommandResult<()> {
        let file = File::create(&temporary).map_err(|error| error.to_string())?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer
            .start_file("manifest.json", options)
            .map_err(|error| error.to_string())?;
        writer
            .write_all(&serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        writer
            .start_file("checksums.json", options)
            .map_err(|error| error.to_string())?;
        writer
            .write_all(&serde_json::to_vec_pretty(&checksums).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        for source in sources {
            let (expected_size, expected_hash) = expected
                .get(source.archive_path.as_str())
                .ok_or_else(|| "A package payload was not declared in its manifest".to_string())?;
            writer
                .start_file(&source.archive_path, options)
                .map_err(|error| error.to_string())?;
            let mut input = File::open(&source.path).map_err(|error| error.to_string())?;
            let mut hasher = blake3::Hasher::new();
            let mut written = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                written = written
                    .checked_add(read as u64)
                    .ok_or_else(|| "Project package size is invalid".to_string())?;
                hasher.update(&buffer[..read]);
                writer
                    .write_all(&buffer[..read])
                    .map_err(|error| error.to_string())?;
            }
            if written != *expected_size || hasher.finalize().to_hex().as_str() != *expected_hash {
                return Err(format!(
                    "Project file changed while the package was being exported: {}",
                    source.path.display()
                ));
            }
        }
        writer.finish().map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if destination.exists() {
        if let Err(error) = std::fs::remove_file(destination) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.to_string());
        }
    }
    if let Err(error) = std::fs::rename(&temporary, destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

fn validate_package(path: &Path) -> CommandResult<ProjectPackageManifest> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| "Choose an available Volum project package".to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PACKAGE_BYTES {
        return Err("The project package is empty or exceeds the 8 GB limit".into());
    }
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive =
        ZipArchive::new(file).map_err(|_| "The file is not a valid project package".to_string())?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err("The project package contains too many files".into());
    }
    let manifest: ProjectPackageManifest = read_json_entry(&mut archive, "manifest.json")?;
    validate_manifest(&manifest)?;
    let checksums: BTreeMap<String, String> = read_json_entry(&mut archive, "checksums.json")?;
    let expected = manifest
        .files
        .iter()
        .map(|entry| {
            (
                entry.archive_path.clone(),
                (entry.byte_size, entry.blake3.clone()),
            )
        })
        .chain(manifest.photos.iter().map(|entry| {
            (
                entry.archive_path.clone(),
                (entry.byte_size, entry.blake3.clone()),
            )
        }))
        .collect::<BTreeMap<_, _>>();
    if expected.len() != manifest.files.len() + manifest.photos.len() {
        return Err("The project package contains duplicate payload paths".into());
    }
    let expected_checksums = expected
        .iter()
        .map(|(path, (_, hash))| (path.clone(), hash.clone()))
        .collect::<BTreeMap<_, _>>();
    if checksums != expected_checksums {
        return Err("The project package checksum index does not match its manifest".into());
    }
    let mut archive_names = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_string();
        if !safe_archive_path(&name) || !archive_names.insert(name.clone()) {
            return Err("The project package contains an unsafe or duplicate path".into());
        }
        if name != "manifest.json" && name != "checksums.json" && !expected.contains_key(&name) {
            return Err("The project package contains an undeclared file".into());
        }
    }
    if archive_names.len() != expected.len() + 2 {
        return Err("The project package is missing a declared file".into());
    }
    let mut total = 0_u64;
    for (name, (expected_size, expected_hash)) in expected {
        let mut entry = archive
            .by_name(&name)
            .map_err(|_| format!("Package file is missing: {name}"))?;
        if entry.size() != expected_size {
            return Err(format!(
                "Package file size does not match its manifest: {name}"
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "Project package size is invalid".to_string())?;
        if total > MAX_PACKAGE_BYTES {
            return Err("The project package exceeds the 8 GB extraction limit".into());
        }
        let actual = hash_reader(&mut entry)?;
        if actual != expected_hash {
            return Err(format!("Project package checksum failed: {name}"));
        }
    }
    Ok(manifest)
}

fn validate_manifest(manifest: &ProjectPackageManifest) -> CommandResult<()> {
    if manifest.format != PACKAGE_FORMAT || manifest.format_version != PACKAGE_FORMAT_VERSION {
        return Err("This Volum project package version is not supported".into());
    }
    if manifest.project.id.trim().is_empty()
        || manifest.project.id.chars().count() > 200
        || manifest.package_export_id.trim().is_empty()
        || manifest.package_export_id.chars().count() > 200
        || manifest.app_version.chars().count() > 100
    {
        return Err("The project package identity is invalid".into());
    }
    chrono::DateTime::parse_from_rfc3339(&manifest.exported_at)
        .map_err(|_| "The project package export date is invalid".to_string())?;
    if manifest.project.name.trim().is_empty() || manifest.project.name.chars().count() > 160 {
        return Err("The project package has an invalid project name".into());
    }
    if manifest.project.notes.chars().count() > 100_000 {
        return Err("The project package notes are too long".into());
    }
    if manifest.files.len() > MAX_PROJECT_FILES {
        return Err("The project package may contain up to 1,000 model files".into());
    }
    if manifest.photos.len() > MAX_PROJECT_PHOTOS {
        return Err("The project package contains too many photos".into());
    }
    if manifest
        .photos
        .iter()
        .filter(|photo| photo.is_cover)
        .count()
        > 1
    {
        return Err("The project package contains more than one cover photo".into());
    }
    if manifest.files.iter().filter(|asset| asset.primary).count() > 1 {
        return Err("The project package contains more than one primary file".into());
    }
    let mut asset_ids = HashSet::new();
    for asset in &manifest.files {
        if !safe_archive_path(&asset.archive_path)
            || !asset
                .archive_path
                .starts_with(&format!("assets/{}/", asset.id))
            || !SUPPORTED_FILES.contains(&asset.extension.as_str())
            || asset.filename.trim().is_empty()
            || asset.filename.chars().count() > 255
            || ![
                "source",
                "printable",
                "plate",
                "reference",
                "version",
                "variant",
            ]
            .contains(&asset.role.as_str())
            || asset.byte_size == 0
            || asset.byte_size > MAX_PACKAGE_BYTES
            || !valid_checksum(&asset.blake3)
            || !asset_ids.insert(asset.id.as_str())
        {
            return Err("The project package contains invalid model-file metadata".into());
        }
    }
    let mut photo_ids = HashSet::new();
    for photo in &manifest.photos {
        if !safe_archive_path(&photo.archive_path)
            || !photo.archive_path.starts_with("photos/")
            || !["image/jpeg", "image/png", "image/webp"].contains(&photo.mime_type.as_str())
            || photo.original_filename.trim().is_empty()
            || photo.original_filename.chars().count() > 255
            || photo.byte_size == 0
            || photo.byte_size > 25 * 1024 * 1024
            || !photo.sort_order.is_finite()
            || !valid_checksum(&photo.blake3)
            || photo.caption.chars().count() > 500
            || !photo_ids.insert(photo.id.as_str())
        {
            return Err("The project package contains invalid photo metadata".into());
        }
    }
    if manifest.custom_fields.len() > 200
        || manifest.custom_values.len() > 200
        || manifest.bom_items.len() > 2_000
        || manifest.assembly_steps.len() > 1_000
        || manifest.tags.len() > 500
        || manifest.web_sources.len() > 100
    {
        return Err("The project package contains too much documentation".into());
    }
    let mut field_ids = HashSet::new();
    for field in &manifest.custom_fields {
        if !field_ids.insert(field.id.as_str())
            || field.name.trim().is_empty()
            || field.name.chars().count() > 80
            || !["text", "number", "date", "boolean", "url", "select"]
                .contains(&field.field_type.as_str())
            || field
                .unit
                .as_deref()
                .is_some_and(|unit| unit.chars().count() > 20)
            || field.options.len() > 100
            || !unique_case_insensitive(&field.options)
            || field
                .options
                .iter()
                .any(|option| option.trim().is_empty() || option.chars().count() > 80)
            || (field.field_type == "select" && field.options.is_empty())
        {
            return Err("The project package contains an invalid custom field".into());
        }
    }
    let field_map = manifest
        .custom_fields
        .iter()
        .map(|field| (field.id.as_str(), field))
        .collect::<HashMap<_, _>>();
    let mut value_fields = HashSet::new();
    for value in &manifest.custom_values {
        let field = field_map
            .get(value.field_id.as_str())
            .ok_or_else(|| "A packaged custom value has no field definition".to_string())?;
        if !value_fields.insert(value.field_id.as_str()) {
            return Err("The project package repeats a custom field value".into());
        }
        project_docs::validate_custom_value(
            &field.field_type,
            &serde_json::to_string(&field.options).map_err(|error| error.to_string())?,
            &value.value,
        )?;
    }
    let mut bom_ids = HashSet::new();
    for item in &manifest.bom_items {
        if !bom_ids.insert(item.id.as_str())
            || !["printed", "purchased"].contains(&item.kind.as_str())
            || item.name.trim().is_empty()
            || item.name.chars().count() > 160
            || !item.quantity.is_finite()
            || item.quantity <= 0.0
            || item.quantity > 1_000_000.0
            || item.unit.trim().is_empty()
            || item.unit.chars().count() > 24
            || item.notes.chars().count() > 10_000
            || !item.sort_order.is_finite()
            || item
                .linked_asset_id
                .as_deref()
                .is_some_and(|id| !asset_ids.contains(id))
            || item.url.as_deref().is_some_and(|url| !valid_http_url(url))
        {
            return Err("The project package contains an invalid BOM item".into());
        }
    }
    let mut step_ids = HashSet::new();
    for step in &manifest.assembly_steps {
        if !step_ids.insert(step.id.as_str())
            || step.title.trim().is_empty()
            || step.title.chars().count() > 160
            || step.instructions.chars().count() > 50_000
            || !step.sort_order.is_finite()
            || step.bom_item_ids.len() > 500
            || step.photo_ids.len() > 100
            || !unique_strings(&step.bom_item_ids)
            || !unique_strings(&step.photo_ids)
            || step
                .bom_item_ids
                .iter()
                .any(|id| !bom_ids.contains(id.as_str()))
            || step
                .photo_ids
                .iter()
                .any(|id| !photo_ids.contains(id.as_str()))
        {
            return Err("The project package contains an invalid assembly step".into());
        }
    }
    for source in &manifest.web_sources {
        if !matches!(source.provider.as_str(), "makerworld" | "printables")
            || !valid_https_url(&source.canonical_url)
            || source.title.trim().is_empty()
            || source.title.chars().count() > 500
            || source
                .creator
                .as_deref()
                .is_some_and(|value| value.chars().count() > 200)
            || source
                .description
                .as_deref()
                .is_some_and(|value| value.chars().count() > 4_000)
            || source
                .license
                .as_deref()
                .is_some_and(|value| value.chars().count() > 200)
            || source
                .image_url
                .as_deref()
                .is_some_and(|value| !valid_https_url(value))
            || source
                .filament_grams
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err("The project package contains invalid web-source metadata".into());
        }
    }
    if let Some(estimate) = &manifest.estimate {
        if !estimate.plastic_g.is_finite()
            || estimate.plastic_g <= 0.0
            || estimate.quantity <= 0
            || estimate.quantity > 1_000_000
            || estimate
                .asset_id
                .as_deref()
                .is_some_and(|id| !asset_ids.contains(id))
        {
            return Err("The project package contains an invalid cost estimate".into());
        }
        if let Some(material) = &estimate.material {
            if material.name.trim().is_empty()
                || material.name.chars().count() > 80
                || material.material_type.trim().is_empty()
                || material.material_type.chars().count() > 40
                || material.currency.len() != 3
                || !material
                    .currency
                    .chars()
                    .all(|character| character.is_ascii_uppercase())
                || material
                    .color_name
                    .as_deref()
                    .is_some_and(|value| value.chars().count() > 80)
                || material
                    .color_hex
                    .as_deref()
                    .is_some_and(|value| !valid_color(value))
                || material.spool_price_minor < 0
                || !material.spool_weight_g.is_finite()
                || material.spool_weight_g <= 0.0
                || material
                    .density
                    .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                return Err("The project package contains invalid material metadata".into());
            }
        }
    }
    let mut tag_names = HashSet::new();
    for tag in &manifest.tags {
        if tag.name.trim().is_empty()
            || tag.name.chars().count() > 40
            || !valid_color(&tag.color)
            || !tag_names.insert(tag.name.trim().to_lowercase())
        {
            return Err("The project package contains invalid tag metadata".into());
        }
    }
    Ok(())
}

fn valid_checksum(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn unique_case_insensitive(values: &[String]) -> bool {
    let mut seen = HashSet::new();
    values
        .iter()
        .all(|value| seen.insert(value.trim().to_lowercase()))
}

fn unique_strings(values: &[String]) -> bool {
    values.iter().collect::<HashSet<_>>().len() == values.len()
}

fn valid_http_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .ok()
        .is_some_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn valid_https_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .ok()
        .is_some_and(|url| url.scheme() == "https")
}

fn extract_package_payloads(
    package_path: &Path,
    manifest: &ProjectPackageManifest,
    staging: &Path,
    photo_staging: &Path,
    target_folder: &str,
) -> CommandResult<(Vec<String>, HashMap<String, PathBuf>)> {
    let file = File::open(package_path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut used_names = HashSet::new();
    let mut relative_paths = Vec::with_capacity(manifest.files.len());
    for asset in &manifest.files {
        let output_name = unique_filename(&asset.filename, &asset.extension, &mut used_names);
        let output = staging.join(&output_name);
        extract_entry(&mut archive, &asset.archive_path, &output, &asset.blake3)?;
        relative_paths.push(format!("{target_folder}/{output_name}"));
    }
    let mut photo_paths = HashMap::new();
    for photo in &manifest.photos {
        let extension = match photo.mime_type.as_str() {
            "image/png" => "png",
            "image/webp" => "webp",
            _ => "jpg",
        };
        let output = photo_staging.join(format!("{}.{}", photo.id, extension));
        extract_entry(&mut archive, &photo.archive_path, &output, &photo.blake3)?;
        photo_paths.insert(photo.id.clone(), output);
    }
    Ok((relative_paths, photo_paths))
}

async fn ensure_package_folder(
    root_id: &str,
    relative: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<String> {
    let root_name: String =
        sqlx::query_scalar("SELECT display_name FROM library_roots WHERE id = ?")
            .bind(root_id)
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
    let root_folder_id: String =
        sqlx::query_scalar("SELECT id FROM folders WHERE root_id = ? AND relative_path = ''")
            .bind(root_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?
            .unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, NULL, '', ?, ?)")
        .bind(&root_folder_id)
        .bind(root_id)
        .bind(root_name)
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    let mut parent_id = root_folder_id;
    let mut current = String::new();
    for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        let folder_id: String =
            sqlx::query_scalar("SELECT id FROM folders WHERE root_id = ? AND relative_path = ?")
                .bind(root_id)
                .bind(&current)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?
                .unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&folder_id)
            .bind(root_id)
            .bind(&parent_id)
            .bind(&current)
            .bind(segment)
            .bind(now())
            .execute(pool)
            .await
            .map_err(db_error)?;
        parent_id = folder_id;
    }
    Ok(parent_id)
}

async fn create_imported_project(
    manifest: &ProjectPackageManifest,
    asset_ids: &[String],
    root_id: &str,
    target_folder: &str,
    state: &AppState,
) -> CommandResult<String> {
    if asset_ids.len() != manifest.files.len() {
        return Err("Imported project files were not indexed completely".into());
    }
    let folder_id: String = if let Some(asset_id) = asset_ids.first() {
        sqlx::query_scalar("SELECT folder_id FROM assets WHERE id = ?")
            .bind(asset_id)
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?
    } else {
        ensure_package_folder(root_id, target_folder, &state.pool).await?
    };
    let project_id = Uuid::new_v4().to_string();
    let storage_mode: String = sqlx::query_scalar("SELECT CASE root_kind WHEN 'managed' THEN 'managed' ELSE 'linked' END FROM library_roots WHERE id = ?")
        .bind(root_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    let timestamp = now();
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    sqlx::query("INSERT INTO models (id, folder_id, display_name, notes, favorite, grouping_key, version_key, bundle_mode, storage_mode, added_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, '', 'manual', ?, ?, ?)")
        .bind(&project_id)
        .bind(&folder_id)
        .bind(manifest.project.name.trim())
        .bind(&manifest.project.notes)
        .bind(i64::from(manifest.project.favorite))
        .bind(format!("manual:{project_id}"))
        .bind(storage_mode)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    let mut prior_projects = Vec::new();
    for (index, asset_id) in asset_ids.iter().enumerate() {
        let prior: Vec<String> =
            sqlx::query_scalar("SELECT model_id FROM model_assets WHERE asset_id = ?")
                .bind(asset_id)
                .fetch_all(&mut *transaction)
                .await
                .map_err(db_error)?;
        prior_projects.extend(prior);
        sqlx::query(
            "UPDATE model_assets SET model_id = ?, role = ?, sort_order = ? WHERE asset_id = ?",
        )
        .bind(&project_id)
        .bind(&manifest.files[index].role)
        .bind(index as i64)
        .bind(asset_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    }
    if let Some((index, _)) = manifest
        .files
        .iter()
        .enumerate()
        .find(|(_, asset)| asset.primary)
    {
        sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
            .bind(&asset_ids[index])
            .bind(&project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    } else if let Some(asset_id) = asset_ids.first() {
        sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
            .bind(asset_id)
            .bind(&project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    prior_projects.sort();
    prior_projects.dedup();
    for prior in prior_projects {
        sqlx::query("DELETE FROM models WHERE id = ? AND NOT EXISTS (SELECT 1 FROM model_assets WHERE model_id = ?)")
            .bind(&prior)
            .bind(&prior)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("INSERT INTO project_package_imports (model_id, package_project_id, package_export_id, imported_at) VALUES (?, ?, ?, ?)")
        .bind(&project_id)
        .bind(&manifest.project.id)
        .bind(&manifest.package_export_id)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(project_id)
}

async fn apply_imported_documentation(
    manifest: &ProjectPackageManifest,
    project_id: &str,
    asset_ids: &[String],
    photo_paths: &HashMap<String, PathBuf>,
    state: &AppState,
) -> CommandResult<()> {
    for tag in &manifest.tags {
        if tag.name.trim().is_empty() || tag.name.chars().count() > 40 {
            continue;
        }
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM tags WHERE name = ? COLLATE NOCASE")
                .bind(tag.name.trim())
                .fetch_optional(&state.pool)
                .await
                .map_err(db_error)?;
        let tag_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT OR IGNORE INTO tags (id, name, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&tag_id)
            .bind(tag.name.trim())
            .bind(if valid_color(&tag.color) { tag.color.as_str() } else { "#ff5a36" })
            .bind(now())
            .bind(now())
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        sqlx::query(
            "INSERT OR IGNORE INTO model_tags (model_id, tag_id, added_at) VALUES (?, ?, ?)",
        )
        .bind(project_id)
        .bind(tag_id)
        .bind(now())
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    }
    let mut field_map = HashMap::new();
    for field in &manifest.custom_fields {
        let local_id = import_field_definition(field, &state.pool).await?;
        field_map.insert(field.id.clone(), local_id);
    }
    for value in &manifest.custom_values {
        let Some(field_id) = field_map.get(&value.field_id) else {
            continue;
        };
        let row = sqlx::query(
            "SELECT field_type, options_json FROM custom_field_definitions WHERE id = ?",
        )
        .bind(field_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
        project_docs::validate_custom_value(
            &row.get::<String, _>("field_type"),
            &row.get::<String, _>("options_json"),
            &value.value,
        )?;
        sqlx::query("INSERT OR REPLACE INTO project_custom_values (model_id, field_id, value_json, updated_at) VALUES (?, ?, ?, ?)")
            .bind(project_id)
            .bind(field_id)
            .bind(serde_json::to_string(&value.value).map_err(|error| error.to_string())?)
            .bind(now())
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    let asset_map = manifest
        .files
        .iter()
        .zip(asset_ids)
        .map(|(packaged, local)| (packaged.id.as_str(), local.as_str()))
        .collect::<HashMap<_, _>>();
    let mut photo_map = HashMap::new();
    for photo in &manifest.photos {
        let source = photo_paths
            .get(&photo.id)
            .ok_or_else(|| "An imported project photo is missing".to_string())?;
        let imported = project_docs::store_project_photo(
            project_id,
            source,
            Some(&photo.original_filename),
            &photo.caption,
            Some(photo.sort_order),
            Some(photo.is_cover),
            state,
        )
        .await?;
        photo_map.insert(photo.id.clone(), imported.id);
    }
    let mut bom_map = HashMap::new();
    for item in &manifest.bom_items {
        let id = Uuid::new_v4().to_string();
        let linked = item
            .linked_asset_id
            .as_deref()
            .and_then(|asset_id| asset_map.get(asset_id).copied());
        sqlx::query("INSERT INTO project_bom_items (id, model_id, kind, linked_asset_id, name, quantity, unit, url, notes, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(if item.kind == "printed" { "printed" } else { "purchased" })
            .bind(linked)
            .bind(item.name.trim().chars().take(160).collect::<String>())
            .bind(item.quantity.max(f64::EPSILON))
            .bind(item.unit.trim().chars().take(24).collect::<String>())
            .bind(&item.url)
            .bind(item.notes.chars().take(10_000).collect::<String>())
            .bind(item.sort_order)
            .bind(now())
            .bind(now())
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        bom_map.insert(item.id.clone(), id);
    }
    for step in &manifest.assembly_steps {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO assembly_steps (id, model_id, title, instructions, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(step.title.trim().chars().take(160).collect::<String>())
            .bind(step.instructions.chars().take(50_000).collect::<String>())
            .bind(step.sort_order)
            .bind(now())
            .bind(now())
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        for (index, old_id) in step.bom_item_ids.iter().enumerate() {
            if let Some(local_id) = bom_map.get(old_id) {
                sqlx::query("INSERT INTO assembly_step_bom_items (step_id, bom_item_id, sort_order) VALUES (?, ?, ?)")
                    .bind(&id)
                    .bind(local_id)
                    .bind(index as f64)
                    .execute(&state.pool)
                    .await
                    .map_err(db_error)?;
            }
        }
        for (index, old_id) in step.photo_ids.iter().enumerate() {
            if let Some(local_id) = photo_map.get(old_id) {
                sqlx::query("INSERT INTO assembly_step_photos (step_id, photo_id, sort_order) VALUES (?, ?, ?)")
                    .bind(&id)
                    .bind(local_id)
                    .bind(index as f64)
                    .execute(&state.pool)
                    .await
                    .map_err(db_error)?;
            }
        }
    }
    import_web_sources(&manifest.web_sources, project_id, &state.pool).await?;
    import_estimate(
        manifest.estimate.as_ref(),
        project_id,
        &asset_map,
        &state.pool,
    )
    .await?;
    Ok(())
}

async fn import_field_definition(
    field: &CustomFieldDefinition,
    pool: &sqlx::SqlitePool,
) -> CommandResult<String> {
    if !["text", "number", "date", "boolean", "url", "select"].contains(&field.field_type.as_str())
        || field.name.trim().is_empty()
    {
        return Err("The project package contains an invalid custom field".into());
    }
    if let Some(row) = sqlx::query("SELECT id, field_type, unit, options_json FROM custom_field_definitions WHERE name = ? COLLATE NOCASE")
        .bind(field.name.trim())
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
    {
        let local_type: String = row.get("field_type");
        let local_unit: Option<String> = row.get("unit");
        let local_options: Vec<String> =
            serde_json::from_str(&row.get::<String, _>("options_json")).unwrap_or_default();
        let compatible = local_type == field.field_type
            && (field.field_type != "number" || local_unit == field.unit)
            && (field.field_type != "select" || local_options == field.options);
        if compatible {
            return Ok(row.get("id"));
        }
    }
    let mut name = field.name.trim().chars().take(80).collect::<String>();
    let base = name.clone();
    let mut suffix = 2;
    while sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM custom_field_definitions WHERE name = ? COLLATE NOCASE",
    )
    .bind(&name)
    .fetch_one(pool)
    .await
    .map_err(db_error)?
        != 0
    {
        name = format!(
            "{} (imported {suffix})",
            base.chars().take(62).collect::<String>()
        );
        suffix += 1;
    }
    let id = Uuid::new_v4().to_string();
    let order: f64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM custom_field_definitions")
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
    sqlx::query("INSERT INTO custom_field_definitions (id, name, field_type, unit, options_json, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(&field.field_type)
        .bind(&field.unit)
        .bind(serde_json::to_string(&field.options).map_err(|error| error.to_string())?)
        .bind(order)
        .bind(now())
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(id)
}

async fn preview_for_manifest(
    manifest: &ProjectPackageManifest,
    state: &AppState,
) -> CommandResult<ProjectPackagePreview> {
    let already_imported: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_package_imports WHERE package_project_id = ?",
    )
    .bind(&manifest.project.id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    let total_bytes = manifest
        .files
        .iter()
        .map(|entry| entry.byte_size)
        .chain(manifest.photos.iter().map(|entry| entry.byte_size))
        .sum();
    Ok(ProjectPackagePreview {
        format_version: manifest.format_version,
        app_version: manifest.app_version.clone(),
        package_project_id: manifest.project.id.clone(),
        package_export_id: manifest.package_export_id.clone(),
        project_name: manifest.project.name.clone(),
        exported_at: manifest.exported_at.clone(),
        file_count: manifest.files.len(),
        photo_count: manifest.photos.len(),
        bom_item_count: manifest.bom_items.len(),
        assembly_step_count: manifest.assembly_steps.len(),
        total_bytes,
        already_imported: already_imported != 0,
    })
}

async fn package_web_sources(
    model_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<Vec<PackageWebSource>> {
    let rows = sqlx::query("SELECT provider, remote_id, canonical_url, title, creator, description, license, image_url, filament_grams FROM web_sources ws WHERE ws.model_id = ? OR EXISTS (SELECT 1 FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.root_id = ws.root_id AND a.relative_path = ws.relative_path) ORDER BY updated_at DESC")
        .bind(model_id)
        .bind(model_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| PackageWebSource {
            provider: row.get("provider"),
            remote_id: row.get("remote_id"),
            canonical_url: row.get("canonical_url"),
            title: row.get("title"),
            creator: row.get("creator"),
            description: row.get("description"),
            license: row.get("license"),
            image_url: row.get("image_url"),
            filament_grams: row.get("filament_grams"),
        })
        .collect())
}

async fn package_estimate(
    model_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<Option<PackageEstimate>> {
    let row = sqlx::query("SELECT e.asset_id, e.plastic_g, e.quantity, e.source, m.name material_name, m.material_type, m.color_name, m.color_hex, m.spool_price_minor, m.currency, m.spool_weight_g, m.density FROM cost_estimates e LEFT JOIN materials m ON m.id = e.material_id WHERE e.model_id = ? ORDER BY e.updated_at DESC LIMIT 1")
        .bind(model_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    Ok(row.map(|row| PackageEstimate {
        asset_id: row.get("asset_id"),
        plastic_g: row.get("plastic_g"),
        quantity: row.get("quantity"),
        source: row.get("source"),
        material: row
            .get::<Option<String>, _>("material_name")
            .map(|name| PackageMaterial {
                name,
                material_type: row.get("material_type"),
                color_name: row.get("color_name"),
                color_hex: row.get("color_hex"),
                spool_price_minor: row.get("spool_price_minor"),
                currency: row.get("currency"),
                spool_weight_g: row.get("spool_weight_g"),
                density: row.get("density"),
            }),
    }))
}

async fn import_web_sources(
    sources: &[PackageWebSource],
    project_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<()> {
    for source in sources {
        if !matches!(source.provider.as_str(), "makerworld" | "printables")
            || reqwest::Url::parse(&source.canonical_url)
                .ok()
                .is_none_or(|url| url.scheme() != "https")
        {
            continue;
        }
        let timestamp = now();
        sqlx::query("INSERT INTO web_sources (id, provider, remote_id, canonical_url, title, creator, description, license, image_url, filament_grams, status, model_id, fetched_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'imported', ?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(&source.provider)
            .bind(&source.remote_id)
            .bind(&source.canonical_url)
            .bind(source.title.chars().take(500).collect::<String>())
            .bind(&source.creator)
            .bind(&source.description)
            .bind(&source.license)
            .bind(&source.image_url)
            .bind(source.filament_grams)
            .bind(project_id)
            .bind(&timestamp)
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

async fn import_estimate(
    estimate: Option<&PackageEstimate>,
    project_id: &str,
    asset_map: &HashMap<&str, &str>,
    pool: &sqlx::SqlitePool,
) -> CommandResult<()> {
    let Some(estimate) = estimate else {
        return Ok(());
    };
    if !estimate.plastic_g.is_finite()
        || estimate.plastic_g <= 0.0
        || estimate.quantity <= 0
        || estimate.quantity > 1_000_000
    {
        return Ok(());
    }
    let material_id = if let Some(material) = &estimate.material {
        let existing = sqlx::query(
            "SELECT id, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density FROM materials WHERE name = ? COLLATE NOCASE AND material_type = ? ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(&material.name)
        .bind(&material.material_type)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
        let matching_id = existing.as_ref().and_then(|row| {
            (row.get::<Option<String>, _>("color_name") == material.color_name
                && row.get::<Option<String>, _>("color_hex") == material.color_hex
                && row.get::<i64, _>("spool_price_minor") == material.spool_price_minor
                && row.get::<String, _>("currency") == material.currency
                && row.get::<f64, _>("spool_weight_g") == material.spool_weight_g
                && row.get::<Option<f64>, _>("density") == material.density)
                .then(|| row.get::<String, _>("id"))
        });
        if matching_id.is_some() {
            matching_id
        } else if material.spool_weight_g.is_finite() && material.spool_weight_g > 0.0 {
            let id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO materials (id, name, material_type, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(material.name.chars().take(80).collect::<String>())
                .bind(material.material_type.chars().take(40).collect::<String>())
                .bind(&material.color_name)
                .bind(&material.color_hex)
                .bind(material.spool_price_minor.max(0))
                .bind(material.currency.chars().take(3).collect::<String>())
                .bind(material.spool_weight_g)
                .bind(material.density)
                .bind(now())
                .bind(now())
                .execute(pool)
                .await
                .map_err(db_error)?;
            Some(id)
        } else {
            None
        }
    } else {
        None
    };
    let asset_id = estimate
        .asset_id
        .as_deref()
        .and_then(|id| asset_map.get(id).copied());
    sqlx::query("INSERT INTO cost_estimates (id, model_id, asset_id, material_id, plastic_g, quantity, source, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(project_id)
        .bind(asset_id)
        .bind(material_id)
        .bind(estimate.plastic_g)
        .bind(estimate.quantity)
        .bind(if ["file", "web", "volume", "manual"].contains(&estimate.source.as_str()) { estimate.source.as_str() } else { "manual" })
        .bind(now())
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn read_json_entry<T: DeserializeOwned>(
    archive: &mut ZipArchive<File>,
    name: &str,
) -> CommandResult<T> {
    let entry = archive
        .by_name(name)
        .map_err(|_| format!("Project package {name} is missing"))?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(format!("Project package {name} is too large"));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| format!("Project package {name} is invalid"))
}

fn extract_entry(
    archive: &mut ZipArchive<File>,
    archive_path: &str,
    output: &Path,
    expected_hash: &str,
) -> CommandResult<()> {
    let mut entry = archive
        .by_name(archive_path)
        .map_err(|_| format!("Package file is missing: {archive_path}"))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = entry.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
    }
    if hasher.finalize().to_hex().as_str() != expected_hash {
        let _ = std::fs::remove_file(output);
        return Err(format!("Project package checksum failed: {archive_path}"));
    }
    Ok(())
}

fn hash_file(path: &Path) -> CommandResult<(u64, String)> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let hash = hash_reader(&mut file)?;
    Ok((size, hash))
}

fn hash_reader(reader: &mut impl Read) -> CommandResult<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn normalize_relative_folder(value: &str) -> CommandResult<String> {
    let normalized = value.trim().replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Choose a safe relative destination folder".into());
    }
    let segments = normalized.split('/').map(safe_name).collect::<Vec<_>>();
    if segments.iter().any(String::is_empty) || segments.len() > 20 {
        return Err("The destination folder is invalid".into());
    }
    Ok(segments.join("/"))
}

fn unique_destination(root: &Path, requested: &str) -> CommandResult<(String, PathBuf)> {
    let path = Path::new(requested);
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "The destination folder is invalid".to_string())?;
    for suffix in 1..=1_000 {
        let candidate_name = if suffix == 1 {
            name.to_string()
        } else {
            format!("{name} ({suffix})")
        };
        let relative = parent
            .join(candidate_name)
            .to_string_lossy()
            .replace('\\', "/");
        let destination = root.join(&relative);
        if !destination.exists() {
            return Ok((relative, destination));
        }
    }
    Err("Could not find an unused destination folder".into())
}

fn create_destination_without_symlinks(root: &Path, relative: &str) -> CommandResult<()> {
    let mut current = root.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(segment) = component else {
            return Err("The destination folder is invalid".into());
        };
        current.push(segment);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("The destination path contains a link or non-folder entry".into());
                }
                if index + 1 == components.len() {
                    return Err("The destination folder was created by another process; choose another destination".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|error| error.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn unique_filename(filename: &str, extension: &str, used: &mut HashSet<String>) -> String {
    let clean = archive_filename(filename);
    let stem = Path::new(&clean)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(safe_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "model".into());
    let extension = if SUPPORTED_FILES.contains(&extension) {
        extension
    } else {
        "stl"
    };
    for suffix in 1..=10_000 {
        let candidate = if suffix == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({suffix}).{extension}")
        };
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("{}.{}", Uuid::new_v4(), extension)
}

fn archive_filename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|part| part.to_str())
        .map(|part| {
            part.chars()
                .filter(|character| !character.is_control())
                .collect()
        })
        .filter(|part: &String| !part.is_empty() && part != "." && part != "..")
        .unwrap_or_else(|| "file".into())
}

fn safe_name(value: &str) -> String {
    let value = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(120)
        .collect::<String>();
    let value = value.trim_matches(['.', ' ']).to_string();
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        format!("_{value}")
    } else {
        value
    }
}

fn safe_archive_path(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && !path.is_absolute()
        && !name.contains('\\')
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn safe_media_path(data_dir: &Path, relative: &str) -> CommandResult<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Project photo has an unsafe storage path".into());
    }
    Ok(data_dir.join("media").join(path))
}

fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(hash: String, size: u64) -> ProjectPackageManifest {
        ProjectPackageManifest {
            format: PACKAGE_FORMAT.into(),
            format_version: PACKAGE_FORMAT_VERSION,
            app_version: "test".into(),
            package_export_id: "export-test".into(),
            exported_at: "2026-09-24T00:00:00Z".into(),
            project: PackageProject {
                id: "project-test".into(),
                name: "Portable fixture".into(),
                notes: "Round trip".into(),
                favorite: true,
            },
            files: vec![PackageAsset {
                id: "asset-test".into(),
                archive_path: "assets/asset-test/fixture.stl".into(),
                filename: "fixture.stl".into(),
                extension: "stl".into(),
                role: "printable".into(),
                primary: true,
                byte_size: size,
                blake3: hash,
            }],
            photos: Vec::new(),
            custom_fields: Vec::new(),
            custom_values: Vec::new(),
            bom_items: Vec::new(),
            assembly_steps: Vec::new(),
            tags: Vec::new(),
            web_sources: Vec::new(),
            estimate: None,
        }
    }

    fn test_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("volum-project-package-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn rejects_unsafe_package_paths() {
        assert!(safe_archive_path("assets/id/model.stl"));
        assert!(!safe_archive_path("../model.stl"));
        assert!(!safe_archive_path("/model.stl"));
        assert!(!safe_archive_path("assets\\model.stl"));
    }

    #[test]
    fn creates_portable_safe_names() {
        assert_eq!(safe_name("Lamp: final/ready"), "Lamp_ final_ready");
        assert_eq!(archive_filename("folder/model.stl"), "model.stl");
    }

    #[test]
    fn writes_and_validates_a_self_contained_package() {
        let directory = test_directory("round-trip");
        std::fs::create_dir_all(&directory).unwrap();
        let payload = directory.join("fixture.stl");
        let bytes = b"solid fixture\nendsolid fixture\n";
        std::fs::write(&payload, bytes).unwrap();
        let manifest =
            sample_manifest(blake3::hash(bytes).to_hex().to_string(), bytes.len() as u64);
        let package = directory.join("fixture.volumproject");
        write_package(
            &package,
            &manifest,
            &[ArchiveSource {
                path: payload,
                archive_path: manifest.files[0].archive_path.clone(),
            }],
        )
        .unwrap();
        let decoded = validate_package(&package).unwrap();
        assert_eq!(decoded.project.name, "Portable fixture");
        assert_eq!(decoded.files[0].blake3, manifest.files[0].blake3);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn writes_and_validates_an_empty_project_package() {
        let directory = test_directory("empty");
        std::fs::create_dir_all(&directory).unwrap();
        let mut manifest = sample_manifest(String::new(), 0);
        manifest.files.clear();
        let package = directory.join("empty.volumproject");
        write_package(&package, &manifest, &[]).unwrap();
        let decoded = validate_package(&package).unwrap();
        assert!(decoded.files.is_empty());
        assert_eq!(decoded.project.name, "Portable fixture");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn imports_a_managed_package_through_the_journaled_storage_pipeline() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        let payload = source.path().join("fixture.stl");
        let bytes = b"solid fixture\nendsolid fixture\n";
        std::fs::write(&payload, bytes).unwrap();
        let manifest =
            sample_manifest(blake3::hash(bytes).to_hex().to_string(), bytes.len() as u64);
        let package = source.path().join("fixture.volumproject");
        write_package(
            &package,
            &manifest,
            &[ArchiveSource {
                path: payload,
                archive_path: manifest.files[0].archive_path.clone(),
            }],
        )
        .unwrap();
        let root_id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, ?, 'Managed', 'online', 'managed', ?, ?)")
            .bind(&root_id)
            .bind(library.path().to_string_lossy().to_string())
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(&state.pool)
            .await
            .unwrap();

        let project_id =
            import_managed_project_package(&package, &manifest, library.path(), &state)
                .await
                .unwrap();

        let relative_path: String = sqlx::query_scalar("SELECT a.relative_path FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ?")
            .bind(&project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let package_job: String = sqlx::query_scalar(
            "SELECT state FROM file_operation_jobs WHERE operation = 'import-package'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(library.path().join(relative_path).is_file());
        assert!(library
            .path()
            .join("Projects/Portable fixture/.volum/project.json")
            .is_file());
        assert_eq!(package_job, "complete");
    }

    #[test]
    fn rejects_a_package_with_a_tampered_payload_checksum() {
        let directory = test_directory("tampered");
        std::fs::create_dir_all(&directory).unwrap();
        let payload = directory.join("fixture.stl");
        let bytes = b"solid fixture\nendsolid fixture\n";
        std::fs::write(&payload, bytes).unwrap();
        let manifest = sample_manifest("0".repeat(64), bytes.len() as u64);
        let package = directory.join("tampered.volumproject");
        let mut writer = ZipWriter::new(File::create(&package).unwrap());
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        writer.start_file("checksums.json", options).unwrap();
        writer
            .write_all(
                &serde_json::to_vec(&BTreeMap::from([(
                    manifest.files[0].archive_path.clone(),
                    manifest.files[0].blake3.clone(),
                )]))
                .unwrap(),
            )
            .unwrap();
        writer
            .start_file(&manifest.files[0].archive_path, options)
            .unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap();
        assert!(validate_package(&package)
            .unwrap_err()
            .contains("checksum failed"));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_destination_symlinks() {
        let directory = test_directory("symlink");
        let root = directory.join("root");
        let outside = directory.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

        assert!(create_destination_without_symlinks(&root, "linked/project").is_err());
        assert!(!outside.join("project").exists());
        let _ = std::fs::remove_dir_all(directory);
    }
}
