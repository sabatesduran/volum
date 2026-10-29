use crate::{
    domain::{AssetMetadata, LibraryRoot},
    indexing, parsers, project_docs,
    state::AppState,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use walkdir::WalkDir;

type CommandResult<T> = Result<T, String>;

const SUPPORTED: &[&str] = &["stl", "3mf", "obj", "step", "stp"];
const PROJECTS_DIRECTORY: &str = "Projects";
const INBOX_DIRECTORY: &str = "Inbox";
const TRASH_DIRECTORY: &str = "Trash";
const STAGING_DIRECTORY: &str = ".staging";
const MARKER_DIRECTORY: &str = ".volum";
const MARKER_FILENAME: &str = "project.json";
const MAX_MARKER_BYTES: u64 = 1024 * 1024;
const MAX_PROJECT_FILES: usize = 2_000;
const MAX_PROJECT_NAME_CHARS: usize = 160;
pub(crate) const PROJECT_REVISION_CONFLICT: &str = "Project changed on another device";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedLibraryStatus {
    pub root: Option<LibraryRoot>,
    pub projects_path: Option<String>,
    pub inbox_path: Option<String>,
    pub trash_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAssetCandidate {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub filename: String,
    pub extension: String,
    pub byte_size: i64,
    pub missing: bool,
    pub storage_mode: String,
    pub root_name: String,
    pub relative_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateManagedProjectInput {
    pub name: String,
    #[serde(default)]
    pub file_paths: Vec<String>,
    #[serde(default)]
    pub asset_ids: Vec<String>,
    #[serde(default = "default_transfer_mode")]
    pub transfer_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectFilesInput {
    pub project_id: String,
    #[serde(default)]
    pub file_paths: Vec<String>,
    #[serde(default)]
    pub asset_ids: Vec<String>,
    #[serde(default = "default_transfer_mode")]
    pub transfer_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertProjectInput {
    pub project_id: String,
    #[serde(default = "default_transfer_mode")]
    pub transfer_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidationPreview {
    pub project_id: String,
    pub project_name: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub source_folders: Vec<String>,
    pub managed_folder_name: String,
    pub already_managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxImportResult {
    pub project_ids: Vec<String>,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct CompanionUploadImport {
    pub project_id: String,
    pub asset_id: String,
    pub root_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMarker {
    format_version: u32,
    project_id: String,
    name: String,
    files: Vec<ProjectMarkerFile>,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMarkerFile {
    asset_id: String,
    path: String,
    role: String,
}

#[derive(Debug, Clone)]
struct SourceFile {
    path: PathBuf,
    original_path: String,
    original_asset_id: Option<String>,
    source_model_id: Option<String>,
    source_root_id: Option<String>,
    source_relative_path: Option<String>,
    role: String,
}

#[derive(Debug)]
struct PreparedFile {
    source: SourceFile,
    staged_path: PathBuf,
    filename: String,
    extension: String,
    byte_size: i64,
    modified_at: String,
    partial_fingerprint: Option<String>,
    content_hash: String,
    parse_status: String,
    metadata_json: String,
    asset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileOperationPayload {
    project_id: String,
    mode: String,
    #[serde(default)]
    files: Vec<FileOperationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileOperationEntry {
    source: String,
    destination: String,
    content_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageOperationPayload {
    project_id: String,
    staging: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathOperationPayload {
    entity_id: String,
    source: String,
    destination: String,
    old_relative: String,
    new_relative: String,
}

#[derive(Debug)]
struct ManagedProjectRetirement {
    folder_id: String,
    source_directory: PathBuf,
    copied_photos: Vec<(PathBuf, PathBuf)>,
}

fn default_transfer_mode() -> String {
    "copy".into()
}

#[tauri::command]
pub async fn get_managed_library_status(
    state: State<'_, AppState>,
) -> CommandResult<ManagedLibraryStatus> {
    managed_library_status(&state.pool).await
}

#[tauri::command]
pub async fn configure_managed_library(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<LibraryRoot> {
    configure_managed_library_impl(path, &state, Some(app)).await
}

pub(crate) async fn configure_managed_library_impl(
    path: String,
    state: &AppState,
    app: Option<AppHandle>,
) -> CommandResult<LibraryRoot> {
    let _guard = state.file_operation_lock.lock().await;
    let selected = PathBuf::from(path);
    std::fs::create_dir_all(&selected)
        .map_err(|error| format!("Could not create the managed library: {error}"))?;
    let canonical = selected
        .canonicalize()
        .map_err(|_| "Choose a folder that is currently available".to_string())?;
    if !canonical.is_dir() {
        return Err("The managed library location must be a folder".into());
    }
    if state
        .data_dir
        .canonicalize()
        .is_ok_and(|data_dir| paths_overlap(&canonical, &data_dir))
    {
        return Err("Choose a managed library outside Volum’s app-data folder".into());
    }
    let normalized = canonical.to_string_lossy().to_string();
    let backup_paths: Vec<String> =
        sqlx::query_scalar("SELECT location FROM backup_destinations WHERE kind = 'folder'")
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    if backup_paths
        .into_iter()
        .map(PathBuf::from)
        .any(|backup| backup.starts_with(&canonical))
    {
        return Err("The managed library cannot contain a backup destination".into());
    }
    let roots = sqlx::query("SELECT id, path, root_kind, (SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = library_roots.id) model_count FROM library_roots WHERE status != 'removed'")
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let mut existing_id = None;
    for row in roots {
        let id: String = row.get("id");
        let root_path = PathBuf::from(row.get::<String, _>("path"));
        let kind: String = row.get("root_kind");
        if root_path == canonical {
            if kind == "managed" {
                existing_id = Some(id);
                continue;
            }
            if row.get::<i64, _>("model_count") != 0 {
                return Err("This folder is already a linked library. Choose an empty folder for Volum’s managed library.".into());
            }
            existing_id = Some(id);
            continue;
        }
        if kind == "managed" {
            return Err("Volum already has a managed library. Relocate it instead of creating a second one.".into());
        }
        if paths_overlap(&canonical, &root_path) {
            return Err(
                "The managed library cannot be inside, or contain, an existing linked library."
                    .into(),
            );
        }
    }
    ensure_layout(&canonical)?;
    write_library_marker(&canonical)?;
    let timestamp = now();
    let id = existing_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let display_name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Volum")
        .to_string();
    sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, ?, ?, 'online', 'managed', ?, ?) ON CONFLICT(id) DO UPDATE SET path = excluded.path, display_name = excluded.display_name, status = 'online', root_kind = 'managed', updated_at = excluded.updated_at")
        .bind(&id)
        .bind(&normalized)
        .bind(&display_name)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let root = root_by_id(&id, &state.pool).await?;
    if let Err(error) = indexing::start(id, state, app).await {
        log::warn!("Could not start the managed library watcher: {error}");
    }
    Ok(root)
}

#[tauri::command]
pub async fn list_library_assets(
    state: State<'_, AppState>,
) -> CommandResult<Vec<LibraryAssetCandidate>> {
    let rows = sqlx::query("SELECT a.id, m.id project_id, m.display_name project_name, a.filename, a.extension, a.byte_size, a.missing_since, m.storage_mode, r.display_name root_name, a.relative_path FROM assets a JOIN model_assets ma ON ma.asset_id = a.id JOIN models m ON m.id = ma.model_id JOIN library_roots r ON r.id = a.root_id WHERE m.trashed_at IS NULL ORDER BY lower(m.display_name), lower(a.filename)")
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| LibraryAssetCandidate {
            id: row.get("id"),
            project_id: row.get("project_id"),
            project_name: row.get("project_name"),
            filename: row.get("filename"),
            extension: row.get("extension"),
            byte_size: row.get("byte_size"),
            missing: row.get::<Option<String>, _>("missing_since").is_some(),
            storage_mode: row.get("storage_mode"),
            root_name: row.get("root_name"),
            relative_path: row.get("relative_path"),
        })
        .collect())
}

#[tauri::command]
pub async fn import_managed_inbox(
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<InboxImportResult> {
    let result = import_managed_inbox_impl(&state).await?;
    if !result.project_ids.is_empty() {
        let _ = app.emit("models-changed", ());
    }
    Ok(result)
}

pub(crate) async fn import_managed_inbox_impl(
    state: &AppState,
) -> CommandResult<InboxImportResult> {
    let _guard = state.file_operation_lock.lock().await;
    let (_, root_path, _) = managed_root(&state.pool).await?;
    let inbox = root_path.join(INBOX_DIRECTORY);
    let candidates = tokio::task::spawn_blocking(move || discover_inbox_projects(&inbox))
        .await
        .map_err(|error| error.to_string())??;
    let mut project_ids = Vec::new();
    let mut skipped = 0;
    let mut errors = Vec::new();
    for (name, paths) in candidates {
        if paths.is_empty() {
            skipped += 1;
            continue;
        }
        let input = CreateManagedProjectInput {
            name,
            file_paths: paths,
            asset_ids: Vec::new(),
            transfer_mode: "move".into(),
        };
        match create_managed_project_internal(&input, state).await {
            Ok((project_id, _)) => project_ids.push(project_id),
            Err(error) => errors.push(error),
        }
    }
    Ok(InboxImportResult {
        project_ids,
        skipped,
        errors,
    })
}

#[tauri::command]
pub async fn create_managed_project(
    input: CreateManagedProjectInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<String> {
    validate_transfer_mode(&input.transfer_mode)?;
    validate_project_name(&input.name)?;
    validate_source_count(input.file_paths.len() + input.asset_ids.len())?;
    let _guard = state.file_operation_lock.lock().await;
    if input.transfer_mode == "link" {
        let project_id = create_linked_project(&input, &state).await?;
        let _ = app.emit("models-changed", ());
        return Ok(project_id);
    }
    let (project_id, root_id) = create_managed_project_internal(&input, &state).await?;
    let _ = app.emit("models-changed", &root_id);
    Ok(project_id)
}

pub(crate) async fn move_paths_into_managed_project(
    name: &str,
    file_paths: Vec<String>,
    state: &AppState,
) -> CommandResult<String> {
    let _guard = state.file_operation_lock.lock().await;
    let input = CreateManagedProjectInput {
        name: name.to_string(),
        file_paths,
        asset_ids: Vec::new(),
        transfer_mode: "move".into(),
    };
    validate_project_name(&input.name)?;
    validate_source_count(input.file_paths.len())?;
    create_managed_project_internal(&input, state)
        .await
        .map(|(project_id, _)| project_id)
}

pub(crate) async fn create_managed_project_internal(
    input: &CreateManagedProjectInput,
    state: &AppState,
) -> CommandResult<(String, String)> {
    create_managed_project_internal_with_id(input, Uuid::new_v4().to_string(), state).await
}

pub(crate) async fn create_managed_project_internal_with_id(
    input: &CreateManagedProjectInput,
    project_id: String,
    state: &AppState,
) -> CommandResult<(String, String)> {
    let (root_id, root_path, root_name) = managed_root(&state.pool).await?;
    ensure_layout(&root_path)?;
    if Uuid::parse_str(&project_id).is_err() {
        return Err("Managed project identity is invalid".into());
    }
    let folder_name = unique_project_folder(&root_path.join(PROJECTS_DIRECTORY), &input.name)?;
    let relative_folder = format!("{PROJECTS_DIRECTORY}/{folder_name}");
    let project_path = root_path.join(&relative_folder);
    std::fs::create_dir(&project_path)
        .map_err(|error| format!("Could not create the project folder: {error}"))?;
    let folder_id =
        ensure_folder_record(&root_id, &relative_folder, &root_name, &state.pool).await?;
    let timestamp = now();
    if let Err(error) = sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, bundle_mode, storage_mode, added_at, updated_at) VALUES (?, ?, ?, ?, '', 'manual', 'managed', ?, ?)")
        .bind(&project_id)
        .bind(&folder_id)
        .bind(input.name.trim())
        .bind(format!("manual:{project_id}"))
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)
    {
        let _ = std::fs::remove_dir(&project_path);
        return Err(error);
    }
    if let Err(error) = write_project_marker(&project_id, &state.pool).await {
        let _ = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(&project_id)
            .execute(&state.pool)
            .await;
        let _ = std::fs::remove_dir_all(&project_path);
        return Err(error);
    }
    if !input.file_paths.is_empty() || !input.asset_ids.is_empty() {
        if let Err(error) = import_into_project(
            &project_id,
            &input.file_paths,
            &input.asset_ids,
            &input.transfer_mode,
            input.transfer_mode == "move",
            state,
        )
        .await
        {
            let _ = sqlx::query("DELETE FROM models WHERE id = ?")
                .bind(&project_id)
                .execute(&state.pool)
                .await;
            let _ = std::fs::remove_dir_all(&project_path);
            return Err(error);
        }
    }
    indexing::refresh_search(&project_id, &state.pool).await?;
    Ok((project_id, root_id))
}

#[tauri::command]
pub async fn add_files_to_project(
    input: AddProjectFilesInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<Vec<String>> {
    validate_transfer_mode(&input.transfer_mode)?;
    validate_source_count(input.file_paths.len() + input.asset_ids.len())?;
    let _guard = state.file_operation_lock.lock().await;
    if input.transfer_mode == "link" {
        return add_linked_files_to_project(&input, &state).await;
    }
    let ids = import_into_project(
        &input.project_id,
        &input.file_paths,
        &input.asset_ids,
        &input.transfer_mode,
        input.transfer_mode == "move",
        &state,
    )
    .await?;
    let root_id: String = sqlx::query_scalar(
        "SELECT f.root_id FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?",
    )
    .bind(&input.project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    let _ = app.emit("models-changed", &root_id);
    Ok(ids)
}

pub(crate) async fn import_companion_upload(
    project_id: Option<&str>,
    project_name: Option<&str>,
    upload_path: &Path,
    state: &AppState,
) -> CommandResult<CompanionUploadImport> {
    validate_source_file(upload_path)?;
    let _guard = state.file_operation_lock.lock().await;
    let upload_path = upload_path.to_string_lossy().to_string();
    let (project_id, root_id, asset_id) = if let Some(project_id) = project_id {
        if Uuid::parse_str(project_id).is_err() {
            return Err("Project identity is invalid".into());
        }
        let row = sqlx::query("SELECT m.storage_mode, m.trashed_at, f.root_id FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "Project not found".to_string())?;
        if row.get::<Option<String>, _>("trashed_at").is_some() {
            return Err("Files cannot be uploaded to a project in Trash".into());
        }
        if row.get::<String, _>("storage_mode") != "managed" {
            return Err("Files can only be uploaded to a managed project".into());
        }
        let asset_ids = import_into_project(
            project_id,
            std::slice::from_ref(&upload_path),
            &[],
            "move",
            false,
            state,
        )
        .await?;
        let asset_id = asset_ids
            .into_iter()
            .next()
            .ok_or_else(|| "The uploaded file was not imported".to_string())?;
        (project_id.to_string(), row.get("root_id"), asset_id)
    } else {
        let project_name = project_name.ok_or_else(|| "Enter a project name".to_string())?;
        validate_project_name(project_name)?;
        let input = CreateManagedProjectInput {
            name: project_name.to_string(),
            file_paths: vec![upload_path],
            asset_ids: Vec::new(),
            transfer_mode: "move".into(),
        };
        let (project_id, root_id) = create_managed_project_internal(&input, state).await?;
        let asset_id: String = sqlx::query_scalar(
            "SELECT asset_id FROM model_assets WHERE model_id = ? ORDER BY sort_order LIMIT 1",
        )
        .bind(&project_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
        (project_id, root_id, asset_id)
    };
    Ok(CompanionUploadImport {
        project_id,
        asset_id,
        root_id,
    })
}

#[tauri::command]
pub async fn rename_project(
    project_id: String,
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    rename_project_impl(&project_id, &name, None, state.inner(), Some(&app)).await
}

pub(crate) async fn rename_project_if_revision(
    project_id: &str,
    name: &str,
    expected_revision: &str,
    state: &AppState,
    app: Option<&AppHandle>,
) -> CommandResult<()> {
    rename_project_impl(project_id, name, Some(expected_revision), state, app).await
}

async fn rename_project_impl(
    project_id: &str,
    name: &str,
    expected_revision: Option<&str>,
    state: &AppState,
    app: Option<&AppHandle>,
) -> CommandResult<()> {
    validate_project_name(name)?;
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT m.storage_mode, m.folder_id, m.updated_at, f.relative_path, f.root_id, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ? AND m.trashed_at IS NULL")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if expected_revision.is_some_and(|expected| expected != row.get::<String, _>("updated_at")) {
        return Err(PROJECT_REVISION_CONFLICT.into());
    }
    let storage_mode: String = row.get("storage_mode");
    let root_id: String = row.get("root_id");
    if storage_mode != "managed" {
        sqlx::query("UPDATE models SET display_name = ?, grouping_key = 'manual:' || id, bundle_mode = 'manual', updated_at = ? WHERE id = ?")
            .bind(name.trim())
            .bind(now())
            .bind(project_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(project_id, &state.pool).await?;
        if let Some(app) = app {
            let _ = app.emit("models-changed", &root_id);
        }
        return Ok(());
    }
    let old_relative: String = row.get("relative_path");
    let old_path = PathBuf::from(row.get::<String, _>("root_path")).join(&old_relative);
    let parent = old_path
        .parent()
        .ok_or_else(|| "The managed project folder is invalid".to_string())?;
    let folder_component = safe_component(name.trim(), "Project")?;
    let new_path = parent.join(&folder_component);
    if !same_path_case_insensitive(&old_path, &new_path)
        && path_exists_case_insensitive(parent, &folder_component)?
    {
        return Err("A project folder with that name already exists".into());
    }
    let new_relative = format!("{PROJECTS_DIRECTORY}/{folder_component}");
    let job_id = begin_path_operation(
        "rename-project",
        &PathOperationPayload {
            entity_id: project_id.to_string(),
            source: old_path.to_string_lossy().to_string(),
            destination: new_path.to_string_lossy().to_string(),
            old_relative: old_relative.clone(),
            new_relative: new_relative.clone(),
        },
        &state.pool,
    )
    .await?;
    if let Err(error) = rename_case_safe(&old_path, &new_path) {
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    if let Err(error) = set_operation_state(&job_id, "committing", None, &state.pool).await {
        let _ = rename_case_safe(&new_path, &old_path);
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    let result = rename_project_records(
        project_id,
        row.get::<String, _>("folder_id"),
        &old_relative,
        &new_relative,
        name.trim(),
        &state.pool,
    )
    .await;
    if let Err(error) = result {
        let _ = rename_case_safe(&new_path, &old_path);
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    let _ = set_operation_state(&job_id, "complete", None, &state.pool).await;
    if let Err(error) = write_project_marker(project_id, &state.pool).await {
        log::warn!("Could not refresh sidecar after renaming project {project_id}: {error}");
    }
    if let Err(error) = indexing::refresh_search(project_id, &state.pool).await {
        log::warn!("Could not refresh search after renaming project {project_id}: {error}");
    }
    if let Some(app) = app {
        let _ = app.emit("models-changed", &root_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn rename_project_file(
    asset_id: String,
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT a.filename, a.extension, a.relative_path, a.root_id, r.path root_path, ma.model_id, m.storage_mode FROM assets a JOIN library_roots r ON r.id = a.root_id JOIN model_assets ma ON ma.asset_id = a.id JOIN models m ON m.id = ma.model_id WHERE a.id = ?")
        .bind(&asset_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project file not found".to_string())?;
    let extension: String = row.get("extension");
    let filename = validate_renamed_file(&name, &extension)?;
    let root_path = PathBuf::from(row.get::<String, _>("root_path"));
    let old_relative: String = row.get("relative_path");
    let old_path = root_path.join(&old_relative);
    let parent = old_path
        .parent()
        .ok_or_else(|| "The project file path is invalid".to_string())?;
    if path_exists_case_insensitive(parent, &filename)?
        && !row
            .get::<String, _>("filename")
            .eq_ignore_ascii_case(&filename)
    {
        return Err("A file with that name already exists".into());
    }
    let parent_relative = Path::new(&old_relative)
        .parent()
        .filter(|path| *path != Path::new(""))
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    let new_relative = parent_relative
        .map(|parent| format!("{parent}/{filename}"))
        .unwrap_or_else(|| filename.clone());
    let new_path = parent.join(&filename);
    let job_id = begin_path_operation(
        "rename-asset",
        &PathOperationPayload {
            entity_id: asset_id.clone(),
            source: old_path.to_string_lossy().to_string(),
            destination: new_path.to_string_lossy().to_string(),
            old_relative: old_relative.clone(),
            new_relative: new_relative.clone(),
        },
        &state.pool,
    )
    .await?;
    if let Err(error) = rename_case_safe(&old_path, &new_path) {
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    if let Err(error) = set_operation_state(&job_id, "committing", None, &state.pool).await {
        let _ = rename_case_safe(&new_path, &old_path);
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    let model_id: String = row.get("model_id");
    let result = sqlx::query("UPDATE assets SET filename = ?, relative_path = ?, modified_at = ?, missing_since = NULL WHERE id = ?")
        .bind(&filename)
        .bind(&new_relative)
        .bind(now())
        .bind(&asset_id)
        .execute(&state.pool)
        .await
        .map_err(db_error);
    if let Err(error) = result {
        let _ = rename_case_safe(&new_path, &old_path);
        let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
        return Err(error);
    }
    if let Err(error) = sqlx::query("UPDATE models SET grouping_key = 'manual:' || id, bundle_mode = 'manual', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&model_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)
    {
        log::warn!("Could not update project grouping after renaming file {asset_id}: {error}");
    }
    let _ = set_operation_state(&job_id, "complete", None, &state.pool).await;
    let storage: String = row.get("storage_mode");
    if storage == "managed" {
        if let Err(error) = write_project_marker(&model_id, &state.pool).await {
            log::warn!("Could not refresh sidecar after renaming file {asset_id}: {error}");
        }
    }
    if let Err(error) = indexing::refresh_search(&model_id, &state.pool).await {
        log::warn!("Could not refresh search after renaming file {asset_id}: {error}");
    }
    let _ = app.emit("models-changed", row.get::<String, _>("root_id"));
    Ok(())
}

#[tauri::command]
pub async fn convert_project_to_managed(
    input: ConvertProjectInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    validate_transfer_mode(&input.transfer_mode)?;
    if input.transfer_mode == "link" {
        return Err("Consolidation must copy or move files into the managed library".into());
    }
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT m.display_name, m.storage_mode, m.folder_id, m.bundle_mode, m.grouping_key, f.root_id old_root_id, f.relative_path old_folder FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?")
        .bind(&input.project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if row.get::<String, _>("storage_mode") == "managed" {
        return Ok(());
    }
    let asset_ids: Vec<String> = sqlx::query_scalar(
        "SELECT asset_id FROM model_assets WHERE model_id = ? ORDER BY sort_order",
    )
    .bind(&input.project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;
    validate_source_count(asset_ids.len())?;
    let (root_id, root_path, root_name) = managed_root(&state.pool).await?;
    let project_name: String = row.get("display_name");
    let folder_name = unique_project_folder(&root_path.join(PROJECTS_DIRECTORY), &project_name)?;
    let relative_folder = format!("{PROJECTS_DIRECTORY}/{folder_name}");
    let project_path = root_path.join(&relative_folder);
    std::fs::create_dir(&project_path)
        .map_err(|error| format!("Could not create the managed project folder: {error}"))?;
    let folder_id =
        ensure_folder_record(&root_id, &relative_folder, &root_name, &state.pool).await?;
    let old_folder_id: String = row.get("folder_id");
    sqlx::query("UPDATE models SET folder_id = ?, storage_mode = 'managed', bundle_mode = 'manual', grouping_key = 'manual:' || id, updated_at = ? WHERE id = ?")
        .bind(&folder_id)
        .bind(now())
        .bind(&input.project_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let photo_migration = match project_docs::move_project_photo_originals_to_managed(
        &input.project_id,
        &state,
    )
    .await
    {
        Ok(migration) => migration,
        Err(error) => {
            rollback_project_conversion_record(
                &input.project_id,
                &old_folder_id,
                &row.get::<String, _>("bundle_mode"),
                &row.get::<String, _>("grouping_key"),
                &folder_id,
                &project_path,
                &state,
            )
            .await;
            return Err(error);
        }
    };
    let result = import_into_project(
        &input.project_id,
        &[],
        &asset_ids,
        &input.transfer_mode,
        true,
        &state,
    )
    .await;
    if let Err(error) = result {
        if let Err(rollback_error) =
            project_docs::rollback_managed_photo_migration(photo_migration, &state).await
        {
            return Err(format!(
                "{error}. Photo rollback also failed; the managed project was preserved for recovery: {rollback_error}"
            ));
        }
        rollback_project_conversion_record(
            &input.project_id,
            &old_folder_id,
            &row.get::<String, _>("bundle_mode"),
            &row.get::<String, _>("grouping_key"),
            &folder_id,
            &project_path,
            &state,
        )
        .await;
        return Err(error);
    }
    project_docs::finish_managed_photo_migration(photo_migration);
    if let Err(error) = write_project_marker(&input.project_id, &state.pool).await {
        log::warn!(
            "Could not refresh sidecar after converting project {}: {error}",
            input.project_id
        );
    }
    let _ = app.emit("models-changed", &root_id);
    let _ = app.emit("models-changed", row.get::<String, _>("old_root_id"));
    Ok(())
}

#[tauri::command]
pub async fn preview_project_consolidation(
    project_id: String,
    state: State<'_, AppState>,
) -> CommandResult<ConsolidationPreview> {
    let row = sqlx::query("SELECT m.display_name, m.storage_mode, COUNT(a.id) file_count, COALESCE(SUM(a.byte_size), 0) total_bytes FROM models m LEFT JOIN model_assets ma ON ma.model_id = m.id LEFT JOIN assets a ON a.id = ma.asset_id WHERE m.id = ? GROUP BY m.id")
        .bind(&project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    let project_name: String = row.get("display_name");
    let source_folders = sqlx::query("SELECT DISTINCT r.path root_path, f.relative_path FROM model_assets ma JOIN assets a ON a.id = ma.asset_id JOIN folders f ON f.id = a.folder_id JOIN library_roots r ON r.id = a.root_id WHERE ma.model_id = ? ORDER BY r.path, f.relative_path")
        .bind(&project_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|source| {
            let root = PathBuf::from(source.get::<String, _>("root_path"));
            let relative: String = source.get("relative_path");
            if relative.is_empty() {
                root.to_string_lossy().to_string()
            } else {
                root.join(relative).to_string_lossy().to_string()
            }
        })
        .collect();
    Ok(ConsolidationPreview {
        project_id,
        managed_folder_name: safe_component(&project_name, "Project")?,
        project_name,
        file_count: row.get("file_count"),
        total_bytes: row.get("total_bytes"),
        source_folders,
        already_managed: row.get::<String, _>("storage_mode") == "managed",
    })
}

#[tauri::command]
pub async fn trash_project(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    trash_project_impl(&project_id, &state, Some(&app)).await
}

pub(crate) async fn trash_project_impl(
    project_id: &str,
    state: &AppState,
    app: Option<&AppHandle>,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT m.storage_mode, m.trashed_at, m.folder_id, f.relative_path, f.root_id, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if row.get::<Option<String>, _>("trashed_at").is_some() {
        return Ok(());
    }
    let root_id: String = row.get("root_id");
    if row.get::<String, _>("storage_mode") == "managed" {
        let old_relative: String = row.get("relative_path");
        let root_path = PathBuf::from(row.get::<String, _>("root_path"));
        let old_path = root_path.join(&old_relative);
        let original_name = old_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Project");
        let trash_name = unique_project_folder(&root_path.join(TRASH_DIRECTORY), original_name)?;
        let new_relative = format!("{TRASH_DIRECTORY}/{trash_name}");
        let new_path = root_path.join(&new_relative);
        let job_id = begin_path_operation(
            "trash-project",
            &PathOperationPayload {
                entity_id: project_id.to_string(),
                source: old_path.to_string_lossy().to_string(),
                destination: new_path.to_string_lossy().to_string(),
                old_relative: old_relative.clone(),
                new_relative: new_relative.clone(),
            },
            &state.pool,
        )
        .await?;
        if let Err(error) = std::fs::rename(&old_path, &new_path) {
            let message = format!("Could not move the project to Volum Trash: {error}");
            let _ = set_operation_state(&job_id, "failed", Some(&message), &state.pool).await;
            return Err(message);
        }
        if let Err(error) = set_operation_state(&job_id, "committing", None, &state.pool).await {
            let _ = std::fs::rename(&new_path, &old_path);
            let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
            return Err(error);
        }
        if let Err(error) = update_project_folder_paths(
            project_id,
            &row.get::<String, _>("folder_id"),
            &old_relative,
            &new_relative,
            Some(&old_relative),
            Some(now()),
            &state.pool,
        )
        .await
        {
            let _ = std::fs::rename(&new_path, &old_path);
            let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
            return Err(error);
        }
        let _ = set_operation_state(&job_id, "complete", None, &state.pool).await;
    } else {
        sqlx::query("UPDATE models SET trashed_at = ?, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(now())
            .bind(project_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    if let Some(app) = app {
        let _ = app.emit("models-changed", &root_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn restore_project_from_trash(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    restore_project_from_trash_impl(&project_id, &state, Some(&app)).await
}

pub(crate) async fn restore_project_from_trash_impl(
    project_id: &str,
    state: &AppState,
    app: Option<&AppHandle>,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT m.display_name, m.storage_mode, m.trashed_at, m.trash_original_relative_path, m.folder_id, f.relative_path, f.root_id, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if row.get::<Option<String>, _>("trashed_at").is_none() {
        return Ok(());
    }
    let root_id: String = row.get("root_id");
    if row.get::<String, _>("storage_mode") == "managed" {
        let root_path = PathBuf::from(row.get::<String, _>("root_path"));
        let old_relative: String = row.get("relative_path");
        let old_path = root_path.join(&old_relative);
        let requested = row
            .get::<Option<String>, _>("trash_original_relative_path")
            .and_then(|relative| {
                Path::new(&relative)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| row.get::<String, _>("display_name"));
        let folder_name = unique_project_folder(&root_path.join(PROJECTS_DIRECTORY), &requested)?;
        let new_relative = format!("{PROJECTS_DIRECTORY}/{folder_name}");
        let new_path = root_path.join(&new_relative);
        let job_id = begin_path_operation(
            "restore-project",
            &PathOperationPayload {
                entity_id: project_id.to_string(),
                source: old_path.to_string_lossy().to_string(),
                destination: new_path.to_string_lossy().to_string(),
                old_relative: old_relative.clone(),
                new_relative: new_relative.clone(),
            },
            &state.pool,
        )
        .await?;
        if let Err(error) = std::fs::rename(&old_path, &new_path) {
            let message = format!("Could not restore the project folder: {error}");
            let _ = set_operation_state(&job_id, "failed", Some(&message), &state.pool).await;
            return Err(message);
        }
        if let Err(error) = set_operation_state(&job_id, "committing", None, &state.pool).await {
            let _ = std::fs::rename(&new_path, &old_path);
            let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
            return Err(error);
        }
        if let Err(error) = update_project_folder_paths(
            project_id,
            &row.get::<String, _>("folder_id"),
            &old_relative,
            &new_relative,
            None,
            None,
            &state.pool,
        )
        .await
        {
            let _ = std::fs::rename(&new_path, &old_path);
            let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
            return Err(error);
        }
        let _ = set_operation_state(&job_id, "complete", None, &state.pool).await;
        if let Err(error) = write_project_marker(project_id, &state.pool).await {
            log::warn!("Could not refresh sidecar after restoring project {project_id}: {error}");
        }
    } else {
        sqlx::query("UPDATE models SET trashed_at = NULL, trash_original_relative_path = NULL, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(project_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    if let Err(error) = indexing::refresh_search(project_id, &state.pool).await {
        log::warn!("Could not refresh search after restoring project {project_id}: {error}");
    }
    if let Some(app) = app {
        let _ = app.emit("models-changed", &root_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_project_permanently(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    delete_project_permanently_impl(&project_id, &state, Some(&app)).await
}

pub(crate) async fn delete_project_permanently_impl(
    project_id: &str,
    state: &AppState,
    app: Option<&AppHandle>,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    let row = sqlx::query("SELECT m.storage_mode, m.trashed_at, f.relative_path, f.root_id, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if row.get::<Option<String>, _>("trashed_at").is_none() {
        return Err("Move the project to Trash before deleting it permanently".into());
    }
    let root_id: String = row.get("root_id");
    let mut staged_directory = None;
    let mut operation_job = None;
    if row.get::<String, _>("storage_mode") == "managed" {
        let root_path = PathBuf::from(row.get::<String, _>("root_path"));
        let relative_path: String = row.get("relative_path");
        let project_path = root_path.join(&relative_path);
        let staging = root_path
            .join(STAGING_DIRECTORY)
            .join(format!("delete-{}", Uuid::new_v4()));
        let job_id = begin_path_operation(
            "delete-project",
            &PathOperationPayload {
                entity_id: project_id.to_string(),
                source: project_path.to_string_lossy().to_string(),
                destination: staging.to_string_lossy().to_string(),
                old_relative: relative_path.clone(),
                new_relative: format!(
                    "{STAGING_DIRECTORY}/{}",
                    staging
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("delete")
                ),
            },
            &state.pool,
        )
        .await?;
        if let Err(error) = std::fs::rename(&project_path, &staging) {
            let message = format!("Could not stage the project for deletion: {error}");
            let _ = set_operation_state(&job_id, "failed", Some(&message), &state.pool).await;
            return Err(message);
        }
        if let Err(error) = set_operation_state(&job_id, "committing", None, &state.pool).await {
            let _ = std::fs::rename(&staging, &project_path);
            let _ = set_operation_state(&job_id, "failed", Some(&error), &state.pool).await;
            return Err(error);
        }
        operation_job = Some(job_id);
        staged_directory = Some((project_path, staging));
    }
    let photo_rows =
        sqlx::query("SELECT media_path, thumbnail_path FROM project_photos WHERE model_id = ?")
            .bind(project_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let asset_ids: Vec<String> =
        sqlx::query_scalar("SELECT asset_id FROM model_assets WHERE model_id = ?")
            .bind(project_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    sqlx::query("DELETE FROM models WHERE id = ?")
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for asset_id in asset_ids {
        sqlx::query("DELETE FROM assets WHERE id = ? AND NOT EXISTS (SELECT 1 FROM model_assets WHERE asset_id = ?)")
            .bind(&asset_id)
            .bind(&asset_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    if let Err(error) = transaction.commit().await.map_err(db_error) {
        if let Some((original, staging)) = &staged_directory {
            let _ = std::fs::rename(staging, original);
        }
        if let Some(job_id) = &operation_job {
            let _ = set_operation_state(job_id, "failed", Some(&error), &state.pool).await;
        }
        return Err(error);
    }
    if let Some(job_id) = &operation_job {
        let _ = set_operation_state(job_id, "cleanup", None, &state.pool).await;
    }
    let mut path_cleanup_complete = true;
    if let Some((_, staging)) = staged_directory {
        if let Err(error) = std::fs::remove_dir_all(&staging) {
            path_cleanup_complete = false;
            log::warn!(
                "Could not finish deleting staged project {}: {error}",
                staging.display()
            );
        }
    }
    if path_cleanup_complete {
        if let Some(job_id) = &operation_job {
            let _ = set_operation_state(job_id, "complete", None, &state.pool).await;
        }
    }
    for row in photo_rows {
        for relative in [
            row.get::<String, _>("media_path"),
            row.get::<String, _>("thumbnail_path"),
        ] {
            if let Ok(path) = safe_app_media_path(&state.data_dir, &relative) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    if let Some(app) = app {
        let _ = app.emit("models-changed", &root_id);
    }
    Ok(())
}

pub async fn recover_pending_operations(pool: &SqlitePool, data_dir: &Path) -> CommandResult<()> {
    let restored_marker = data_dir.join("managed-restore-pending");
    if restored_marker.is_file() {
        let restored_path = std::fs::read_to_string(&restored_marker)
            .map_err(|error| format!("Could not read the managed restore marker: {error}"))?;
        let restored = PathBuf::from(restored_path);
        ensure_layout(&restored)?;
        write_library_marker(&restored)?;
        sqlx::query("UPDATE library_roots SET path = ?, display_name = ?, status = 'online', updated_at = ? WHERE root_kind = 'managed'")
            .bind(restored.to_string_lossy().to_string())
            .bind(
                restored
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Managed Library"),
            )
            .bind(now())
            .execute(pool)
            .await
            .map_err(db_error)?;
        std::fs::remove_file(&restored_marker)
            .map_err(|error| format!("Could not finish the managed restore: {error}"))?;
    }
    let roots: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM library_roots WHERE root_kind = 'managed' AND status != 'removed'",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let root_paths = roots.iter().map(PathBuf::from).collect::<Vec<_>>();
    let all_root_paths: Vec<PathBuf> =
        sqlx::query_scalar::<_, String>("SELECT path FROM library_roots WHERE status != 'removed'")
            .fetch_all(pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(PathBuf::from)
            .collect();
    let jobs = sqlx::query("SELECT id, operation, state, payload_json FROM file_operation_jobs WHERE state IN ('pending', 'staging', 'committing', 'cleanup')")
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    for job in jobs {
        let id: String = job.get("id");
        let operation: String = job.get("operation");
        let state: String = job.get("state");
        let payload_json: String = job.get("payload_json");
        let result: CommandResult<bool> = if operation == "import-package" {
            match serde_json::from_str::<PackageOperationPayload>(&payload_json) {
                Ok(payload) => rollback_interrupted_package(&payload, &root_paths, pool, data_dir)
                    .await
                    .map(|_| false),
                Err(error) => Err(format!(
                    "Could not read interrupted package import: {error}"
                )),
            }
        } else if matches!(
            operation.as_str(),
            "rename-project"
                | "rename-asset"
                | "trash-project"
                | "restore-project"
                | "delete-project"
        ) {
            match serde_json::from_str::<PathOperationPayload>(&payload_json) {
                Ok(payload) => {
                    recover_path_operation(&operation, &payload, &all_root_paths, pool).await
                }
                Err(error) => Err(format!(
                    "Could not read interrupted path operation: {error}"
                )),
            }
        } else {
            let payload = serde_json::from_str::<FileOperationPayload>(&payload_json);
            match (state.as_str(), payload) {
                ("cleanup", Ok(payload)) if payload.mode == "move" => {
                    cleanup_moved_sources(&payload).map(|_| true)
                }
                ("committing", Ok(payload)) => {
                    rollback_uncommitted_files(&payload, &root_paths).map(|_| false)
                }
                ("pending" | "staging", _) => Ok(false),
                (_, Err(error)) => Err(format!("Could not read interrupted operation: {error}")),
                _ => Err("Interrupted file operation has an unsupported recovery state".into()),
            }
        };
        let (next_state, error) = match result {
            Ok(true) => ("complete", None),
            Ok(false) => (
                "failed",
                Some(if operation == "import-package" {
                    "Interrupted package import was rolled back".to_string()
                } else if operation == "delete-project" {
                    "Interrupted deletion was rolled back".to_string()
                } else {
                    "Interrupted before database commit".to_string()
                }),
            ),
            Err(error) => ("failed", Some(error)),
        };
        sqlx::query(
            "UPDATE file_operation_jobs SET state = ?, error = ?, updated_at = ? WHERE id = ?",
        )
        .bind(next_state)
        .bind(error)
        .bind(now())
        .bind(&id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    }
    for root in &root_paths {
        let staging = root.join(STAGING_DIRECTORY);
        if !staging.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&staging).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.parent() == Some(staging.as_path()) {
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(path);
                } else {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
    Ok(())
}

pub async fn write_project_marker(project_id: &str, pool: &SqlitePool) -> CommandResult<()> {
    let row = sqlx::query("SELECT m.display_name, m.storage_mode, f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if row.get::<String, _>("storage_mode") != "managed" {
        return Ok(());
    }
    let files = sqlx::query("SELECT a.id, a.relative_path, ma.role FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? ORDER BY ma.sort_order, lower(a.filename)")
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let folder_relative: String = row.get("relative_path");
    let prefix = format!("{folder_relative}/");
    let marker = ProjectMarker {
        format_version: 1,
        project_id: project_id.to_string(),
        name: row.get("display_name"),
        files: files
            .into_iter()
            .filter_map(|file| {
                let relative: String = file.get("relative_path");
                relative
                    .strip_prefix(&prefix)
                    .map(|path| ProjectMarkerFile {
                        asset_id: file.get("id"),
                        path: path.to_string(),
                        role: file.get("role"),
                    })
            })
            .collect(),
        updated_at: now(),
    };
    let directory = PathBuf::from(row.get::<String, _>("root_path"))
        .join(folder_relative)
        .join(MARKER_DIRECTORY);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create project metadata folder: {error}"))?;
    let destination = directory.join(MARKER_FILENAME);
    let temporary = directory.join(format!("{MARKER_FILENAME}.{}.tmp", Uuid::new_v4()));
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("Could not save project identity: {error}"))?;
    replace_file_atomically(&temporary, &destination)
        .map_err(|error| format!("Could not finish saving project identity: {error}"))?;
    Ok(())
}

pub(crate) async fn reconcile_managed_sidecars(
    root_id: &str,
    root_path: &Path,
    pool: &SqlitePool,
) -> CommandResult<()> {
    let projects = root_path.join(PROJECTS_DIRECTORY);
    if !projects.is_dir() {
        return Ok(());
    }
    let canonical_root = root_path
        .canonicalize()
        .map_err(|_| "The managed library is unavailable".to_string())?;
    let canonical_projects = projects
        .canonicalize()
        .map_err(|_| "The managed Projects folder is unavailable".to_string())?;
    if !canonical_projects.starts_with(&canonical_root) {
        return Err("The managed Projects folder escapes the managed library".into());
    }
    let mut candidates = Vec::new();
    let mut identity_counts = HashMap::<String, usize>::new();
    for entry in std::fs::read_dir(&projects)
        .map_err(|error| format!("Could not inspect managed projects: {error}"))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!("Could not inspect a managed project entry: {error}");
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                log::warn!("Could not inspect {}: {error}", entry.path().display());
                continue;
            }
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let project_directory = entry.path();
        let marker_path = project_directory
            .join(MARKER_DIRECTORY)
            .join(MARKER_FILENAME);
        if let Err(error) = restore_atomic_file_backup(&marker_path) {
            log::warn!(
                "Could not recover managed sidecar backup {}: {error}",
                marker_path.display()
            );
        }
        if !marker_path.is_file() {
            continue;
        }
        let marker_metadata = match std::fs::symlink_metadata(&marker_path) {
            Ok(metadata)
                if metadata.file_type().is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.len() <= MAX_MARKER_BYTES =>
            {
                metadata
            }
            _ => continue,
        };
        if marker_metadata.len() == 0 {
            continue;
        }
        let marker = std::fs::read(&marker_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ProjectMarker>(&bytes).ok());
        let Some(marker) = marker else {
            log::warn!(
                "Could not parse managed project sidecar {}",
                marker_path.display()
            );
            continue;
        };
        if marker.format_version != 1 || Uuid::parse_str(&marker.project_id).is_err() {
            log::warn!(
                "Ignoring unsupported managed project sidecar {}",
                marker_path.display()
            );
            continue;
        }
        *identity_counts
            .entry(marker.project_id.clone())
            .or_default() += 1;
        candidates.push((project_directory, marker_path, marker.project_id));
    }
    for (project_directory, marker_path, project_id) in candidates {
        if identity_counts.get(&project_id).copied().unwrap_or(0) != 1 {
            log::warn!(
                "Refusing ambiguous managed project identity {project_id}; more than one sidecar claims it"
            );
            continue;
        }
        if let Err(error) = recover_project_sidecar(
            root_id,
            &canonical_root,
            &canonical_projects,
            project_directory.as_path(),
            marker_path.as_path(),
            pool,
        )
        .await
        {
            log::warn!(
                "Could not recover managed project identity from {}: {error}",
                marker_path.display()
            );
        }
    }
    Ok(())
}

async fn recover_project_sidecar(
    root_id: &str,
    canonical_root: &Path,
    canonical_projects: &Path,
    project_directory: &Path,
    marker_path: &Path,
    pool: &SqlitePool,
) -> CommandResult<()> {
    let marker_metadata = std::fs::symlink_metadata(marker_path)
        .map_err(|error| format!("Could not inspect project sidecar: {error}"))?;
    if !marker_metadata.file_type().is_file()
        || marker_metadata.file_type().is_symlink()
        || marker_metadata.len() > MAX_MARKER_BYTES
    {
        return Err("Project sidecar is not a safe regular file".into());
    }
    let canonical_project = project_directory
        .canonicalize()
        .map_err(|_| "Managed project folder is unavailable".to_string())?;
    if canonical_project.parent() != Some(canonical_projects) {
        return Err("Managed project sidecar is outside Projects".into());
    }
    let marker: ProjectMarker = serde_json::from_slice(
        &std::fs::read(marker_path)
            .map_err(|error| format!("Could not read project sidecar: {error}"))?,
    )
    .map_err(|error| format!("Could not parse project sidecar: {error}"))?;
    if marker.format_version != 1
        || Uuid::parse_str(&marker.project_id).is_err()
        || marker.files.len() > MAX_PROJECT_FILES
    {
        return Err("Project sidecar has an unsupported identity or size".into());
    }
    validate_project_name(&marker.name)?;
    let folder_name = project_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Managed project folder name is invalid".to_string())?;
    let folder_relative = format!("{PROJECTS_DIRECTORY}/{folder_name}");
    let root_name: String = sqlx::query_scalar(
        "SELECT display_name FROM library_roots WHERE id = ? AND root_kind = 'managed'",
    )
    .bind(root_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| "Managed library record is unavailable".to_string())?;
    let folder_id = ensure_folder_record(root_id, &folder_relative, &root_name, pool).await?;
    let existing = sqlx::query("SELECT m.storage_mode, f.root_id FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ?")
        .bind(&marker.project_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    if let Some(existing) = &existing {
        if existing.get::<String, _>("storage_mode") != "managed"
            || existing.get::<String, _>("root_id") != root_id
        {
            return Err("Project sidecar ID is already owned outside this managed library".into());
        }
        sqlx::query("UPDATE models SET folder_id = ?, storage_mode = 'managed', bundle_mode = 'manual', grouping_key = 'manual:' || id, trashed_at = NULL, trash_original_relative_path = NULL, missing_since = NULL, updated_at = ? WHERE id = ?")
            .bind(&folder_id)
            .bind(now())
            .bind(&marker.project_id)
            .execute(pool)
            .await
            .map_err(db_error)?;
    } else {
        let timestamp = now();
        sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, bundle_mode, storage_mode, added_at, updated_at) VALUES (?, ?, ?, ?, '', 'manual', 'managed', ?, ?)")
            .bind(&marker.project_id)
            .bind(&folder_id)
            .bind(marker.name.trim())
            .bind(format!("manual:{}", marker.project_id))
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    let mut seen_ids = HashSet::new();
    let mut seen_paths = HashSet::new();
    for file in &marker.files {
        if Uuid::parse_str(&file.asset_id).is_err()
            || !seen_ids.insert(file.asset_id.clone())
            || !seen_paths.insert(file.path.to_ascii_lowercase())
        {
            return Err("Project sidecar contains duplicate or invalid file identities".into());
        }
        let relative = Path::new(&file.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || relative
                .components()
                .next()
                .is_some_and(|component| component.as_os_str() == MARKER_DIRECTORY)
        {
            return Err("Project sidecar contains an unsafe file path".into());
        }
        let source = project_directory.join(relative);
        let metadata = match std::fs::symlink_metadata(&source) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                metadata
            }
            _ => continue,
        };
        let canonical_source = source
            .canonicalize()
            .map_err(|_| "A sidecar project file is unavailable".to_string())?;
        if !canonical_source.starts_with(&canonical_project)
            || !canonical_source.starts_with(canonical_root)
        {
            return Err("A sidecar project file escapes the managed project".into());
        }
        let extension = extension_for_path(&source)?;
        let filename = source
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "A sidecar project filename is invalid".to_string())?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let asset_folder_relative = if parent.as_os_str().is_empty() {
            folder_relative.clone()
        } else {
            format!(
                "{folder_relative}/{}",
                parent.to_string_lossy().replace('\\', "/")
            )
        };
        let asset_folder_id =
            ensure_folder_record(root_id, &asset_folder_relative, &root_name, pool).await?;
        let asset_relative = format!(
            "{folder_relative}/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        let path_asset = sqlx::query("SELECT a.id, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.root_id = ? AND a.relative_path = ?")
        .bind(root_id)
        .bind(&asset_relative)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
        let asset_id = if let Some(path_asset) = path_asset {
            let owner: Option<String> = path_asset.try_get("model_id").ok();
            if owner
                .as_deref()
                .is_some_and(|owner| owner != marker.project_id)
            {
                return Err("A managed project path is already owned by another project".into());
            }
            path_asset.get("id")
        } else {
            let identity = sqlx::query("SELECT a.root_id, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.id = ?")
                .bind(&file.asset_id)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?;
            if let Some(identity) = identity {
                let owner: Option<String> = identity.try_get("model_id").ok();
                if identity.get::<String, _>("root_id") != root_id
                    || owner
                        .as_deref()
                        .is_some_and(|owner| owner != marker.project_id)
                {
                    return Err("Project sidecar reuses an asset ID owned by another file".into());
                }
            }
            file.asset_id.clone()
        };
        let modified: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at, parse_status, metadata_json, missing_since) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', '{}', NULL) ON CONFLICT(id) DO UPDATE SET root_id = excluded.root_id, folder_id = excluded.folder_id, relative_path = excluded.relative_path, filename = excluded.filename, extension = excluded.extension, byte_size = excluded.byte_size, modified_at = excluded.modified_at, missing_since = NULL")
            .bind(&asset_id)
            .bind(root_id)
            .bind(&asset_folder_id)
            .bind(&asset_relative)
            .bind(filename)
            .bind(&extension)
            .bind(metadata.len() as i64)
            .bind(modified.to_rfc3339())
            .execute(pool)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM model_assets WHERE asset_id = ? AND model_id != ?")
            .bind(&asset_id)
            .bind(&marker.project_id)
            .execute(pool)
            .await
            .map_err(db_error)?;
        let role = if matches!(
            file.role.as_str(),
            "source" | "printable" | "plate" | "reference" | "version" | "variant"
        ) {
            file.role.as_str()
        } else {
            role_for_extension(&extension)
        };
        sqlx::query("INSERT INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, ?, COALESCE((SELECT MAX(sort_order) + 1 FROM model_assets WHERE model_id = ?), 0)) ON CONFLICT(model_id, asset_id) DO UPDATE SET role = excluded.role")
            .bind(&marker.project_id)
            .bind(&asset_id)
            .bind(role)
            .bind(&marker.project_id)
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    let mut transaction = pool.begin().await.map_err(db_error)?;
    set_primary_asset(&marker.project_id, &mut transaction).await?;
    transaction.commit().await.map_err(db_error)
}

async fn import_into_project(
    project_id: &str,
    file_paths: &[String],
    asset_ids: &[String],
    transfer_mode: &str,
    preserve_selected_ids: bool,
    state: &AppState,
) -> CommandResult<Vec<String>> {
    let project = sqlx::query("SELECT m.storage_mode, m.folder_id, f.relative_path folder_relative, f.root_id, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if project.get::<String, _>("storage_mode") != "managed" {
        return Err(
            "Files can only be imported into a managed project. Convert this project first.".into(),
        );
    }
    let root_path = PathBuf::from(project.get::<String, _>("root_path"));
    let root_canonical = root_path
        .canonicalize()
        .map_err(|_| "The managed library is unavailable".to_string())?;
    let folder_relative: String = project.get("folder_relative");
    let project_path = root_path.join(&folder_relative);
    let project_canonical = project_path
        .canonicalize()
        .map_err(|_| "The managed project folder is unavailable".to_string())?;
    if !project_canonical.starts_with(&root_canonical) {
        return Err("The managed project folder is outside the managed library".into());
    }
    let sources = collect_sources(file_paths, asset_ids, &state.pool).await?;
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    validate_source_count(sources.len())?;
    let job_id = Uuid::new_v4().to_string();
    let staging = root_path.join(STAGING_DIRECTORY).join(&job_id);
    std::fs::create_dir_all(&staging)
        .map_err(|error| format!("Could not prepare the file import: {error}"))?;
    let timestamp = now();
    sqlx::query("INSERT INTO file_operation_jobs (id, operation, state, payload_json, created_at, updated_at) VALUES (?, 'import-files', 'staging', ?, ?, ?)")
        .bind(&job_id)
        .bind(
            serde_json::to_string(&FileOperationPayload {
                project_id: project_id.to_string(),
                mode: transfer_mode.to_string(),
                files: Vec::new(),
            })
            .map_err(|error| error.to_string())?,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let existing_names = directory_names(&project_path)?;
    let mut reserved_names = existing_names;
    let mut prepared = Vec::with_capacity(sources.len());
    for source in sources {
        let filename = unique_asset_filename(&source.path, &mut reserved_names)?;
        let staged_path = staging.join(&filename);
        if let Err(error) = std::fs::copy(&source.path, &staged_path) {
            fail_job(&job_id, &error.to_string(), &state.pool).await;
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Could not copy {}: {error}", source.path.display()));
        }
        let source_hash = parsers::full_hash(&source.path)?;
        let staged_hash = parsers::full_hash(&staged_path)?;
        if source_hash != staged_hash {
            fail_job(
                &job_id,
                "Copied file did not pass verification",
                &state.pool,
            )
            .await;
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Could not verify the copied file {filename}"));
        }
        let extension = extension_for_path(&staged_path)?;
        let metadata = staged_path.metadata().map_err(|error| error.to_string())?;
        let modified: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let parsed: AssetMetadata = parsers::parse_metadata(&staged_path, &extension);
        let parse_status = if parsed.warning.is_some() {
            "warning"
        } else {
            "ready"
        };
        let partial = parsers::partial_fingerprint(&staged_path, metadata.len()).ok();
        let asset_id = if preserve_selected_ids {
            source
                .original_asset_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            Uuid::new_v4().to_string()
        };
        prepared.push(PreparedFile {
            source,
            staged_path,
            filename,
            extension,
            byte_size: metadata.len() as i64,
            modified_at: modified.to_rfc3339(),
            partial_fingerprint: partial,
            content_hash: source_hash,
            parse_status: parse_status.into(),
            metadata_json: serde_json::to_string(&parsed).map_err(|error| error.to_string())?,
            asset_id,
        });
    }
    let journal = FileOperationPayload {
        project_id: project_id.to_string(),
        mode: transfer_mode.to_string(),
        files: prepared
            .iter()
            .map(|file| FileOperationEntry {
                source: file.source.path.to_string_lossy().to_string(),
                destination: project_path
                    .join(&file.filename)
                    .to_string_lossy()
                    .to_string(),
                content_hash: file.content_hash.clone(),
            })
            .collect(),
    };
    sqlx::query("UPDATE file_operation_jobs SET state = 'committing', payload_json = ?, updated_at = ? WHERE id = ?")
        .bind(serde_json::to_string(&journal).map_err(|error| error.to_string())?)
        .bind(now())
        .bind(&job_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let mut final_paths = Vec::with_capacity(prepared.len());
    for file in &prepared {
        let destination = project_path.join(&file.filename);
        if destination.exists() {
            rollback_final_files(&final_paths, &staging);
            fail_job(
                &job_id,
                "A destination file appeared during import",
                &state.pool,
            )
            .await;
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("A file named {} already exists", file.filename));
        }
        if let Err(error) = std::fs::rename(&file.staged_path, &destination) {
            rollback_final_files(&final_paths, &staging);
            fail_job(&job_id, &error.to_string(), &state.pool).await;
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!(
                "Could not finish importing {}: {error}",
                file.filename
            ));
        }
        final_paths.push(destination);
    }
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    let mut affected_projects = HashSet::new();
    let root_id: String = project.get("root_id");
    let folder_id: String = project.get("folder_id");
    for file in &prepared {
        let relative_path = format!("{folder_relative}/{}", file.filename);
        if preserve_selected_ids && file.source.original_asset_id.is_some() {
            sqlx::query("DELETE FROM model_assets WHERE asset_id = ?")
                .bind(&file.asset_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
            sqlx::query("UPDATE assets SET root_id = ?, folder_id = ?, relative_path = ?, filename = ?, extension = ?, byte_size = ?, modified_at = ?, partial_fingerprint = ?, content_hash = ?, parse_status = ?, metadata_json = ?, missing_since = NULL WHERE id = ?")
                .bind(&root_id)
                .bind(&folder_id)
                .bind(&relative_path)
                .bind(&file.filename)
                .bind(&file.extension)
                .bind(file.byte_size)
                .bind(&file.modified_at)
                .bind(&file.partial_fingerprint)
                .bind(&file.content_hash)
                .bind(&file.parse_status)
                .bind(&file.metadata_json)
                .bind(&file.asset_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
            if let (Some(source_root_id), Some(source_relative)) = (
                file.source.source_root_id.as_deref(),
                file.source.source_relative_path.as_deref(),
            ) {
                sqlx::query("INSERT INTO ignored_asset_paths (id, root_id, relative_path, content_hash, reason, created_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(root_id, relative_path) DO UPDATE SET content_hash = excluded.content_hash, reason = excluded.reason, created_at = excluded.created_at")
                        .bind(Uuid::new_v4().to_string())
                        .bind(source_root_id)
                        .bind(source_relative)
                        .bind(&file.content_hash)
                        .bind(if transfer_mode == "move" { "consolidated-move" } else { "consolidated-copy" })
                        .bind(now())
                        .execute(&mut *transaction)
                        .await
                        .map_err(db_error)?;
            }
        } else {
            sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at, partial_fingerprint, content_hash, parse_status, metadata_json, missing_since) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)")
                .bind(&file.asset_id)
                .bind(&root_id)
                .bind(&folder_id)
                .bind(&relative_path)
                .bind(&file.filename)
                .bind(&file.extension)
                .bind(file.byte_size)
                .bind(&file.modified_at)
                .bind(&file.partial_fingerprint)
                .bind(&file.content_hash)
                .bind(&file.parse_status)
                .bind(&file.metadata_json)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        sqlx::query("INSERT INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, ?, COALESCE((SELECT MAX(sort_order) + 1 FROM model_assets WHERE model_id = ?), 0))")
            .bind(project_id)
            .bind(&file.asset_id)
            .bind(&file.source.role)
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        sqlx::query("INSERT INTO asset_origins (asset_id, source_kind, original_path, import_action, original_hash, imported_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(asset_id) DO UPDATE SET source_kind = excluded.source_kind, original_path = excluded.original_path, import_action = excluded.import_action, original_hash = excluded.original_hash, imported_at = excluded.imported_at")
            .bind(&file.asset_id)
            .bind(if file.source.original_asset_id.is_some() { "library" } else { "filesystem" })
            .bind(&file.source.original_path)
            .bind(transfer_mode)
            .bind(&file.content_hash)
            .bind(now())
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        if let Some(source_model_id) = &file.source.source_model_id {
            if source_model_id != project_id {
                affected_projects.insert(source_model_id.clone());
            }
        }
    }
    set_primary_asset(project_id, &mut transaction).await?;
    sqlx::query("UPDATE models SET missing_since = NULL, bundle_mode = 'manual', grouping_key = 'manual:' || id, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for source_project in &affected_projects {
        set_primary_asset(source_project, &mut transaction).await?;
        sqlx::query("UPDATE models SET bundle_mode = 'manual', grouping_key = 'manual:' || id, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(source_project)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    let committed_state = if transfer_mode == "move" {
        "cleanup"
    } else {
        "complete"
    };
    sqlx::query("UPDATE file_operation_jobs SET state = ?, updated_at = ? WHERE id = ?")
        .bind(committed_state)
        .bind(now())
        .bind(&job_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    if let Err(error) = transaction.commit().await.map_err(db_error) {
        for path in &final_paths {
            let _ = std::fs::remove_file(path);
        }
        fail_job(&job_id, &error, &state.pool).await;
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if transfer_mode == "move" {
        match cleanup_moved_sources(&journal) {
            Ok(()) => {
                let _ = sqlx::query("UPDATE file_operation_jobs SET state = 'complete', error = NULL, updated_at = ? WHERE id = ?")
                    .bind(now())
                    .bind(&job_id)
                    .execute(&state.pool)
                    .await;
            }
            Err(error) => {
                log::warn!("Managed import {job_id} will retry source cleanup: {error}");
                let _ = sqlx::query(
                    "UPDATE file_operation_jobs SET error = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&error)
                .bind(now())
                .bind(&job_id)
                .execute(&state.pool)
                .await;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(error) = write_project_marker(project_id, &state.pool).await {
        log::warn!("Could not refresh project sidecar after import for {project_id}: {error}");
    }
    if let Err(error) = indexing::refresh_search(project_id, &state.pool).await {
        log::warn!("Could not refresh project search after import for {project_id}: {error}");
    }
    let affected = affected_projects.into_iter().collect::<Vec<_>>();
    let empty_sources: Vec<String> = if affected.is_empty() || transfer_mode != "move" {
        Vec::new()
    } else {
        match serde_json::to_string(&affected) {
            Ok(ids) => sqlx::query_scalar("SELECT id FROM models WHERE id IN (SELECT value FROM json_each(?)) AND NOT EXISTS (SELECT 1 FROM model_assets WHERE model_id = models.id)")
                .bind(ids)
                .fetch_all(&state.pool)
                .await
                .map_err(db_error)
                .unwrap_or_else(|error| {
                    log::warn!("Could not identify emptied source projects after import: {error}");
                    Vec::new()
                }),
            Err(error) => {
                log::warn!("Could not serialize source project IDs after import: {error}");
                Vec::new()
            }
        }
    };
    let mut merged_sources = Vec::new();
    for source_project in &empty_sources {
        let retirement = match prepare_managed_project_retirement(source_project, project_id, state)
            .await
        {
            Ok(retirement) => retirement,
            Err(error) => {
                log::warn!("Could not prepare emptied project {source_project} for merge: {error}");
                continue;
            }
        };
        match crate::commands::merge_projects_impl(
            project_id,
            std::slice::from_ref(source_project),
            &state.pool,
        )
        .await
        {
            Ok(()) => {
                if let Some(retirement) = retirement {
                    let folder_id = retirement.folder_id.clone();
                    finish_managed_project_retirement(retirement);
                    let _ = sqlx::query("DELETE FROM folders WHERE id = ? AND NOT EXISTS (SELECT 1 FROM models WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM assets WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM folders WHERE parent_id = ?)")
                        .bind(&folder_id)
                        .bind(&folder_id)
                        .bind(&folder_id)
                        .bind(&folder_id)
                        .execute(&state.pool)
                        .await;
                }
                merged_sources.push(source_project.clone());
            }
            Err(error) => {
                if let Some(retirement) = retirement {
                    rollback_managed_project_retirement(retirement);
                }
                log::warn!(
                    "Could not merge emptied source project {source_project} into {project_id}: {error}"
                );
            }
        }
    }
    for source_project in affected {
        if !merged_sources.contains(&source_project) {
            if let Err(error) = indexing::refresh_search(&source_project, &state.pool).await {
                log::warn!("Could not refresh source project search for {source_project}: {error}");
            }
            if transfer_mode == "move" {
                let source_storage: Option<String> =
                    sqlx::query_scalar("SELECT storage_mode FROM models WHERE id = ?")
                        .bind(&source_project)
                        .fetch_optional(&state.pool)
                        .await
                        .unwrap_or(None);
                if source_storage.as_deref() == Some("managed") {
                    if let Err(error) = write_project_marker(&source_project, &state.pool).await {
                        log::warn!("Could not refresh source project sidecar for {source_project}: {error}");
                    }
                }
            }
        }
    }
    Ok(prepared.into_iter().map(|file| file.asset_id).collect())
}

async fn rollback_project_conversion_record(
    project_id: &str,
    old_folder_id: &str,
    old_bundle_mode: &str,
    old_grouping_key: &str,
    managed_folder_id: &str,
    project_path: &Path,
    state: &AppState,
) {
    let restored = sqlx::query("UPDATE models SET folder_id = ?, storage_mode = 'linked', bundle_mode = ?, grouping_key = ?, updated_at = ? WHERE id = ?")
        .bind(old_folder_id)
        .bind(old_bundle_mode)
        .bind(old_grouping_key)
        .bind(now())
        .bind(project_id)
        .execute(&state.pool)
        .await;
    if let Err(error) = restored {
        log::error!(
            "Could not restore linked project {project_id} after failed conversion; its managed folder was preserved: {error}"
        );
        return;
    }
    if let Err(error) = std::fs::remove_dir_all(project_path) {
        log::warn!(
            "Could not remove failed managed project folder {}: {error}",
            project_path.display()
        );
    }
    let _ = sqlx::query("DELETE FROM folders WHERE id = ? AND NOT EXISTS (SELECT 1 FROM models WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM assets WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM folders WHERE parent_id = ?)")
        .bind(managed_folder_id)
        .bind(managed_folder_id)
        .bind(managed_folder_id)
        .bind(managed_folder_id)
        .execute(&state.pool)
        .await;
}

async fn create_linked_project(
    input: &CreateManagedProjectInput,
    state: &AppState,
) -> CommandResult<String> {
    if input.file_paths.is_empty() && input.asset_ids.is_empty() {
        return Err("Empty projects need a managed Volum library".into());
    }
    let asset_ids = resolve_linked_assets(&input.file_paths, &input.asset_ids, state).await?;
    let folder_id: String =
        sqlx::query_scalar("SELECT folder_id FROM assets WHERE id = ? AND missing_since IS NULL")
            .bind(&asset_ids[0])
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?;
    let project_id = Uuid::new_v4().to_string();
    let timestamp = now();
    sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, bundle_mode, storage_mode, added_at, updated_at) VALUES (?, ?, ?, ?, '', 'manual', 'linked', ?, ?)")
        .bind(&project_id)
        .bind(&folder_id)
        .bind(input.name.trim())
        .bind(format!("manual:{project_id}"))
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    if let Err(error) = attach_linked_assets(&project_id, &asset_ids, state).await {
        let _ = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(&project_id)
            .execute(&state.pool)
            .await;
        return Err(error);
    }
    Ok(project_id)
}

async fn add_linked_files_to_project(
    input: &AddProjectFilesInput,
    state: &AppState,
) -> CommandResult<Vec<String>> {
    let mode: String = sqlx::query_scalar("SELECT storage_mode FROM models WHERE id = ?")
        .bind(&input.project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if mode != "linked" {
        return Err(
            "Managed projects must copy or move new files into their project folder".into(),
        );
    }
    let asset_ids = resolve_linked_assets(&input.file_paths, &input.asset_ids, state).await?;
    attach_linked_assets(&input.project_id, &asset_ids, state).await?;
    Ok(asset_ids)
}

async fn resolve_linked_assets(
    file_paths: &[String],
    selected_asset_ids: &[String],
    state: &AppState,
) -> CommandResult<Vec<String>> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for asset_id in selected_asset_ids {
        let row = sqlx::query("SELECT a.id, a.missing_since, r.root_kind FROM assets a JOIN library_roots r ON r.id = a.root_id WHERE a.id = ?")
            .bind(asset_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "A selected library file no longer exists".to_string())?;
        if row.get::<Option<String>, _>("missing_since").is_some() {
            return Err("A selected library file is unavailable".into());
        }
        if row.get::<String, _>("root_kind") == "managed" {
            return Err(
                "Managed files cannot be kept linked in another project. Choose Copy or Move."
                    .into(),
            );
        }
        let id: String = row.get("id");
        if seen.insert(id.clone()) {
            result.push(id);
        }
    }
    let roots =
        sqlx::query("SELECT id, path, root_kind FROM library_roots WHERE status != 'removed'")
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    for value in file_paths {
        let path = PathBuf::from(value)
            .canonicalize()
            .map_err(|_| format!("The selected file is unavailable: {value}"))?;
        validate_source_file(&path)?;
        let mut candidates = Vec::new();
        for row in &roots {
            let root_path = PathBuf::from(row.get::<String, _>("path"));
            if let Ok(canonical_root) = root_path.canonicalize() {
                if path.starts_with(&canonical_root) {
                    candidates.push((
                        canonical_root.components().count(),
                        row.get::<String, _>("id"),
                        row.get::<String, _>("root_kind"),
                        canonical_root,
                    ));
                }
            }
        }
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
        let (root_id, root_path) = if let Some((_, id, kind, root_path)) =
            candidates.into_iter().next()
        {
            if kind == "managed" {
                return Err(
                    "Files inside the managed Volum library are already owned by a managed project"
                        .into(),
                );
            }
            (id, root_path)
        } else {
            let parent = path
                .parent()
                .ok_or_else(|| "The selected file has no parent folder".to_string())?
                .to_path_buf();
            for row in &roots {
                if row.get::<String, _>("root_kind") == "managed" {
                    let managed = PathBuf::from(row.get::<String, _>("path"));
                    if paths_overlap(&parent, &managed) {
                        return Err(
                            "A linked folder cannot overlap the managed Volum library".into()
                        );
                    }
                }
            }
            let root_id = Uuid::new_v4().to_string();
            let display_name = parent
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Linked files")
                .to_string();
            let timestamp = now();
            sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, ?, ?, 'online', 'linked', ?, ?)")
                .bind(&root_id)
                .bind(parent.to_string_lossy().to_string())
                .bind(display_name)
                .bind(&timestamp)
                .bind(&timestamp)
                .execute(&state.pool)
                .await
                .map_err(db_error)?;
            (root_id, parent)
        };
        let relative = path
            .strip_prefix(&root_path)
            .map_err(|_| "Could not resolve a selected linked file".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let ids = indexing::index_imported_files(
            &root_id,
            &root_path,
            std::slice::from_ref(&relative),
            &state.pool,
        )
        .await?;
        for id in ids {
            if seen.insert(id.clone()) {
                result.push(id);
            }
        }
    }
    if result.is_empty() {
        return Err("Choose at least one project file".into());
    }
    Ok(result)
}

async fn attach_linked_assets(
    project_id: &str,
    asset_ids: &[String],
    state: &AppState,
) -> CommandResult<()> {
    let mut source_projects = HashSet::new();
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    for asset_id in asset_ids {
        let rows = sqlx::query("SELECT model_id, role FROM model_assets WHERE asset_id = ?")
            .bind(asset_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(db_error)?;
        let role = rows
            .first()
            .map(|row| row.get::<String, _>("role"))
            .unwrap_or_else(|| "variant".into());
        for row in rows {
            let source: String = row.get("model_id");
            if source != project_id {
                source_projects.insert(source);
            }
        }
        sqlx::query("DELETE FROM model_assets WHERE asset_id = ?")
            .bind(asset_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        sqlx::query("INSERT INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, ?, COALESCE((SELECT MAX(sort_order) + 1 FROM model_assets WHERE model_id = ?), 0))")
            .bind(project_id)
            .bind(asset_id)
            .bind(role)
            .bind(project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    set_primary_asset(project_id, &mut transaction).await?;
    sqlx::query("UPDATE models SET grouping_key = 'manual:' || id, bundle_mode = 'manual', missing_since = NULL, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for source in &source_projects {
        set_primary_asset(source, &mut transaction).await?;
    }
    transaction.commit().await.map_err(db_error)?;
    let empty_sources: Vec<String> = if source_projects.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar("SELECT id FROM models WHERE id IN (SELECT value FROM json_each(?)) AND NOT EXISTS (SELECT 1 FROM model_assets WHERE model_id = models.id)")
            .bind(serde_json::to_string(&source_projects).map_err(|error| error.to_string())?)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
    };
    if !empty_sources.is_empty() {
        crate::commands::merge_projects_impl(project_id, &empty_sources, &state.pool).await?;
    }
    for source in source_projects {
        if !empty_sources.contains(&source) {
            indexing::refresh_search(&source, &state.pool).await?;
        }
    }
    indexing::refresh_search(project_id, &state.pool).await
}

async fn collect_sources(
    file_paths: &[String],
    asset_ids: &[String],
    pool: &SqlitePool,
) -> CommandResult<Vec<SourceFile>> {
    let mut sources = Vec::with_capacity(file_paths.len() + asset_ids.len());
    let mut seen = HashSet::new();
    for path in file_paths {
        let canonical = PathBuf::from(path)
            .canonicalize()
            .map_err(|_| format!("The selected file is unavailable: {path}"))?;
        validate_source_file(&canonical)?;
        let key = canonical.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            sources.push(SourceFile {
                original_path: canonical.to_string_lossy().to_string(),
                path: canonical.clone(),
                original_asset_id: None,
                source_model_id: None,
                source_root_id: None,
                source_relative_path: None,
                role: role_for_extension(&extension_for_path(&canonical)?).into(),
            });
        }
    }
    for asset_id in asset_ids {
        let row = sqlx::query("SELECT a.id, a.relative_path, a.missing_since, a.root_id, r.path root_path, ma.model_id, ma.role FROM assets a JOIN library_roots r ON r.id = a.root_id JOIN model_assets ma ON ma.asset_id = a.id WHERE a.id = ?")
            .bind(asset_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "A selected library file no longer exists".to_string())?;
        if row.get::<Option<String>, _>("missing_since").is_some() {
            return Err("A selected library file is currently unavailable".into());
        }
        let root = PathBuf::from(row.get::<String, _>("root_path"));
        let canonical_root = root
            .canonicalize()
            .map_err(|_| "A selected library is offline".to_string())?;
        let relative: String = row.get("relative_path");
        let canonical = root
            .join(&relative)
            .canonicalize()
            .map_err(|_| "A selected library file is unavailable".to_string())?;
        if !canonical.starts_with(&canonical_root) {
            return Err("A selected library file is outside its approved root".into());
        }
        validate_source_file(&canonical)?;
        let key = canonical.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            sources.push(SourceFile {
                original_path: canonical.to_string_lossy().to_string(),
                path: canonical,
                original_asset_id: Some(row.get("id")),
                source_model_id: Some(row.get("model_id")),
                source_root_id: Some(row.get("root_id")),
                source_relative_path: Some(relative),
                role: row.get("role"),
            });
        }
    }
    Ok(sources)
}

async fn rename_project_records(
    project_id: &str,
    folder_id: String,
    old_relative: &str,
    new_relative: &str,
    name: &str,
    pool: &SqlitePool,
) -> CommandResult<()> {
    let rows = sqlx::query("SELECT id, relative_path FROM assets WHERE id IN (SELECT asset_id FROM model_assets WHERE model_id = ?)")
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE folders SET relative_path = ?, name = ?, modified_at = ? WHERE id = ?")
        .bind(new_relative)
        .bind(
            Path::new(new_relative)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(name),
        )
        .bind(now())
        .bind(&folder_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for row in rows {
        let relative: String = row.get("relative_path");
        let suffix = relative
            .strip_prefix(old_relative)
            .ok_or_else(|| "A project file is outside its managed folder".to_string())?;
        sqlx::query("UPDATE assets SET relative_path = ? WHERE id = ?")
            .bind(format!("{new_relative}{suffix}"))
            .bind(row.get::<String, _>("id"))
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE models SET display_name = ?, grouping_key = 'manual:' || id, bundle_mode = 'manual', updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(now())
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)
}

async fn update_project_folder_paths(
    project_id: &str,
    folder_id: &str,
    old_relative: &str,
    new_relative: &str,
    trash_original_relative_path: Option<&str>,
    trashed_at: Option<String>,
    pool: &SqlitePool,
) -> CommandResult<()> {
    let rows = sqlx::query("SELECT id, relative_path FROM assets WHERE id IN (SELECT asset_id FROM model_assets WHERE model_id = ?)")
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE folders SET relative_path = ?, name = ?, modified_at = ? WHERE id = ?")
        .bind(new_relative)
        .bind(
            Path::new(new_relative)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Project"),
        )
        .bind(now())
        .bind(folder_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for row in rows {
        let relative: String = row.get("relative_path");
        let suffix = relative
            .strip_prefix(old_relative)
            .ok_or_else(|| "A project file is outside its managed folder".to_string())?;
        sqlx::query("UPDATE assets SET relative_path = ? WHERE id = ?")
            .bind(format!("{new_relative}{suffix}"))
            .bind(row.get::<String, _>("id"))
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE models SET trashed_at = ?, trash_original_relative_path = ?, updated_at = ? WHERE id = ?")
        .bind(trashed_at)
        .bind(trash_original_relative_path)
        .bind(now())
        .bind(project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)
}

async fn set_primary_asset(
    project_id: &str,
    transaction: &mut Transaction<'_, Sqlite>,
) -> CommandResult<()> {
    let primary: Option<String> = sqlx::query_scalar("SELECT a.id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.missing_since IS NULL ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 WHEN 'step' THEN 3 WHEN 'stp' THEN 3 ELSE 4 END, ma.sort_order, lower(a.filename) LIMIT 1")
        .bind(project_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
        .bind(primary)
        .bind(project_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn managed_library_status(pool: &SqlitePool) -> CommandResult<ManagedLibraryStatus> {
    let row = sqlx::query("SELECT id FROM library_roots WHERE root_kind = 'managed' AND status != 'removed' ORDER BY created_at LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(ManagedLibraryStatus {
            root: None,
            projects_path: None,
            inbox_path: None,
            trash_path: None,
        });
    };
    let root = root_by_id(&row.get::<String, _>("id"), pool).await?;
    Ok(ManagedLibraryStatus {
        projects_path: Some(
            PathBuf::from(&root.path)
                .join(PROJECTS_DIRECTORY)
                .to_string_lossy()
                .to_string(),
        ),
        inbox_path: Some(
            PathBuf::from(&root.path)
                .join(INBOX_DIRECTORY)
                .to_string_lossy()
                .to_string(),
        ),
        trash_path: Some(
            PathBuf::from(&root.path)
                .join(TRASH_DIRECTORY)
                .to_string_lossy()
                .to_string(),
        ),
        root: Some(root),
    })
}

async fn managed_root(pool: &SqlitePool) -> CommandResult<(String, PathBuf, String)> {
    let row = sqlx::query("SELECT id, path, display_name FROM library_roots WHERE root_kind = 'managed' AND status != 'removed' ORDER BY created_at LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Choose a managed Volum library in Settings first".to_string())?;
    Ok((
        row.get("id"),
        PathBuf::from(row.get::<String, _>("path")),
        row.get("display_name"),
    ))
}

async fn root_by_id(id: &str, pool: &SqlitePool) -> CommandResult<LibraryRoot> {
    let row = sqlx::query("SELECT r.id, r.path, r.display_name, r.status, r.last_scan_at, r.root_kind, (SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = r.id AND m.missing_since IS NULL AND m.trashed_at IS NULL) model_count FROM library_roots r WHERE r.id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    Ok(LibraryRoot {
        id: row.get("id"),
        path: row.get("path"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        last_scan_at: row.get("last_scan_at"),
        model_count: row.get("model_count"),
        root_kind: row.get("root_kind"),
    })
}

async fn ensure_folder_record(
    root_id: &str,
    relative: &str,
    root_name: &str,
    pool: &SqlitePool,
) -> CommandResult<String> {
    let mut current = String::new();
    let segments: Vec<&str> = relative
        .split('/')
        .filter(|value| !value.is_empty())
        .collect();
    if segments.is_empty() {
        return Err("The project folder is invalid".into());
    }
    let root_folder =
        sqlx::query("SELECT id FROM folders WHERE root_id = ? AND relative_path = ''")
            .bind(root_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;
    let root_folder_id = root_folder
        .map(|row| row.get::<String, _>("id"))
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, NULL, '', ?, ?)")
        .bind(&root_folder_id)
        .bind(root_id)
        .bind(root_name)
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    let mut parent_id = Some(root_folder_id);
    for segment in segments {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM folders WHERE root_id = ? AND relative_path = ?")
                .bind(root_id)
                .bind(&current)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?;
        let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(root_id)
            .bind(&parent_id)
            .bind(&current)
            .bind(segment)
            .bind(now())
            .execute(pool)
            .await
            .map_err(db_error)?;
        parent_id = Some(id);
    }
    parent_id.ok_or_else(|| "Could not create the project folder record".into())
}

fn ensure_layout(root: &Path) -> CommandResult<()> {
    for name in [
        PROJECTS_DIRECTORY,
        INBOX_DIRECTORY,
        TRASH_DIRECTORY,
        STAGING_DIRECTORY,
    ] {
        let path = root.join(name);
        if path.exists() && !path.is_dir() {
            return Err(format!("{} is not a folder", path.display()));
        }
        std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn write_library_marker(root: &Path) -> CommandResult<()> {
    let marker = root.join(".volum-library.json");
    let temporary = root.join(format!(".volum-library.{}.tmp", Uuid::new_v4()));
    let payload = serde_json::json!({
        "formatVersion": 1,
        "kind": "volum-managed-library"
    });
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    replace_file_atomically(&temporary, &marker)
}

fn atomic_file_backup(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    path.with_file_name(format!("{filename}.volum-backup"))
}

fn restore_atomic_file_backup(path: &Path) -> CommandResult<()> {
    let backup = atomic_file_backup(path);
    if !path.exists() && backup.is_file() {
        std::fs::rename(&backup, path).map_err(|error| error.to_string())?;
    } else if path.exists() && backup.exists() {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}

fn replace_file_atomically(temporary: &Path, destination: &Path) -> CommandResult<()> {
    restore_atomic_file_backup(destination)?;
    let backup = atomic_file_backup(destination);
    let had_destination = destination.exists();
    if had_destination {
        std::fs::rename(destination, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = std::fs::rename(temporary, destination) {
        if had_destination {
            let _ = std::fs::rename(&backup, destination);
        }
        return Err(error.to_string());
    }
    if had_destination {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}

pub(crate) fn validate_project_name(name: &str) -> CommandResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_PROJECT_NAME_CHARS {
        return Err(format!(
            "Project names must contain 1–{MAX_PROJECT_NAME_CHARS} characters"
        ));
    }
    safe_component(trimmed, "Project")?;
    Ok(())
}

fn validate_source_count(count: usize) -> CommandResult<()> {
    if count > MAX_PROJECT_FILES {
        Err(format!(
            "A project can import at most {MAX_PROJECT_FILES} files at once"
        ))
    } else {
        Ok(())
    }
}

fn validate_transfer_mode(mode: &str) -> CommandResult<()> {
    if matches!(mode, "copy" | "move") {
        Ok(())
    } else {
        Err("Choose whether to copy or move the selected files".into())
    }
}

fn validate_source_file(path: &Path) -> CommandResult<()> {
    if !path.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    extension_for_path(path).map(|_| ())
}

fn discover_inbox_projects(inbox: &Path) -> CommandResult<Vec<(String, Vec<String>)>> {
    if !inbox.is_dir() {
        return Ok(Vec::new());
    }
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(inbox)
        .map_err(|error| format!("Could not read the managed Inbox: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Could not read an Inbox entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Could not inspect an Inbox entry: {error}"))?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().trim().to_string();
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        if file_type.is_file() {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if SUPPORTED.contains(&extension.as_str()) {
                let project_name = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Inbox project")
                    .to_string();
                projects.push((project_name, vec![path.to_string_lossy().to_string()]));
            }
            continue;
        }
        if !file_type.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        for child in WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|child| {
                child.path() == path
                    || child
                        .file_name()
                        .to_str()
                        .is_some_and(|value| !value.starts_with('.'))
            })
        {
            let child = match child {
                Ok(child) => child,
                Err(error) => {
                    log::warn!("Could not inspect an Inbox file: {error}");
                    continue;
                }
            };
            if child.file_type().is_symlink() || !child.file_type().is_file() {
                continue;
            }
            let extension = child
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if SUPPORTED.contains(&extension.as_str()) {
                files.push(child.path().to_string_lossy().to_string());
            }
            if files.len() > MAX_PROJECT_FILES {
                return Err(format!(
                    "Inbox folder {name} contains more than {MAX_PROJECT_FILES} project files"
                ));
            }
        }
        files.sort_by_key(|path| path.to_ascii_lowercase());
        projects.push((name, files));
    }
    projects.sort_by(|left, right| {
        left.0
            .to_ascii_lowercase()
            .cmp(&right.0.to_ascii_lowercase())
    });
    Ok(projects)
}

fn extension_for_path(path: &Path) -> CommandResult<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if SUPPORTED.contains(&extension.as_str()) {
        Ok(extension)
    } else {
        Err(format!("Unsupported 3D file format: {extension}"))
    }
}

fn role_for_extension(extension: &str) -> &'static str {
    match extension {
        "step" | "stp" => "source",
        "3mf" => "plate",
        _ => "printable",
    }
}

fn safe_component(value: &str, fallback: &str) -> CommandResult<String> {
    if value.contains(['/', '\\']) || value.chars().any(char::is_control) {
        return Err("Names cannot contain path separators or control characters".into());
    }
    let invalid = ['<', '>', ':', '"', '|', '?', '*'];
    let mut safe = value
        .chars()
        .map(|character| {
            if invalid.contains(&character) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>();
    safe = safe.trim().trim_end_matches(['.', ' ']).to_string();
    if safe.is_empty() {
        safe = fallback.into();
    }
    let upper = safe.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or_default();
    if matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        safe.push('-');
    }
    Ok(safe.chars().take(180).collect())
}

fn unique_project_folder(projects: &Path, name: &str) -> CommandResult<String> {
    let base = safe_component(name, "Project")?;
    for suffix in 1..=10_000_u32 {
        let candidate = if suffix == 1 {
            base.clone()
        } else {
            format!("{base} ({suffix})")
        };
        if !path_exists_case_insensitive(projects, &candidate)? {
            return Ok(candidate);
        }
    }
    Err("Could not find an unused project folder name".into())
}

fn unique_asset_filename(path: &Path, reserved: &mut HashSet<String>) -> CommandResult<String> {
    let extension = extension_for_path(path)?;
    let original_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("model");
    let stem = safe_component(original_stem, "model")?;
    for suffix in 1..=10_000_u32 {
        let candidate = if suffix == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{suffix}.{extension}")
        };
        if reserved.insert(candidate.to_lowercase()) {
            return Ok(candidate);
        }
    }
    Err("Could not find an unused filename".into())
}

fn validate_renamed_file(value: &str, extension: &str) -> CommandResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Enter a filename".into());
    }
    let supplied_extension = Path::new(trimmed)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if let Some(supplied) = supplied_extension.as_deref() {
        if supplied != extension {
            return Err("File extensions cannot be changed when renaming".into());
        }
    }
    let stem = if supplied_extension.is_some() {
        Path::new(trimmed)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
    } else {
        trimmed
    };
    let safe = safe_component(stem, "model")?;
    if safe != stem.trim() {
        return Err("The filename contains characters that are not portable".into());
    }
    Ok(format!("{safe}.{extension}"))
}

fn directory_names(path: &Path) -> CommandResult<HashSet<String>> {
    let mut names = HashSet::new();
    for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if let Some(name) = entry.file_name().to_str() {
            names.insert(name.to_lowercase());
        }
    }
    Ok(names)
}

fn path_exists_case_insensitive(parent: &Path, filename: &str) -> CommandResult<bool> {
    if !parent.exists() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(parent).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(filename))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rename_case_safe(source: &Path, destination: &Path) -> CommandResult<()> {
    if source == destination {
        return Ok(());
    }
    if same_path_case_insensitive(source, destination) {
        let parent = source
            .parent()
            .ok_or_else(|| "Invalid file path".to_string())?;
        let temporary = parent.join(format!(".volum-rename-{}", Uuid::new_v4()));
        std::fs::rename(source, &temporary).map_err(|error| error.to_string())?;
        if let Err(error) = std::fs::rename(&temporary, destination) {
            let _ = std::fs::rename(&temporary, source);
            return Err(error.to_string());
        }
        Ok(())
    } else {
        std::fs::rename(source, destination).map_err(|error| error.to_string())
    }
}

fn same_path_case_insensitive(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn safe_app_media_path(data_dir: &Path, relative: &str) -> CommandResult<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("Project photo has an unsafe storage path".into());
    }
    Ok(data_dir.join("media").join(relative))
}

fn rollback_final_files(paths: &[PathBuf], staging: &Path) {
    for path in paths {
        if let Some(name) = path.file_name() {
            let _ = std::fs::rename(path, staging.join(name));
        }
    }
}

async fn prepare_managed_project_retirement(
    source_project_id: &str,
    target_project_id: &str,
    state: &AppState,
) -> CommandResult<Option<ManagedProjectRetirement>> {
    let source = sqlx::query("SELECT m.storage_mode, m.folder_id, f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(source_project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Emptied source project is unavailable".to_string())?;
    if source.get::<String, _>("storage_mode") != "managed" {
        return Ok(None);
    }
    let target = sqlx::query("SELECT f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ? AND m.storage_mode = 'managed'")
        .bind(target_project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Managed destination project is unavailable".to_string())?;
    let source_directory = PathBuf::from(source.get::<String, _>("root_path"))
        .join(source.get::<String, _>("relative_path"));
    let target_directory = PathBuf::from(target.get::<String, _>("root_path"))
        .join(target.get::<String, _>("relative_path"));
    let photos = sqlx::query(
        "SELECT media_path FROM project_photos WHERE model_id = ? AND media_path LIKE 'managed:%'",
    )
    .bind(source_project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;
    let mut copied_photos = Vec::new();
    for photo in photos {
        let storage_path: String = photo.get("media_path");
        let relative = storage_path
            .strip_prefix("managed:")
            .ok_or_else(|| "Managed photo storage path is invalid".to_string())?;
        let relative = Path::new(relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err("Managed photo storage path is unsafe".into());
        }
        let source_path = source_directory.join(relative);
        let destination = target_directory.join(relative);
        if !source_path.is_file() || destination.exists() {
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err(
                "A managed project photo is unavailable or collides at the destination".into(),
            );
        }
        let Some(parent) = destination.parent() else {
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err("Managed photo destination is invalid".into());
        };
        if let Err(error) = std::fs::create_dir_all(parent) {
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err(format!("Could not prepare managed photo transfer: {error}"));
        }
        let temporary = parent.join(format!(".photo-transfer-{}.partial", Uuid::new_v4()));
        if let Err(error) = std::fs::copy(&source_path, &temporary) {
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err(format!("Could not stage a managed photo: {error}"));
        }
        let source_hash = match parsers::full_hash(&source_path) {
            Ok(hash) => hash,
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                rollback_managed_project_retirement(ManagedProjectRetirement {
                    folder_id: source.get("folder_id"),
                    source_directory,
                    copied_photos,
                });
                return Err(error);
            }
        };
        let copied_hash = match parsers::full_hash(&temporary) {
            Ok(hash) => hash,
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                rollback_managed_project_retirement(ManagedProjectRetirement {
                    folder_id: source.get("folder_id"),
                    source_directory,
                    copied_photos,
                });
                return Err(error);
            }
        };
        if source_hash != copied_hash {
            let _ = std::fs::remove_file(&temporary);
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err("A managed photo did not pass transfer verification".into());
        }
        if let Err(error) = std::fs::rename(&temporary, &destination) {
            let _ = std::fs::remove_file(&temporary);
            rollback_managed_project_retirement(ManagedProjectRetirement {
                folder_id: source.get("folder_id"),
                source_directory,
                copied_photos,
            });
            return Err(format!(
                "Could not finish transferring a managed photo: {error}"
            ));
        }
        copied_photos.push((source_path, destination));
    }
    Ok(Some(ManagedProjectRetirement {
        folder_id: source.get("folder_id"),
        source_directory,
        copied_photos,
    }))
}

fn rollback_managed_project_retirement(retirement: ManagedProjectRetirement) {
    for (_, destination) in retirement.copied_photos {
        let _ = std::fs::remove_file(destination);
    }
}

fn finish_managed_project_retirement(retirement: ManagedProjectRetirement) {
    for (source, _) in &retirement.copied_photos {
        if let Err(error) = std::fs::remove_file(source) {
            log::warn!(
                "Could not remove transferred project photo {}: {error}",
                source.display()
            );
        }
    }
    let marker_directory = retirement.source_directory.join(MARKER_DIRECTORY);
    let _ = std::fs::remove_file(marker_directory.join(MARKER_FILENAME));
    let _ = std::fs::remove_dir(&marker_directory);
    let _ = std::fs::remove_dir(retirement.source_directory.join("Photos"));
    if std::fs::remove_dir(&retirement.source_directory).is_ok() {
        return;
    }
    let Some(root) = retirement.source_directory.parent().and_then(Path::parent) else {
        return;
    };
    let trash = root.join(TRASH_DIRECTORY);
    let original_name = retirement
        .source_directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Merged project");
    match unique_project_folder(&trash, &format!("Merged - {original_name}")) {
        Ok(name) => {
            if let Err(error) = std::fs::rename(&retirement.source_directory, trash.join(name)) {
                log::warn!("Could not retire emptied managed project folder: {error}");
            }
        }
        Err(error) => log::warn!("Could not choose a safe retirement folder: {error}"),
    }
}

fn cleanup_moved_sources(payload: &FileOperationPayload) -> CommandResult<()> {
    for file in &payload.files {
        let source = PathBuf::from(&file.source);
        let destination = PathBuf::from(&file.destination);
        if source == destination || !source.exists() {
            continue;
        }
        if !source.is_file() || !destination.is_file() {
            return Err(format!(
                "Could not verify move cleanup for {}",
                source.display()
            ));
        }
        let source_hash = parsers::full_hash(&source)?;
        let destination_hash = parsers::full_hash(&destination)?;
        if source_hash != file.content_hash || destination_hash != file.content_hash {
            return Err(format!(
                "Refused to delete a changed source file: {}",
                source.display()
            ));
        }
        std::fs::remove_file(&source)
            .map_err(|error| format!("Could not remove {}: {error}", source.display()))?;
    }
    Ok(())
}

async fn rollback_interrupted_package(
    payload: &PackageOperationPayload,
    managed_roots: &[PathBuf],
    pool: &SqlitePool,
    data_dir: &Path,
) -> CommandResult<()> {
    if Uuid::parse_str(&payload.project_id).is_err() {
        return Err("Interrupted package import has an invalid project identity".into());
    }
    let staging = PathBuf::from(&payload.staging);
    let staging_allowed = managed_roots.iter().any(|root| {
        staging
            .parent()
            .is_some_and(|parent| parent == root.join(STAGING_DIRECTORY))
    });
    if !staging_allowed {
        return Err("Interrupted package staging is outside the managed library".into());
    }
    let project = sqlx::query("SELECT m.folder_id, f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ? AND m.storage_mode = 'managed' AND r.root_kind = 'managed'")
        .bind(&payload.project_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    if let Some(project) = project {
        let root = PathBuf::from(project.get::<String, _>("root_path"));
        let relative: String = project.get("relative_path");
        let relative_path = Path::new(&relative);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || !relative.starts_with(&format!("{PROJECTS_DIRECTORY}/"))
        {
            return Err("Interrupted package project path is unsafe".into());
        }
        let target = root.join(relative_path);
        if target.exists() {
            let canonical_root = root
                .canonicalize()
                .map_err(|_| "The managed library is unavailable during recovery".to_string())?;
            let canonical_target = target
                .canonicalize()
                .map_err(|_| "The interrupted package folder is unavailable".to_string())?;
            if !canonical_target.starts_with(canonical_root.join(PROJECTS_DIRECTORY)) {
                return Err(
                    "Interrupted package folder escapes the managed Projects folder".into(),
                );
            }
        }
        let asset_ids: Vec<String> =
            sqlx::query_scalar("SELECT asset_id FROM model_assets WHERE model_id = ?")
                .bind(&payload.project_id)
                .fetch_all(pool)
                .await
                .map_err(db_error)?;
        let photo_rows =
            sqlx::query("SELECT thumbnail_path FROM project_photos WHERE model_id = ?")
                .bind(&payload.project_id)
                .fetch_all(pool)
                .await
                .map_err(db_error)?;
        let folder_id: String = project.get("folder_id");
        let mut transaction = pool.begin().await.map_err(db_error)?;
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(&payload.project_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        for asset_id in asset_ids {
            sqlx::query("DELETE FROM assets WHERE id = ? AND NOT EXISTS (SELECT 1 FROM model_assets WHERE asset_id = ?)")
                .bind(&asset_id)
                .bind(&asset_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        sqlx::query("DELETE FROM folders WHERE id = ? AND NOT EXISTS (SELECT 1 FROM models WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM assets WHERE folder_id = ?) AND NOT EXISTS (SELECT 1 FROM folders WHERE parent_id = ?)")
            .bind(&folder_id)
            .bind(&folder_id)
            .bind(&folder_id)
            .bind(&folder_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        transaction.commit().await.map_err(db_error)?;
        if target.is_dir() {
            std::fs::remove_dir_all(&target)
                .map_err(|error| format!("Could not remove interrupted package folder: {error}"))?;
        } else if target.exists() {
            std::fs::remove_file(&target)
                .map_err(|error| format!("Could not remove interrupted package path: {error}"))?;
        }
        for photo in photo_rows {
            let thumbnail: String = photo.get("thumbnail_path");
            if let Ok(path) = safe_app_media_path(data_dir, &thumbnail) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    if staging.is_dir() {
        std::fs::remove_dir_all(&staging)
            .map_err(|error| format!("Could not remove interrupted package staging: {error}"))?;
    } else if staging.exists() {
        std::fs::remove_file(&staging)
            .map_err(|error| format!("Could not remove interrupted package staging: {error}"))?;
    }
    Ok(())
}

async fn recover_path_operation(
    operation: &str,
    payload: &PathOperationPayload,
    managed_roots: &[PathBuf],
    pool: &SqlitePool,
) -> CommandResult<bool> {
    if Uuid::parse_str(&payload.entity_id).is_err() {
        return Err("Interrupted path operation has an invalid identity".into());
    }
    let source = PathBuf::from(&payload.source);
    let destination = PathBuf::from(&payload.destination);
    if !operation_path_is_managed(&source, managed_roots)
        || !operation_path_is_managed(&destination, managed_roots)
    {
        return Err("Interrupted path operation escapes the managed library".into());
    }
    let committed = match operation {
        "rename-project" => {
            let relative: Option<String> = sqlx::query_scalar("SELECT f.relative_path FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ? AND m.storage_mode = 'managed'")
                .bind(&payload.entity_id)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?;
            match relative.as_deref() {
                Some(relative) if relative == payload.new_relative => true,
                Some(relative) if relative == payload.old_relative => false,
                _ => return Err("Interrupted project rename no longer matches the database".into()),
            }
        }
        "rename-asset" => {
            let relative: Option<String> =
                sqlx::query_scalar("SELECT relative_path FROM assets WHERE id = ?")
                    .bind(&payload.entity_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(db_error)?;
            match relative.as_deref() {
                Some(relative) if relative == payload.new_relative => true,
                Some(relative) if relative == payload.old_relative => false,
                _ => return Err("Interrupted file rename no longer matches the database".into()),
            }
        }
        "trash-project" => {
            let row = sqlx::query("SELECT m.trashed_at, f.relative_path FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ? AND m.storage_mode = 'managed'")
                .bind(&payload.entity_id)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?
                .ok_or_else(|| "Interrupted trash project is unavailable".to_string())?;
            let relative: String = row.get("relative_path");
            if row.get::<Option<String>, _>("trashed_at").is_some()
                && relative == payload.new_relative
            {
                true
            } else if row.get::<Option<String>, _>("trashed_at").is_none()
                && relative == payload.old_relative
            {
                false
            } else {
                return Err("Interrupted trash operation no longer matches the database".into());
            }
        }
        "restore-project" => {
            let row = sqlx::query("SELECT m.trashed_at, f.relative_path FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id = ? AND m.storage_mode = 'managed'")
                .bind(&payload.entity_id)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?
                .ok_or_else(|| "Interrupted restored project is unavailable".to_string())?;
            let relative: String = row.get("relative_path");
            if row.get::<Option<String>, _>("trashed_at").is_none()
                && relative == payload.new_relative
            {
                true
            } else if row.get::<Option<String>, _>("trashed_at").is_some()
                && relative == payload.old_relative
            {
                false
            } else {
                return Err("Interrupted restore no longer matches the database".into());
            }
        }
        "delete-project" => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
                .bind(&payload.entity_id)
                .fetch_one(pool)
                .await
                .map_err(db_error)?;
            count == 0
        }
        _ => return Err("Interrupted path operation type is unsupported".into()),
    };
    if operation == "delete-project" && committed {
        if destination.is_dir() {
            std::fs::remove_dir_all(&destination)
                .map_err(|error| format!("Could not finish interrupted deletion: {error}"))?;
        } else if destination.exists() {
            std::fs::remove_file(&destination)
                .map_err(|error| format!("Could not finish interrupted deletion: {error}"))?;
        }
        return Ok(true);
    }
    if committed {
        if destination.exists() {
            return Ok(true);
        }
        if source.exists() {
            rename_case_safe(&source, &destination).map_err(|error| {
                format!("Could not finish interrupted filesystem operation: {error}")
            })?;
            return Ok(true);
        }
        return Err("Both paths from an interrupted committed operation are missing".into());
    }
    if same_path_case_insensitive(&source, &destination) && source != destination {
        if destination.exists() {
            rename_case_safe(&destination, &source).map_err(|error| {
                format!("Could not roll back interrupted case-only rename: {error}")
            })?;
        }
        return Ok(false);
    }
    match (source.exists(), destination.exists()) {
        (true, false) => Ok(false),
        (false, true) => {
            rename_case_safe(&destination, &source).map_err(|error| {
                format!("Could not roll back interrupted filesystem operation: {error}")
            })?;
            Ok(false)
        }
        (true, true) => Err("Both paths from an interrupted operation exist".into()),
        (false, false) => Err("Both paths from an interrupted operation are missing".into()),
    }
}

fn operation_path_is_managed(path: &Path, managed_roots: &[PathBuf]) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(canonical_parent) = parent.canonicalize() else {
        return false;
    };
    managed_roots.iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|canonical_root| canonical_parent.starts_with(canonical_root))
    })
}

fn rollback_uncommitted_files(
    payload: &FileOperationPayload,
    managed_roots: &[PathBuf],
) -> CommandResult<()> {
    for file in &payload.files {
        let source = PathBuf::from(&file.source);
        let destination = PathBuf::from(&file.destination);
        if !destination.exists() {
            continue;
        }
        if !source.is_file() {
            return Err(format!(
                "Interrupted import needs manual recovery because its source is missing: {}",
                source.display()
            ));
        }
        let destination_parent = destination
            .parent()
            .ok_or_else(|| "Interrupted import has an invalid destination".to_string())?
            .canonicalize()
            .map_err(|_| "Interrupted import destination is unavailable".to_string())?;
        let inside_managed_root = managed_roots.iter().any(|root| {
            root.canonicalize()
                .is_ok_and(|canonical| destination_parent.starts_with(canonical))
        });
        if !inside_managed_root {
            return Err("Interrupted import destination is outside the managed library".into());
        }
        if parsers::full_hash(&source)? != file.content_hash
            || parsers::full_hash(&destination)? != file.content_hash
        {
            return Err("Interrupted import files changed before recovery".into());
        }
        std::fs::remove_file(&destination).map_err(|error| {
            format!(
                "Could not roll back interrupted import file {}: {error}",
                destination.display()
            )
        })?;
    }
    Ok(())
}

async fn fail_job(id: &str, error: &str, pool: &SqlitePool) {
    let _ = sqlx::query(
        "UPDATE file_operation_jobs SET state = 'failed', error = ?, updated_at = ? WHERE id = ?",
    )
    .bind(error)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await;
}

async fn begin_path_operation(
    operation: &str,
    payload: &PathOperationPayload,
    pool: &SqlitePool,
) -> CommandResult<String> {
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    sqlx::query("INSERT INTO file_operation_jobs (id, operation, state, payload_json, created_at, updated_at) VALUES (?, ?, 'staging', ?, ?, ?)")
        .bind(&id)
        .bind(operation)
        .bind(serde_json::to_string(payload).map_err(|error| error.to_string())?)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(id)
}

async fn set_operation_state(
    id: &str,
    state: &str,
    error: Option<&str>,
    pool: &SqlitePool,
) -> CommandResult<()> {
    sqlx::query("UPDATE file_operation_jobs SET state = ?, error = ?, updated_at = ? WHERE id = ?")
        .bind(state)
        .bind(error)
        .bind(now())
        .bind(id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
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

    #[test]
    fn creates_portable_project_and_file_names() {
        assert_eq!(
            safe_component("Robot: arm", "Project").unwrap(),
            "Robot- arm"
        );
        assert!(safe_component("../Robot", "Project").is_err());
        assert_eq!(
            validate_renamed_file("Bracket", "stl").unwrap(),
            "Bracket.stl"
        );
        assert!(validate_renamed_file("Bracket.3mf", "stl").is_err());
    }

    #[test]
    fn reserves_windows_device_names() {
        assert_eq!(safe_component("CON", "Project").unwrap(), "CON-");
        assert_eq!(safe_component("LPT1", "Project").unwrap(), "LPT1-");
    }

    #[test]
    fn detects_overlapping_roots() {
        assert!(paths_overlap(
            Path::new("/models"),
            Path::new("/models/volum")
        ));
        assert!(!paths_overlap(Path::new("/models"), Path::new("/other")));
    }

    #[tokio::test]
    async fn recovers_project_and_asset_ids_from_a_managed_sidecar() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
        let root_id = Uuid::new_v4().to_string();
        let project_id = Uuid::new_v4().to_string();
        let asset_id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, ?, 'Managed', 'online', 'managed', ?, ?)")
            .bind(&root_id)
            .bind(library.path().to_string_lossy().to_string())
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(&state.pool)
            .await
            .unwrap();
        let project = library
            .path()
            .join(PROJECTS_DIRECTORY)
            .join("Recovered robot");
        std::fs::create_dir_all(project.join(MARKER_DIRECTORY)).unwrap();
        std::fs::write(project.join("arm.stl"), "solid arm\nendsolid").unwrap();
        let marker = ProjectMarker {
            format_version: 1,
            project_id: project_id.clone(),
            name: "Recovered robot".into(),
            files: vec![ProjectMarkerFile {
                asset_id: asset_id.clone(),
                path: "arm.stl".into(),
                role: "printable".into(),
            }],
            updated_at: timestamp,
        };
        std::fs::write(
            project.join(MARKER_DIRECTORY).join(MARKER_FILENAME),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();

        reconcile_managed_sidecars(&root_id, library.path(), &state.pool)
            .await
            .unwrap();

        let recovered_project: String =
            sqlx::query_scalar("SELECT id FROM models WHERE id = ? AND storage_mode = 'managed'")
                .bind(&project_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let recovered_asset: String = sqlx::query_scalar(
            "SELECT asset_id FROM model_assets WHERE model_id = ? AND asset_id = ?",
        )
        .bind(&project_id)
        .bind(&asset_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(recovered_project, project_id);
        assert_eq!(recovered_asset, asset_id);
    }

    #[tokio::test]
    async fn refuses_duplicate_project_ids_across_managed_sidecars() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
        let root_id = Uuid::new_v4().to_string();
        let project_id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, ?, 'Managed', 'online', 'managed', ?, ?)")
            .bind(&root_id)
            .bind(library.path().to_string_lossy().to_string())
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(&state.pool)
            .await
            .unwrap();
        for name in ["First", "Duplicate"] {
            let directory = library.path().join(PROJECTS_DIRECTORY).join(name);
            std::fs::create_dir_all(directory.join(MARKER_DIRECTORY)).unwrap();
            let marker = ProjectMarker {
                format_version: 1,
                project_id: project_id.clone(),
                name: name.into(),
                files: Vec::new(),
                updated_at: timestamp.clone(),
            };
            std::fs::write(
                directory.join(MARKER_DIRECTORY).join(MARKER_FILENAME),
                serde_json::to_vec(&marker).unwrap(),
            )
            .unwrap();
        }

        reconcile_managed_sidecars(&root_id, library.path(), &state.pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn moves_a_file_into_a_managed_project_with_a_completed_journal() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("bracket.stl");
        std::fs::write(&source, "solid bracket\nendsolid").unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
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
        let input = CreateManagedProjectInput {
            name: "Bracket".into(),
            file_paths: vec![source.to_string_lossy().to_string()],
            asset_ids: Vec::new(),
            transfer_mode: "move".into(),
        };

        let (project_id, _) = create_managed_project_internal(&input, &state)
            .await
            .unwrap();

        assert!(!source.exists());
        assert!(library
            .path()
            .join("Projects/Bracket/bracket.stl")
            .is_file());
        assert!(library
            .path()
            .join("Projects/Bracket/.volum/project.json")
            .is_file());
        let asset_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM model_assets WHERE model_id = ?")
                .bind(project_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let job_state: String = sqlx::query_scalar(
            "SELECT state FROM file_operation_jobs ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(asset_count, 1);
        assert_eq!(job_state, "complete");
    }

    #[tokio::test]
    async fn imports_a_companion_upload_without_exposing_its_staging_path() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let upload_directory = tempfile::tempdir().unwrap();
        let upload = upload_directory.path().join("phone-bracket.stl");
        std::fs::write(&upload, "solid phone-bracket\nendsolid").unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
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

        let imported = import_companion_upload(None, Some("Phone bracket"), &upload, &state)
            .await
            .unwrap();

        assert_eq!(imported.root_id, root_id);
        assert!(!upload.exists());
        assert!(library
            .path()
            .join("Projects/Phone bracket/phone-bracket.stl")
            .is_file());
        let stored: (String, String) = sqlx::query_as(
            "SELECT m.display_name, a.filename FROM models m JOIN model_assets ma ON ma.model_id = m.id JOIN assets a ON a.id = ma.asset_id WHERE m.id = ? AND a.id = ?",
        )
        .bind(&imported.project_id)
        .bind(&imported.asset_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(stored, ("Phone bracket".into(), "phone-bracket.stl".into()));
    }

    #[tokio::test]
    async fn creates_a_linked_project_from_an_external_file() {
        let data = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let source = external.path().join("linked.stl");
        std::fs::write(&source, "solid linked\nendsolid").unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        let input = CreateManagedProjectInput {
            name: "Linked external".into(),
            file_paths: vec![source.to_string_lossy().to_string()],
            asset_ids: Vec::new(),
            transfer_mode: "link".into(),
        };

        let project_id = create_linked_project(&input, &state).await.unwrap();

        let row = sqlx::query("SELECT m.storage_mode, r.path root_path, a.relative_path FROM models m JOIN model_assets ma ON ma.model_id = m.id JOIN assets a ON a.id = ma.asset_id JOIN library_roots r ON r.id = a.root_id WHERE m.id = ?")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("storage_mode"), "linked");
        assert_eq!(
            PathBuf::from(row.get::<String, _>("root_path")),
            external.path().canonicalize().unwrap()
        );
        assert_eq!(row.get::<String, _>("relative_path"), "linked.stl");
        assert!(source.is_file());
    }

    #[tokio::test]
    async fn rolls_back_an_interrupted_managed_package_import() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
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
        let input = CreateManagedProjectInput {
            name: "Interrupted".into(),
            file_paths: Vec::new(),
            asset_ids: Vec::new(),
            transfer_mode: "move".into(),
        };
        let project_id = Uuid::new_v4().to_string();
        create_managed_project_internal_with_id(&input, project_id.clone(), &state)
            .await
            .unwrap();
        let job_id = Uuid::new_v4().to_string();
        let staging = library
            .path()
            .join(STAGING_DIRECTORY)
            .join(format!("package-{job_id}"));
        std::fs::create_dir_all(&staging).unwrap();
        sqlx::query("INSERT INTO file_operation_jobs (id, operation, state, payload_json, created_at, updated_at) VALUES (?, 'import-package', 'committing', ?, ?, ?)")
            .bind(&job_id)
            .bind(serde_json::json!({ "projectId": project_id, "staging": staging.to_string_lossy() }).to_string())
            .bind(&timestamp)
            .bind(&timestamp)
            .execute(&state.pool)
            .await
            .unwrap();

        recover_pending_operations(&state.pool, data.path())
            .await
            .unwrap();

        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
            .bind(&project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let job_state: String =
            sqlx::query_scalar("SELECT state FROM file_operation_jobs WHERE id = ?")
                .bind(&job_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(project_count, 0);
        assert_eq!(job_state, "failed");
        assert!(!library.path().join("Projects/Interrupted").exists());
        assert!(!staging.exists());
    }

    #[tokio::test]
    async fn rolls_back_an_interrupted_managed_file_rename() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("part.stl");
        std::fs::write(&source, "solid part\nendsolid").unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
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
        let input = CreateManagedProjectInput {
            name: "Rename recovery".into(),
            file_paths: vec![source.to_string_lossy().to_string()],
            asset_ids: Vec::new(),
            transfer_mode: "copy".into(),
        };
        let (project_id, _) = create_managed_project_internal(&input, &state)
            .await
            .unwrap();
        let asset_id: String =
            sqlx::query_scalar("SELECT asset_id FROM model_assets WHERE model_id = ?")
                .bind(project_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let old_relative = "Projects/Rename recovery/part.stl".to_string();
        let new_relative = "Projects/Rename recovery/renamed.stl".to_string();
        let old_path = library.path().join(&old_relative);
        let new_path = library.path().join(&new_relative);
        std::fs::rename(&old_path, &new_path).unwrap();
        let job_id = begin_path_operation(
            "rename-asset",
            &PathOperationPayload {
                entity_id: asset_id.clone(),
                source: old_path.to_string_lossy().to_string(),
                destination: new_path.to_string_lossy().to_string(),
                old_relative: old_relative.clone(),
                new_relative,
            },
            &state.pool,
        )
        .await
        .unwrap();
        set_operation_state(&job_id, "committing", None, &state.pool)
            .await
            .unwrap();

        recover_pending_operations(&state.pool, data.path())
            .await
            .unwrap();

        let stored_relative: String =
            sqlx::query_scalar("SELECT relative_path FROM assets WHERE id = ?")
                .bind(asset_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let job_state: String =
            sqlx::query_scalar("SELECT state FROM file_operation_jobs WHERE id = ?")
                .bind(job_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(stored_relative, old_relative);
        assert!(old_path.is_file());
        assert!(!new_path.exists());
        assert_eq!(job_state, "failed");
    }

    #[tokio::test]
    async fn finishes_cleanup_after_a_committed_project_deletion() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        ensure_layout(library.path()).unwrap();
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
        let input = CreateManagedProjectInput {
            name: "Delete recovery".into(),
            file_paths: Vec::new(),
            asset_ids: Vec::new(),
            transfer_mode: "move".into(),
        };
        let project_id = Uuid::new_v4().to_string();
        create_managed_project_internal_with_id(&input, project_id.clone(), &state)
            .await
            .unwrap();
        let source = library.path().join("Projects/Delete recovery");
        let destination = library.path().join(".staging/delete-recovery");
        std::fs::rename(&source, &destination).unwrap();
        let job_id = begin_path_operation(
            "delete-project",
            &PathOperationPayload {
                entity_id: project_id.clone(),
                source: source.to_string_lossy().to_string(),
                destination: destination.to_string_lossy().to_string(),
                old_relative: "Projects/Delete recovery".into(),
                new_relative: ".staging/delete-recovery".into(),
            },
            &state.pool,
        )
        .await
        .unwrap();
        set_operation_state(&job_id, "cleanup", None, &state.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(&project_id)
            .execute(&state.pool)
            .await
            .unwrap();

        recover_pending_operations(&state.pool, data.path())
            .await
            .unwrap();

        let job_state: String =
            sqlx::query_scalar("SELECT state FROM file_operation_jobs WHERE id = ?")
                .bind(job_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(job_state, "complete");
        assert!(!destination.exists());
    }

    #[test]
    fn move_cleanup_refuses_to_delete_changed_sources() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.stl");
        let destination = directory.path().join("destination.stl");
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(&destination, b"original").unwrap();
        let hash = parsers::full_hash(&source).unwrap();
        std::fs::write(&source, b"changed").unwrap();
        let payload = FileOperationPayload {
            project_id: Uuid::new_v4().to_string(),
            mode: "move".into(),
            files: vec![FileOperationEntry {
                source: source.to_string_lossy().to_string(),
                destination: destination.to_string_lossy().to_string(),
                content_hash: hash,
            }],
        };
        assert!(cleanup_moved_sources(&payload).is_err());
        assert!(source.is_file());
    }
}
