use crate::{domain::ScanStatus, geometry, parsers, state::AppState};
use chrono::{DateTime, Utc};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, RecursiveMode, Watcher,
};
use sqlx::{Row, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use uuid::Uuid;
use walkdir::WalkDir;

const SUPPORTED: &[&str] = &["stl", "3mf", "obj", "step", "stp"];

#[derive(Clone)]
struct ScanContext {
    pool: SqlitePool,
    scans: Arc<RwLock<HashMap<String, ScanStatus>>>,
    pauses: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
}

#[derive(Debug)]
struct DiscoveredFile {
    absolute: PathBuf,
    relative: String,
    size: u64,
    modified: String,
    extension: String,
}

pub async fn start(root_id: String, state: &AppState, app: AppHandle) -> Result<(), String> {
    if state
        .scans
        .read()
        .await
        .get(&root_id)
        .is_some_and(|status| status.state == "scanning")
    {
        return Ok(());
    }
    let row = sqlx::query("SELECT path FROM library_roots WHERE id = ?")
        .bind(&root_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Library root not found".to_string())?;
    let root_path = PathBuf::from(row.get::<String, _>("path"));
    let pause = Arc::new(AtomicBool::new(false));
    state.pauses.write().await.insert(root_id.clone(), pause);
    let context = ScanContext {
        pool: state.pool.clone(),
        scans: state.scans.clone(),
        pauses: state.pauses.clone(),
    };
    if let Err(error) = ensure_watcher(&root_id, &root_path, state, context.clone(), app.clone()) {
        log::warn!(
            "Filesystem watcher unavailable for {}: {}",
            root_path.display(),
            error
        );
    }
    launch_scan(root_id, root_path, context, app).await;
    Ok(())
}

pub async fn pause(root_id: &str, state: &AppState) -> Result<(), String> {
    if let Some(flag) = state.pauses.read().await.get(root_id) {
        flag.store(true, Ordering::Relaxed);
    }
    if let Some(status) = state.scans.write().await.get_mut(root_id) {
        status.state = "paused".into();
    }
    Ok(())
}

pub async fn status(root_id: &str, state: &AppState) -> ScanStatus {
    state
        .scans
        .read()
        .await
        .get(root_id)
        .cloned()
        .unwrap_or_else(|| ScanStatus::idle(root_id))
}

async fn launch_scan(root_id: String, root_path: PathBuf, context: ScanContext, app: AppHandle) {
    let status = ScanStatus {
        root_id: root_id.clone(),
        state: "scanning".into(),
        discovered: 0,
        processed: 0,
        errors: 0,
        current_path: None,
        message: None,
    };
    context
        .scans
        .write()
        .await
        .insert(root_id.clone(), status.clone());
    let _ = app.emit("scan-progress", &status);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = scan_root(&root_id, &root_path, &context, &app).await {
            let mut scans = context.scans.write().await;
            let status = scans
                .entry(root_id.clone())
                .or_insert_with(|| ScanStatus::idle(&root_id));
            status.state = "error".into();
            status.message = Some(error);
            let _ = sqlx::query(
                "UPDATE library_roots SET status = 'error', updated_at = ? WHERE id = ?",
            )
            .bind(now())
            .bind(&root_id)
            .execute(&context.pool)
            .await;
            let _ = app.emit("scan-progress", status.clone());
            let _ = app.emit("root-status-changed", &root_id);
        }
    });
}

fn ensure_watcher(
    root_id: &str,
    root_path: &Path,
    state: &AppState,
    context: ScanContext,
    app: AppHandle,
) -> Result<(), String> {
    let mut watchers = state
        .watchers
        .lock()
        .map_err(|_| "Watcher lock poisoned".to_string())?;
    if watchers.contains_key(root_id) {
        return Ok(());
    }
    let watched_id = root_id.to_string();
    let watched_path = root_path.to_path_buf();
    let pending = Arc::new(AtomicBool::new(false));
    let mut watcher =
        notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
            let Ok(event) = result else {
                return;
            };
            if !event_requires_scan(&event.kind) || pending.swap(true, Ordering::SeqCst) {
                return;
            }
            let root_id = watched_id.clone();
            let root_path = watched_path.clone();
            let pending = pending.clone();
            let context = context.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(850)).await;
                pending.store(false, Ordering::SeqCst);
                if !context
                    .scans
                    .read()
                    .await
                    .get(&root_id)
                    .is_some_and(|status| status.state == "scanning")
                {
                    launch_scan(root_id, root_path, context, app).await;
                }
            });
        })
        .map_err(|error| error.to_string())?;
    watcher
        .watch(root_path, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    watchers.insert(root_id.to_string(), watcher);
    Ok(())
}

fn event_requires_scan(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

async fn scan_root(
    root_id: &str,
    root_path: &Path,
    context: &ScanContext,
    app: &AppHandle,
) -> Result<(), String> {
    if !root_path.is_dir() {
        sqlx::query("UPDATE library_roots SET status = 'offline', updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(root_id)
            .execute(&context.pool)
            .await
            .map_err(db_error)?;
        let mut scans = context.scans.write().await;
        let status = scans
            .entry(root_id.to_string())
            .or_insert_with(|| ScanStatus::idle(root_id));
        status.state = "error".into();
        status.message = Some("Library folder is unavailable".into());
        let _ = app.emit("scan-progress", status.clone());
        let _ = app.emit("root-status-changed", root_id);
        return Ok(());
    }
    sqlx::query("UPDATE library_roots SET status = 'scanning', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(root_id)
        .execute(&context.pool)
        .await
        .map_err(db_error)?;
    let path = root_path.to_path_buf();
    let files = tokio::task::spawn_blocking(move || discover(&path))
        .await
        .map_err(|error| error.to_string())??;
    {
        let mut scans = context.scans.write().await;
        let status = scans
            .entry(root_id.to_string())
            .or_insert_with(|| ScanStatus::idle(root_id));
        status.discovered = files.len() as u64;
        let _ = app.emit("scan-progress", status.clone());
    }
    let pause = context
        .pauses
        .read()
        .await
        .get(root_id)
        .cloned()
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let root_name = root_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Library")
        .to_string();
    let mut seen = HashSet::with_capacity(files.len());
    let mut folder_cache = HashMap::new();
    for (index, file) in files.into_iter().enumerate() {
        if pause.load(Ordering::Relaxed) {
            let mut scans = context.scans.write().await;
            let status = scans
                .entry(root_id.to_string())
                .or_insert_with(|| ScanStatus::idle(root_id));
            status.state = "paused".into();
            let _ = app.emit("scan-progress", status.clone());
            sqlx::query("UPDATE library_roots SET status = 'paused', updated_at = ? WHERE id = ?")
                .bind(now())
                .bind(root_id)
                .execute(&context.pool)
                .await
                .map_err(db_error)?;
            return Ok(());
        }
        seen.insert(file.relative.clone());
        let current_path = file.relative.clone();
        let result = index_file(
            root_id,
            root_path,
            &root_name,
            file,
            &context.pool,
            &mut folder_cache,
        )
        .await;
        let mut scans = context.scans.write().await;
        let status = scans
            .entry(root_id.to_string())
            .or_insert_with(|| ScanStatus::idle(root_id));
        status.processed = index as u64 + 1;
        status.current_path = Some(current_path);
        if let Err(error) = result {
            status.errors += 1;
            log::warn!("Indexing error: {error}");
        }
        if index % 8 == 0 || status.processed == status.discovered {
            let _ = app.emit("scan-progress", status.clone());
            let _ = app.emit("models-changed", root_id);
        }
    }
    reconcile_missing(root_id, &seen, &context.pool).await?;
    refresh_duplicate_hashes(&context.pool).await?;
    refresh_geometry_fingerprints(root_id, root_path, &context.pool).await?;
    let timestamp = now();
    sqlx::query(
        "UPDATE library_roots SET status = 'online', last_scan_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&timestamp)
    .bind(&timestamp)
    .bind(root_id)
    .execute(&context.pool)
    .await
    .map_err(db_error)?;
    refresh_all_search(root_id, &context.pool).await?;
    cleanup_thumbnail_cache(&context.pool, 1024 * 1024 * 1024).await?;
    let mut scans = context.scans.write().await;
    let status = scans
        .entry(root_id.to_string())
        .or_insert_with(|| ScanStatus::idle(root_id));
    status.state = "complete".into();
    status.current_path = None;
    let _ = app.emit("scan-progress", status.clone());
    let _ = app.emit("models-changed", root_id);
    let _ = app.emit("root-status-changed", root_id);
    Ok(())
}

fn discover(root: &Path) -> Result<Vec<DiscoveredFile>, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !ignored(entry.path(), root))
    {
        let entry = match entry {
            Ok(value) => value,
            Err(error) => {
                log::warn!("Walk error: {error}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !SUPPORTED.contains(&extension.as_str()) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let modified: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        files.push(DiscoveredFile {
            absolute: entry.path().to_path_buf(),
            relative,
            size: metadata.len(),
            modified: modified.to_rfc3339(),
            extension,
        });
    }
    files.sort_by(|a, b| {
        a.relative
            .to_ascii_lowercase()
            .cmp(&b.relative.to_ascii_lowercase())
    });
    Ok(files)
}

fn ignored(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    name.starts_with('.')
        || matches!(
            name,
            "node_modules" | "__MACOSX" | "$RECYCLE.BIN" | "System Volume Information"
        )
        || name.ends_with(".part")
        || name.ends_with(".tmp")
}

async fn index_file(
    root_id: &str,
    root_path: &Path,
    root_name: &str,
    file: DiscoveredFile,
    pool: &SqlitePool,
    folder_cache: &mut HashMap<String, String>,
) -> Result<(), String> {
    let parent = Path::new(&file.relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let folder_rel = if parent == Path::new(".") {
        "".to_string()
    } else {
        parent.to_string_lossy().replace('\\', "/")
    };
    let folder_id = ensure_folder(root_id, &folder_rel, root_name, pool, folder_cache).await?;
    let filename = Path::new(&file.relative)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&file.relative)
        .to_string();
    let display_name = Path::new(&filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&filename)
        .replace(['_', '-'], " ");
    let grouping_key = normalize_grouping_key(&display_name);
    let version_key = normalize_version_key(&display_name);
    let parse_path = file.absolute.clone();
    let parse_extension = file.extension.clone();
    let parse_size = file.size;
    let (fingerprint, metadata) = tokio::task::spawn_blocking(move || {
        (
            parsers::partial_fingerprint(&parse_path, parse_size).ok(),
            parsers::parse_metadata(&parse_path, &parse_extension),
        )
    })
    .await
    .map_err(|error| error.to_string())?;
    let metadata_json = serde_json::to_string(&metadata).map_err(|error| error.to_string())?;
    let parse_status = if metadata.warning.is_some() {
        "warning"
    } else {
        "ready"
    };
    let existing = sqlx::query("SELECT id FROM assets WHERE root_id = ? AND relative_path = ?")
        .bind(root_id)
        .bind(&file.relative)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    let mut asset_id = existing.map(|row| row.get::<String, _>("id"));
    let mut preserved_model_id = None;
    let mut content_hash = None;
    if asset_id.is_none() {
        if let Some(fingerprint_value) = &fingerprint {
            let candidates = sqlx::query("SELECT a.id, a.relative_path, a.content_hash, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.root_id = ? AND a.partial_fingerprint = ? AND a.byte_size = ? AND a.relative_path != ?")
                .bind(root_id).bind(fingerprint_value).bind(file.size as i64).bind(&file.relative).fetch_all(pool).await.map_err(db_error)?;
            for candidate in candidates {
                let old_relative = candidate.get::<String, _>("relative_path");
                if !root_path.join(&old_relative).exists() {
                    let candidate_hash: Option<String> = candidate.get("content_hash");
                    let new_hash = parsers::full_hash(&file.absolute).ok();
                    if candidate_hash.is_some() && new_hash != candidate_hash {
                        continue;
                    }
                    content_hash = new_hash;
                    asset_id = Some(candidate.get("id"));
                    preserved_model_id = candidate.try_get("model_id").ok();
                    break;
                }
            }
        }
    }
    let asset_id = asset_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at, partial_fingerprint, content_hash, parse_status, metadata_json, missing_since) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL) ON CONFLICT(id) DO UPDATE SET folder_id = excluded.folder_id, relative_path = excluded.relative_path, filename = excluded.filename, extension = excluded.extension, byte_size = excluded.byte_size, content_hash = CASE WHEN excluded.partial_fingerprint = assets.partial_fingerprint AND excluded.byte_size = assets.byte_size AND excluded.modified_at = assets.modified_at THEN COALESCE(excluded.content_hash, assets.content_hash) ELSE excluded.content_hash END, modified_at = excluded.modified_at, partial_fingerprint = excluded.partial_fingerprint, parse_status = excluded.parse_status, metadata_json = excluded.metadata_json, missing_since = NULL")
        .bind(&asset_id).bind(root_id).bind(&folder_id).bind(&file.relative).bind(&filename).bind(&file.extension).bind(file.size as i64).bind(&file.modified).bind(&fingerprint).bind(&content_hash).bind(parse_status).bind(&metadata_json).execute(pool).await.map_err(db_error)?;
    let assigned = sqlx::query("SELECT ma.model_id, m.bundle_mode FROM model_assets ma JOIN models m ON m.id = ma.model_id WHERE ma.asset_id = ?")
        .bind(&asset_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    let assigned_mode = assigned
        .as_ref()
        .map(|row| row.get::<String, _>("bundle_mode"));
    if preserved_model_id.is_none() {
        preserved_model_id = assigned
            .as_ref()
            .map(|row| row.get::<String, _>("model_id"));
    }
    let model_id = if let Some(id) = preserved_model_id {
        let conflict = sqlx::query(
            "SELECT id FROM models WHERE folder_id = ? AND grouping_key = ? AND id != ?",
        )
        .bind(&folder_id)
        .bind(&grouping_key)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
        if assigned_mode.as_deref() == Some("manual") {
            sqlx::query("UPDATE models SET grouping_key = ? WHERE id = ?")
                .bind(format!("manual:{id}"))
                .bind(&id)
                .execute(pool)
                .await
                .map_err(db_error)?;
        } else if conflict.is_none() {
            sqlx::query("UPDATE models SET folder_id = ?, grouping_key = ?, version_key = ?, display_name = ?, missing_since = NULL, updated_at = ? WHERE id = ?").bind(&folder_id).bind(&grouping_key).bind(&version_key).bind(&display_name).bind(now()).bind(&id).execute(pool).await.map_err(db_error)?;
        }
        id
    } else {
        sqlx::query("UPDATE models SET grouping_key = 'manual:' || id WHERE folder_id = ? AND grouping_key = ? AND bundle_mode = 'manual'")
            .bind(&folder_id)
            .bind(&grouping_key)
            .execute(pool)
            .await
            .map_err(db_error)?;
        if let Some(row) =
        sqlx::query("SELECT id FROM models WHERE folder_id = ? AND grouping_key = ? AND bundle_mode = 'automatic'")
            .bind(&folder_id)
            .bind(&grouping_key)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?
        {
            let id: String = row.get("id");
            sqlx::query("UPDATE models SET version_key = ? WHERE id = ?")
                .bind(&version_key)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(db_error)?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            let timestamp = now();
            sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, added_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)").bind(&id).bind(&folder_id).bind(&display_name).bind(&grouping_key).bind(&version_key).bind(&timestamp).bind(&timestamp).execute(pool).await.map_err(db_error)?;
            id
        }
    };
    let asset_role = match file.extension.as_str() {
        "step" | "stp" => "source",
        "3mf" => "plate",
        _ => "printable",
    };
    sqlx::query("INSERT OR IGNORE INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, ?, 0)").bind(&model_id).bind(&asset_id).bind(asset_role).execute(pool).await.map_err(db_error)?;
    sqlx::query("UPDATE model_assets SET role = ? WHERE asset_id = ?")
        .bind(asset_role)
        .bind(&asset_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    let project = sqlx::query("SELECT m.bundle_mode, EXISTS (SELECT 1 FROM assets current WHERE current.id = m.primary_asset_id AND current.missing_since IS NULL) primary_available FROM models m WHERE m.id = ?")
        .bind(&model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let preserve_primary = project.get::<String, _>("bundle_mode") == "manual"
        && project.get::<i64, _>("primary_available") != 0;
    if !preserve_primary {
        let primary = sqlx::query("SELECT a.id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.missing_since IS NULL ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 WHEN 'step' THEN 3 WHEN 'stp' THEN 3 ELSE 4 END, a.filename LIMIT 1")
            .bind(&model_id)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;
        if let Some(primary) = primary {
            sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
                .bind(primary.get::<String, _>("id"))
                .bind(&model_id)
                .execute(pool)
                .await
                .map_err(db_error)?;
        }
    }
    sqlx::query("UPDATE models SET missing_since = NULL, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&model_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    refresh_search(&model_id, pool).await?;
    Ok(())
}

async fn refresh_geometry_fingerprints(
    root_id: &str,
    root_path: &Path,
    pool: &SqlitePool,
) -> Result<(), String> {
    let rows = sqlx::query("SELECT a.id, a.relative_path, a.extension, a.partial_fingerprint, a.byte_size, a.modified_at, g.source_fingerprint FROM assets a LEFT JOIN asset_geometry g ON g.asset_id = a.id WHERE a.root_id = ? AND a.missing_since IS NULL")
        .bind(root_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    for row in rows {
        let asset_id: String = row.get("id");
        let revision = format!(
            "{}:{}:{}",
            row.get::<Option<String>, _>("partial_fingerprint")
                .unwrap_or_default(),
            row.get::<i64, _>("byte_size"),
            row.get::<String, _>("modified_at")
        );
        if row
            .get::<Option<String>, _>("source_fingerprint")
            .as_deref()
            == Some(revision.as_str())
        {
            continue;
        }
        let path = root_path.join(row.get::<String, _>("relative_path"));
        let extension: String = row.get("extension");
        let analysis =
            tokio::task::spawn_blocking(move || geometry::analyze_file(&path, &extension))
                .await
                .map_err(|error| error.to_string())?;
        match analysis {
            Ok(analysis) => {
                let dimensions = serde_json::to_string(&analysis.dimensions_mm)
                    .map_err(|error| error.to_string())?;
                let descriptor =
                    serde_json::to_string(&analysis).map_err(|error| error.to_string())?;
                sqlx::query("INSERT INTO asset_geometry (asset_id, source_fingerprint, algorithm_version, geometry_hash, similarity_key, dimensions_json, surface_area, volume, triangle_count, descriptor_json, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(asset_id) DO UPDATE SET source_fingerprint = excluded.source_fingerprint, algorithm_version = excluded.algorithm_version, geometry_hash = excluded.geometry_hash, similarity_key = excluded.similarity_key, dimensions_json = excluded.dimensions_json, surface_area = excluded.surface_area, volume = excluded.volume, triangle_count = excluded.triangle_count, descriptor_json = excluded.descriptor_json, updated_at = excluded.updated_at")
                    .bind(&asset_id)
                    .bind(&revision)
                    .bind(geometry::ALGORITHM_VERSION)
                    .bind(&analysis.geometry_hash)
                    .bind(&analysis.similarity_key)
                    .bind(dimensions)
                    .bind(analysis.surface_area)
                    .bind(analysis.volume)
                    .bind(analysis.triangle_count as i64)
                    .bind(descriptor)
                    .bind(now())
                    .execute(pool)
                    .await
                    .map_err(db_error)?;
            }
            Err(error) => {
                sqlx::query("DELETE FROM asset_geometry WHERE asset_id = ?")
                    .bind(&asset_id)
                    .execute(pool)
                    .await
                    .map_err(db_error)?;
                log::warn!("Geometry analysis failed for {asset_id}: {error}");
            }
        }
    }
    Ok(())
}

async fn ensure_folder(
    root_id: &str,
    relative: &str,
    root_name: &str,
    pool: &SqlitePool,
    cache: &mut HashMap<String, String>,
) -> Result<String, String> {
    if let Some(id) = cache.get(relative) {
        return Ok(id.clone());
    }
    let mut current = String::new();
    let mut parent_id: Option<String> = None;
    let segments: Vec<&str> = if relative.is_empty() {
        vec![]
    } else {
        relative.split('/').collect()
    };
    let levels = if segments.is_empty() {
        1
    } else {
        segments.len()
    };
    for index in 0..levels {
        if !segments.is_empty() {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(segments[index]);
        }
        if let Some(id) = cache.get(&current) {
            parent_id = Some(id.clone());
            continue;
        }
        let name = if current.is_empty() {
            root_name.to_string()
        } else {
            segments[index].to_string()
        };
        let existing =
            sqlx::query("SELECT id FROM folders WHERE root_id = ? AND relative_path = ?")
                .bind(root_id)
                .bind(&current)
                .fetch_optional(pool)
                .await
                .map_err(db_error)?;
        let id = existing
            .map(|row| row.get("id"))
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, ?, ?, ?, ?)").bind(&id).bind(root_id).bind(&parent_id).bind(&current).bind(&name).bind(now()).execute(pool).await.map_err(db_error)?;
        cache.insert(current.clone(), id.clone());
        parent_id = Some(id);
    }
    parent_id.ok_or_else(|| "Unable to create folder".into())
}

async fn reconcile_missing(
    root_id: &str,
    seen: &HashSet<String>,
    pool: &SqlitePool,
) -> Result<(), String> {
    let rows = sqlx::query("SELECT id, relative_path FROM assets WHERE root_id = ?")
        .bind(root_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let timestamp = now();
    for row in rows {
        let relative = row.get::<String, _>("relative_path");
        if !seen.contains(&relative) {
            sqlx::query(
                "UPDATE assets SET missing_since = COALESCE(missing_since, ?) WHERE id = ?",
            )
            .bind(&timestamp)
            .bind(row.get::<String, _>("id"))
            .execute(pool)
            .await
            .map_err(db_error)?;
        }
    }
    sqlx::query("UPDATE models SET missing_since = COALESCE(missing_since, ?) WHERE id IN (SELECT m.id FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = ? AND NOT EXISTS (SELECT 1 FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = m.id AND a.missing_since IS NULL))").bind(&timestamp).bind(root_id).execute(pool).await.map_err(db_error)?;
    sqlx::query("UPDATE models SET missing_since = NULL WHERE id IN (SELECT ma.model_id FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE a.root_id = ? AND a.missing_since IS NULL)").bind(root_id).execute(pool).await.map_err(db_error)?;
    Ok(())
}

pub async fn refresh_search(model_id: &str, pool: &SqlitePool) -> Result<(), String> {
    let row = sqlx::query("SELECT m.display_name, m.notes, f.relative_path, COALESCE(group_concat(DISTINCT a.filename), ''), COALESCE(group_concat(DISTINCT a.extension), '') FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN model_assets ma ON ma.model_id = m.id LEFT JOIN assets a ON a.id = ma.asset_id WHERE m.id = ? GROUP BY m.id").bind(model_id).fetch_optional(pool).await.map_err(db_error)?;
    let Some(row) = row else {
        return Ok(());
    };
    let collections: String = sqlx::query_scalar("SELECT COALESCE(group_concat(c.name), '') FROM collections c JOIN collection_items ci ON ci.collection_id = c.id WHERE ci.model_id = ?").bind(model_id).fetch_one(pool).await.map_err(db_error)?;
    let tags: String = sqlx::query_scalar("SELECT COALESCE(group_concat(t.name), '') FROM tags t JOIN model_tags mt ON mt.tag_id = t.id WHERE mt.model_id = ?").bind(model_id).fetch_one(pool).await.map_err(db_error)?;
    let searchable_groups = if tags.is_empty() {
        collections
    } else {
        format!("{collections},{tags}")
    };
    sqlx::query("DELETE FROM model_search WHERE model_id = ?")
        .bind(model_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO model_search (model_id, display_name, filenames, folder_path, collections, notes, formats) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(model_id).bind(row.get::<String, _>(0)).bind(row.get::<String, _>(3)).bind(row.get::<String, _>(2)).bind(searchable_groups).bind(row.get::<String, _>(1)).bind(row.get::<String, _>(4)).execute(pool).await.map_err(db_error)?;
    Ok(())
}

async fn refresh_all_search(root_id: &str, pool: &SqlitePool) -> Result<(), String> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT m.id FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = ?",
    )
    .bind(root_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    for id in ids {
        refresh_search(&id, pool).await?;
    }
    Ok(())
}

async fn cleanup_thumbnail_cache(pool: &SqlitePool, max_bytes: u64) -> Result<(), String> {
    let rows = sqlx::query("SELECT id, cache_path FROM thumbnails ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut retained = 0_u64;
    for row in rows {
        let id: String = row.get("id");
        let path = PathBuf::from(row.get::<String, _>("cache_path"));
        let size = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if size == 0 || retained.saturating_add(size) > max_bytes {
            let _ = std::fs::remove_file(path);
            sqlx::query("DELETE FROM thumbnails WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await
                .map_err(db_error)?;
        } else {
            retained += size;
        }
    }
    Ok(())
}

fn normalize_grouping_key(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_version_key(value: &str) -> String {
    let normalized = value
        .to_lowercase()
        .replace(['_', '-', '(', ')', '[', ']'], " ");
    let mut tokens: Vec<&str> = normalized.split_whitespace().collect();
    while let Some(token) = tokens.last().copied() {
        let trimmed = token.trim_matches(|character: char| character == '.' || character == '_');
        let version_number = trimmed
            .strip_prefix('v')
            .or_else(|| trimmed.strip_prefix("ver"))
            .or_else(|| trimmed.strip_prefix("version"))
            .or_else(|| trimmed.strip_prefix("rev"))
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .chars()
                        .all(|character| character.is_ascii_digit() || character == '.')
            });
        let removable = version_number
            || matches!(
                trimmed,
                "final" | "fixed" | "repair" | "repaired" | "copy" | "backup" | "new" | "old"
            );
        if !removable {
            break;
        }
        tokens.pop();
    }
    let key = tokens.join(" ");
    if key.is_empty() {
        normalize_grouping_key(value)
    } else {
        key
    }
}

async fn refresh_duplicate_hashes(pool: &SqlitePool) -> Result<(), String> {
    let rows = sqlx::query("SELECT a.id, r.path root_path, a.relative_path FROM assets a JOIN library_roots r ON r.id = a.root_id JOIN (SELECT partial_fingerprint, byte_size FROM assets WHERE missing_since IS NULL AND partial_fingerprint IS NOT NULL GROUP BY partial_fingerprint, byte_size HAVING COUNT(*) > 1) duplicate_candidates ON duplicate_candidates.partial_fingerprint = a.partial_fingerprint AND duplicate_candidates.byte_size = a.byte_size WHERE a.missing_since IS NULL AND a.content_hash IS NULL")
        .fetch_all(pool).await.map_err(db_error)?;
    for row in rows {
        let id: String = row.get("id");
        let path = PathBuf::from(row.get::<String, _>("root_path"))
            .join(row.get::<String, _>("relative_path"));
        let hash = tokio::task::spawn_blocking(move || parsers::full_hash(&path))
            .await
            .map_err(|error| error.to_string())??;
        sqlx::query("UPDATE assets SET content_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
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
    use crate::state::AppState;
    use notify::event::{CreateKind, ModifyKind};

    #[test]
    fn watcher_ignores_access_events_from_its_own_scan() {
        assert!(!event_requires_scan(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
        assert!(!event_requires_scan(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(event_requires_scan(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(event_requires_scan(&EventKind::Create(CreateKind::File)));
        assert!(event_requires_scan(&EventKind::Modify(ModifyKind::Any)));
    }

    #[test]
    fn grouping_is_stable_for_separators() {
        assert_eq!(
            normalize_grouping_key("Tool Wall  Bracket"),
            "tool wall bracket"
        );
    }

    #[test]
    fn version_keys_remove_only_common_revision_suffixes() {
        assert_eq!(normalize_version_key("Tool-Wall_v12 final"), "tool wall");
        assert_eq!(
            normalize_version_key("MK3 camera mount"),
            "mk3 camera mount"
        );
    }

    #[test]
    fn discovery_ignores_zip_archives() {
        let library = tempfile::tempdir().unwrap();
        std::fs::write(library.path().join("model.zip"), b"archive").unwrap();
        std::fs::write(library.path().join("model.STL"), b"solid model").unwrap();

        let files = discover(library.path()).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative, "model.STL");
        assert_eq!(files[0].extension, "stl");
    }

    #[tokio::test]
    async fn indexes_and_groups_variants() {
        let data = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        std::fs::write(library.path().join("hook.stl"), "solid hook\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 5 2\nendloop\nendfacet\nendsolid").unwrap();
        std::fs::write(
            library.path().join("hook.obj"),
            "v 0 0 0\nv 10 0 0\nv 0 5 2\nf 1 2 3\n",
        )
        .unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        let root_id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, created_at, updated_at) VALUES (?, ?, 'Models', 'online', ?, ?)")
            .bind(&root_id).bind(library.path().to_string_lossy().to_string()).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        let files = discover(library.path()).unwrap();
        let mut cache = HashMap::new();
        for file in files {
            index_file(
                &root_id,
                library.path(),
                "Models",
                file,
                &state.pool,
                &mut cache,
            )
            .await
            .unwrap();
        }
        let model_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let search_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM model_search WHERE model_search MATCH '\"hook\"*'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(model_count, 1);
        assert_eq!(asset_count, 2);
        assert_eq!(search_count, 1);

        let obj_asset: String = sqlx::query_scalar("SELECT id FROM assets WHERE extension = 'obj'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let folder_id: String = sqlx::query_scalar("SELECT folder_id FROM assets WHERE id = ?")
            .bind(&obj_asset)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let manual_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, bundle_mode, added_at, updated_at) VALUES (?, ?, 'Hook source', ?, 'hook', 'manual', ?, ?)")
            .bind(&manual_id).bind(&folder_id).bind(format!("manual:{manual_id}")).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("UPDATE model_assets SET model_id = ? WHERE asset_id = ?")
            .bind(&manual_id)
            .bind(&obj_asset)
            .execute(&state.pool)
            .await
            .unwrap();
        let stl_asset: String = sqlx::query_scalar("SELECT id FROM assets WHERE extension = 'stl'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let automatic_id: String =
            sqlx::query_scalar("SELECT model_id FROM model_assets WHERE asset_id = ?")
                .bind(&stl_asset)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        sqlx::query("UPDATE model_assets SET model_id = ? WHERE asset_id = ?")
            .bind(&manual_id)
            .bind(&stl_asset)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
            .bind(&obj_asset)
            .bind(&manual_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(automatic_id)
            .execute(&state.pool)
            .await
            .unwrap();
        let obj_file = discover(library.path())
            .unwrap()
            .into_iter()
            .find(|file| file.extension == "obj")
            .unwrap();
        index_file(
            &root_id,
            library.path(),
            "Models",
            obj_file,
            &state.pool,
            &mut cache,
        )
        .await
        .unwrap();
        let owner: String =
            sqlx::query_scalar("SELECT model_id FROM model_assets WHERE asset_id = ?")
                .bind(&obj_asset)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(owner, manual_id);
        let stl_file = discover(library.path())
            .unwrap()
            .into_iter()
            .find(|file| file.extension == "stl")
            .unwrap();
        index_file(
            &root_id,
            library.path(),
            "Models",
            stl_file,
            &state.pool,
            &mut cache,
        )
        .await
        .unwrap();
        let primary: String =
            sqlx::query_scalar("SELECT primary_asset_id FROM models WHERE id = ?")
                .bind(&manual_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(primary, obj_asset);
    }
}
