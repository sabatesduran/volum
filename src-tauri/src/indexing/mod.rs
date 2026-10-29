use crate::{
    domain::ScanStatus, geometry, parsers, project_docs, project_storage, state::AppState,
};
use chrono::{DateTime, Utc};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, RecursiveMode, Watcher,
};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use walkdir::WalkDir;

const SUPPORTED: &[&str] = &["stl", "3mf", "obj", "step", "stp"];

#[derive(Clone)]
struct ScanContext {
    pool: SqlitePool,
    scans: Arc<RwLock<HashMap<String, ScanStatus>>>,
    pauses: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    file_operation_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct DiscoveredFile {
    absolute: PathBuf,
    relative: String,
    size: u64,
    modified: String,
    extension: String,
}

#[derive(Debug)]
struct ExistingAsset {
    byte_size: u64,
    modified_at: String,
    missing: bool,
    model_id: Option<String>,
}

#[derive(Debug)]
struct IgnoredAssetPath {
    id: String,
    content_hash: Option<String>,
    byte_size: Option<u64>,
    modified_at: Option<String>,
}

#[derive(Debug)]
struct IndexOutcome {
    asset_id: String,
    affected_model_ids: Vec<String>,
}

pub async fn start(
    root_id: String,
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), String> {
    if state
        .scans
        .read()
        .await
        .get(&root_id)
        .is_some_and(|status| status.state == "scanning")
    {
        return Ok(());
    }
    let row = sqlx::query("SELECT path, root_kind FROM library_roots WHERE id = ?")
        .bind(&root_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Library root not found".to_string())?;
    let root_path = PathBuf::from(row.get::<String, _>("path"));
    let root_kind: String = row.get("root_kind");
    let pause = Arc::new(AtomicBool::new(false));
    state.pauses.write().await.insert(root_id.clone(), pause);
    let context = ScanContext {
        pool: state.pool.clone(),
        scans: state.scans.clone(),
        pauses: state.pauses.clone(),
        file_operation_lock: state.file_operation_lock.clone(),
    };
    if let Err(error) = ensure_watcher(
        &root_id,
        &root_path,
        &root_kind,
        state,
        context.clone(),
        app.clone(),
    ) {
        log::warn!(
            "Filesystem watcher unavailable for {}: {}",
            root_path.display(),
            error
        );
    }
    launch_scan(root_id, root_path, root_kind, context, app).await;
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

async fn launch_scan(
    root_id: String,
    root_path: PathBuf,
    root_kind: String,
    context: ScanContext,
    app: Option<AppHandle>,
) {
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
    if let Some(app) = &app {
        let _ = app.emit("scan-progress", &status);
    }
    tokio::spawn(async move {
        if let Err(error) = scan_root(&root_id, &root_path, &root_kind, &context, &app).await {
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
            if let Some(app) = &app {
                let _ = app.emit("scan-progress", status.clone());
                let _ = app.emit("root-status-changed", &root_id);
            }
        }
    });
}

fn ensure_watcher(
    root_id: &str,
    root_path: &Path,
    root_kind: &str,
    state: &AppState,
    context: ScanContext,
    app: Option<AppHandle>,
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
    let watched_kind = root_kind.to_string();
    let pending = Arc::new(AtomicBool::new(false));
    let runtime = tokio::runtime::Handle::current();
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
            let root_kind = watched_kind.clone();
            let pending = pending.clone();
            let context = context.clone();
            let app = app.clone();
            runtime.spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(850)).await;
                pending.store(false, Ordering::SeqCst);
                if !context
                    .scans
                    .read()
                    .await
                    .get(&root_id)
                    .is_some_and(|status| status.state == "scanning")
                {
                    launch_scan(root_id, root_path, root_kind, context, app).await;
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
    root_kind: &str,
    context: &ScanContext,
    app: &Option<AppHandle>,
) -> Result<(), String> {
    let scan_started = Instant::now();
    let _file_guard = context.file_operation_lock.lock().await;
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
        if let Some(app) = app {
            let _ = app.emit("scan-progress", status.clone());
            let _ = app.emit("root-status-changed", root_id);
        }
        return Ok(());
    }
    sqlx::query("UPDATE library_roots SET status = 'scanning', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(root_id)
        .execute(&context.pool)
        .await
        .map_err(db_error)?;
    let path = if root_kind == "managed" {
        root_path.join("Projects")
    } else {
        root_path.to_path_buf()
    };
    let managed = root_kind == "managed";
    if managed {
        project_storage::reconcile_managed_sidecars(root_id, root_path, &context.pool).await?;
    }
    let files = tokio::task::spawn_blocking(move || {
        let mut files = discover(&path)?;
        if managed {
            for file in &mut files {
                file.relative = format!("Projects/{}", file.relative);
            }
        }
        Ok::<Vec<DiscoveredFile>, String>(files)
    })
    .await
    .map_err(|error| error.to_string())??;
    {
        let mut scans = context.scans.write().await;
        let status = scans
            .entry(root_id.to_string())
            .or_insert_with(|| ScanStatus::idle(root_id));
        status.state = "scanning".into();
        status.discovered = files.len() as u64;
        status.processed = 0;
        status.errors = 0;
        status.current_path = None;
        status.message = None;
        if let Some(app) = app {
            let _ = app.emit("scan-progress", status.clone());
        }
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
    let existing_assets = sqlx::query("SELECT a.relative_path, a.byte_size, a.modified_at, a.missing_since, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.root_id = ?")
        .bind(root_id)
        .fetch_all(&context.pool)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("relative_path"),
                ExistingAsset {
                    byte_size: row.get::<i64, _>("byte_size").max(0) as u64,
                    modified_at: row.get("modified_at"),
                    missing: row.get::<Option<String>, _>("missing_since").is_some(),
                    model_id: row.try_get("model_id").ok(),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let ignored_paths = if managed {
        HashMap::new()
    } else {
        sqlx::query(
            "SELECT id, relative_path, content_hash, byte_size, modified_at FROM ignored_asset_paths WHERE root_id = ?",
        )
        .bind(root_id)
        .fetch_all(&context.pool)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|row| {
                (row.get::<String, _>("relative_path"), IgnoredAssetPath {
                    id: row.get("id"),
                    content_hash: row.get("content_hash"),
                    byte_size: row.get::<Option<i64>, _>("byte_size").map(|size| size.max(0) as u64),
                    modified_at: row.get("modified_at"),
                })
        })
        .collect::<HashMap<_, _>>()
    };
    let managed_projects = if managed {
        sqlx::query("SELECT m.id, f.relative_path FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = ? AND m.storage_mode = 'managed' AND m.trashed_at IS NULL")
            .bind(root_id)
            .fetch_all(&context.pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(|row| (row.get::<String, _>("relative_path"), row.get::<String, _>("id")))
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let discovered_count = files.len();
    let mut changed_models = HashSet::new();
    let mut changed_assets = HashSet::new();
    let mut changed_since_emit = 0_usize;
    let mut last_models_emit = Instant::now();
    let mut scan_errors = 0_u64;
    let mut unchanged_files = 0_usize;
    for (index, file) in files.into_iter().enumerate() {
        if pause.load(Ordering::Relaxed) {
            let mut scans = context.scans.write().await;
            let status = scans
                .entry(root_id.to_string())
                .or_insert_with(|| ScanStatus::idle(root_id));
            status.state = "paused".into();
            status.processed = index as u64;
            status.errors = scan_errors;
            if let Some(app) = app {
                let _ = app.emit("scan-progress", status.clone());
            }
            sqlx::query("UPDATE library_roots SET status = 'paused', updated_at = ? WHERE id = ?")
                .bind(now())
                .bind(root_id)
                .execute(&context.pool)
                .await
                .map_err(db_error)?;
            return Ok(());
        }
        let managed_project_id = if managed {
            managed_project_for_file(&file.relative, &managed_projects).cloned()
        } else {
            None
        };
        let should_skip = managed && managed_project_id.is_none();
        let should_ignore = !should_skip
            && !managed
            && should_ignore_discovered_path(
                &file,
                ignored_paths.get(&file.relative),
                &context.pool,
            )
            .await?;
        if should_ignore {
            seen.insert(file.relative.clone());
        }
        let current_path = file.relative.clone();
        let unchanged = should_skip
            || should_ignore
            || asset_is_unchanged(
                existing_assets.get(&file.relative),
                &file,
                managed_project_id.as_deref(),
                managed,
            );
        if unchanged {
            unchanged_files += 1;
        }
        if !should_skip && !should_ignore {
            seen.insert(file.relative.clone());
        }
        let result = if unchanged {
            None
        } else {
            Some(
                index_file(
                    root_id,
                    root_path,
                    &root_name,
                    file,
                    &context.pool,
                    &mut folder_cache,
                    managed_project_id.as_deref(),
                    None,
                )
                .await,
            )
        };
        if let Some(result) = result {
            match result {
                Ok(outcome) => {
                    changed_models.extend(outcome.affected_model_ids);
                    changed_assets.insert(outcome.asset_id);
                    changed_since_emit += 1;
                }
                Err(error) => {
                    scan_errors += 1;
                    log::warn!("Indexing error: {error}");
                }
            }
        }
        if index % 32 == 0 || index + 1 == discovered_count {
            let snapshot = {
                let mut scans = context.scans.write().await;
                let status = scans
                    .entry(root_id.to_string())
                    .or_insert_with(|| ScanStatus::idle(root_id));
                status.processed = index as u64 + 1;
                status.errors = scan_errors;
                status.current_path = Some(current_path);
                status.clone()
            };
            if let Some(app) = app {
                let _ = app.emit("scan-progress", snapshot);
            }
        }
        if changed_since_emit >= 64 && last_models_emit.elapsed() >= Duration::from_millis(500) {
            if let Some(app) = app {
                let _ = app.emit("models-changed", root_id);
            }
            changed_since_emit = 0;
            last_models_emit = Instant::now();
        }
    }
    for (relative, existing) in &existing_assets {
        if !existing.missing && !seen.contains(relative) {
            if let Some(model_id) = &existing.model_id {
                changed_models.insert(model_id.clone());
            }
        }
    }
    reconcile_missing(root_id, &seen, &context.pool).await?;
    if managed {
        for project_id in &changed_models {
            if let Err(error) =
                project_storage::write_project_marker(project_id, &context.pool).await
            {
                log::warn!("Could not refresh managed project sidecar for {project_id}: {error}");
            }
        }
    }
    if !changed_models.is_empty() {
        refresh_duplicate_hashes(&context.pool).await?;
        refresh_geometry_fingerprints(root_id, root_path, &changed_assets, &context.pool).await?;
        refresh_search_many(&changed_models, &context.pool).await?;
    }
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
    cleanup_thumbnail_cache(&context.pool, 1024 * 1024 * 1024).await?;
    let mut scans = context.scans.write().await;
    let status = scans
        .entry(root_id.to_string())
        .or_insert_with(|| ScanStatus::idle(root_id));
    status.state = "complete".into();
    status.current_path = None;
    if let Some(app) = app {
        let _ = app.emit("scan-progress", status.clone());
        if !changed_models.is_empty() || changed_since_emit > 0 {
            let _ = app.emit("models-changed", root_id);
        }
        let _ = app.emit("root-status-changed", root_id);
    }
    log::info!(
        "Library scan finished: root={root_id} discovered={discovered_count} unchanged={unchanged_files} changed={} errors={scan_errors} elapsed_ms={}",
        changed_assets.len(),
        scan_started.elapsed().as_millis()
    );
    Ok(())
}

fn asset_is_unchanged(
    existing: Option<&ExistingAsset>,
    file: &DiscoveredFile,
    managed_project_id: Option<&str>,
    managed: bool,
) -> bool {
    existing.is_some_and(|existing| {
        !existing.missing
            && existing.byte_size == file.size
            && existing.modified_at == file.modified
            && (!managed || existing.model_id.as_deref() == managed_project_id)
    })
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

pub async fn index_imported_files(
    root_id: &str,
    root_path: &Path,
    relative_paths: &[String],
    pool: &SqlitePool,
) -> Result<Vec<String>, String> {
    let canonical_root = root_path
        .canonicalize()
        .map_err(|_| "The destination library is unavailable".to_string())?;
    let root_name = root_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Library")
        .to_string();
    let mut folder_cache = HashMap::new();
    let mut asset_ids = Vec::with_capacity(relative_paths.len());
    let mut affected_models = HashSet::new();
    for relative in relative_paths {
        let relative_path = Path::new(relative);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err("Imported project contains an unsafe file path".into());
        }
        let absolute = root_path.join(relative_path);
        let canonical = absolute
            .canonicalize()
            .map_err(|_| "An imported project file is unavailable".to_string())?;
        if !canonical.starts_with(&canonical_root) {
            return Err("Imported project file is outside the destination library".into());
        }
        let extension = canonical
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !SUPPORTED.contains(&extension.as_str()) {
            return Err(format!("Unsupported imported project format: {extension}"));
        }
        let metadata = canonical.metadata().map_err(|error| error.to_string())?;
        let modified: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let outcome = index_file(
            root_id,
            root_path,
            &root_name,
            DiscoveredFile {
                absolute: canonical,
                relative: relative.clone(),
                size: metadata.len(),
                modified: modified.to_rfc3339(),
                extension,
            },
            pool,
            &mut folder_cache,
            None,
            None,
        )
        .await?;
        asset_ids.push(outcome.asset_id);
        affected_models.extend(outcome.affected_model_ids);
    }
    refresh_duplicate_hashes(pool).await?;
    refresh_search_many(&affected_models, pool).await?;
    Ok(asset_ids)
}

fn managed_project_for_file<'a>(
    relative: &str,
    projects: &'a HashMap<String, String>,
) -> Option<&'a String> {
    let mut current = Path::new(relative).parent();
    while let Some(path) = current {
        let key = path.to_string_lossy().replace('\\', "/");
        if let Some(project_id) = projects.get(&key) {
            return Some(project_id);
        }
        current = path.parent();
    }
    None
}

async fn should_ignore_discovered_path(
    file: &DiscoveredFile,
    ignored: Option<&IgnoredAssetPath>,
    pool: &SqlitePool,
) -> Result<bool, String> {
    let Some(ignored) = ignored else {
        return Ok(false);
    };
    if ignored.byte_size == Some(file.size)
        && ignored.modified_at.as_deref() == Some(file.modified.as_str())
    {
        return Ok(true);
    }
    let path = file.absolute.clone();
    let matches = match &ignored.content_hash {
        Some(expected) => {
            let expected = expected.clone();
            tokio::task::spawn_blocking(move || parsers::full_hash(&path))
                .await
                .map_err(|error| error.to_string())??
                == expected
        }
        None => true,
    };
    if matches {
        sqlx::query("UPDATE ignored_asset_paths SET byte_size = ?, modified_at = ? WHERE id = ?")
            .bind(file.size as i64)
            .bind(&file.modified)
            .bind(&ignored.id)
            .execute(pool)
            .await
            .map_err(db_error)?;
    } else {
        sqlx::query("DELETE FROM ignored_asset_paths WHERE id = ?")
            .bind(&ignored.id)
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    Ok(matches)
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

#[allow(clippy::too_many_arguments)]
async fn index_file(
    root_id: &str,
    root_path: &Path,
    root_name: &str,
    file: DiscoveredFile,
    pool: &SqlitePool,
    folder_cache: &mut HashMap<String, String>,
    managed_project_id: Option<&str>,
    preferred_asset_id: Option<&str>,
) -> Result<IndexOutcome, String> {
    let parent = Path::new(&file.relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let folder_rel = if parent == Path::new(".") {
        "".to_string()
    } else {
        parent.to_string_lossy().replace('\\', "/")
    };
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
    let mut transaction = pool.begin().await.map_err(db_error)?;
    let folder_id = ensure_folder(
        root_id,
        &folder_rel,
        root_name,
        &mut transaction,
        folder_cache,
    )
    .await?;
    let existing = sqlx::query("SELECT id FROM assets WHERE root_id = ? AND relative_path = ?")
        .bind(root_id)
        .bind(&file.relative)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(db_error)?;
    let mut asset_id = existing.map(|row| row.get::<String, _>("id"));
    let mut preserved_model_id = None;
    let mut content_hash = None;
    if asset_id.is_none() {
        if let Some(preferred) = preferred_asset_id {
            let preferred_row = sqlx::query("SELECT a.id, a.root_id, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.id = ?")
                .bind(preferred)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(db_error)?;
            if let Some(preferred_row) = preferred_row {
                let assigned_project: Option<String> = preferred_row.try_get("model_id").ok();
                if preferred_row.get::<String, _>("root_id") != root_id
                    || assigned_project.as_deref().is_some_and(|assigned| {
                        managed_project_id.is_some_and(|project| assigned != project)
                    })
                {
                    return Err("A managed sidecar reuses an asset ID owned by another file".into());
                }
                preserved_model_id = assigned_project;
            }
            asset_id = Some(preferred.to_string());
        }
    }
    if asset_id.is_none() {
        if let Some(fingerprint_value) = &fingerprint {
            let candidates = sqlx::query("SELECT a.id, a.relative_path, a.content_hash, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.root_id = ? AND a.partial_fingerprint = ? AND a.byte_size = ? AND a.relative_path != ? AND (? IS NULL OR ma.model_id = ?)")
                .bind(root_id).bind(fingerprint_value).bind(file.size as i64).bind(&file.relative).bind(managed_project_id).bind(managed_project_id).fetch_all(&mut *transaction).await.map_err(db_error)?;
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
        .bind(&asset_id).bind(root_id).bind(&folder_id).bind(&file.relative).bind(&filename).bind(&file.extension).bind(file.size as i64).bind(&file.modified).bind(&fingerprint).bind(&content_hash).bind(parse_status).bind(&metadata_json).execute(&mut *transaction).await.map_err(db_error)?;
    let assigned = sqlx::query("SELECT ma.model_id, m.bundle_mode FROM model_assets ma JOIN models m ON m.id = ma.model_id WHERE ma.asset_id = ? ORDER BY CASE WHEN ma.model_id = ? THEN 0 ELSE 1 END LIMIT 1")
        .bind(&asset_id)
        .bind(managed_project_id.unwrap_or_default())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(db_error)?;
    let previously_assigned_model_id = assigned
        .as_ref()
        .map(|row| row.get::<String, _>("model_id"));
    let assigned_mode = assigned
        .as_ref()
        .map(|row| row.get::<String, _>("bundle_mode"));
    if preserved_model_id.is_none() {
        preserved_model_id = assigned
            .as_ref()
            .map(|row| row.get::<String, _>("model_id"));
    }
    let model_id = if let Some(id) = managed_project_id {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ? AND storage_mode = 'managed' AND trashed_at IS NULL")
            .bind(id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(db_error)?;
        if exists == 0 {
            return Err("Managed project identity is unavailable".into());
        }
        sqlx::query("DELETE FROM model_assets WHERE asset_id = ? AND model_id != ?")
            .bind(&asset_id)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        id.to_string()
    } else if let Some(id) = preserved_model_id {
        let conflict = sqlx::query(
            "SELECT id FROM models WHERE folder_id = ? AND grouping_key = ? AND id != ?",
        )
        .bind(&folder_id)
        .bind(&grouping_key)
        .bind(&id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(db_error)?;
        if assigned_mode.as_deref() == Some("manual") {
            sqlx::query("UPDATE models SET grouping_key = ? WHERE id = ?")
                .bind(format!("manual:{id}"))
                .bind(&id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        } else if conflict.is_none() {
            sqlx::query("UPDATE models SET folder_id = ?, grouping_key = ?, version_key = ?, display_name = ?, missing_since = NULL, updated_at = ? WHERE id = ?").bind(&folder_id).bind(&grouping_key).bind(&version_key).bind(&display_name).bind(now()).bind(&id).execute(&mut *transaction).await.map_err(db_error)?;
        }
        id
    } else {
        sqlx::query("UPDATE models SET grouping_key = 'manual:' || id WHERE folder_id = ? AND grouping_key = ? AND bundle_mode = 'manual'")
            .bind(&folder_id)
            .bind(&grouping_key)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        if let Some(row) =
        sqlx::query("SELECT id FROM models WHERE folder_id = ? AND grouping_key = ? AND bundle_mode = 'automatic'")
            .bind(&folder_id)
            .bind(&grouping_key)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(db_error)?
        {
            let id: String = row.get("id");
            sqlx::query("UPDATE models SET version_key = ? WHERE id = ?")
                .bind(&version_key)
                .bind(&id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            let timestamp = now();
            sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, added_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)").bind(&id).bind(&folder_id).bind(&display_name).bind(&grouping_key).bind(&version_key).bind(&timestamp).bind(&timestamp).execute(&mut *transaction).await.map_err(db_error)?;
            id
        }
    };
    let asset_role = match file.extension.as_str() {
        "step" | "stp" => "source",
        "3mf" => "plate",
        _ => "printable",
    };
    sqlx::query("INSERT OR IGNORE INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, ?, 0)").bind(&model_id).bind(&asset_id).bind(asset_role).execute(&mut *transaction).await.map_err(db_error)?;
    sqlx::query("UPDATE model_assets SET role = ? WHERE asset_id = ?")
        .bind(asset_role)
        .bind(&asset_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    let project = sqlx::query("SELECT m.bundle_mode, EXISTS (SELECT 1 FROM assets current WHERE current.id = m.primary_asset_id AND current.missing_since IS NULL) primary_available FROM models m WHERE m.id = ?")
        .bind(&model_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(db_error)?;
    let preserve_primary = project.get::<String, _>("bundle_mode") == "manual"
        && project.get::<i64, _>("primary_available") != 0;
    if !preserve_primary {
        let primary = sqlx::query("SELECT a.id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.missing_since IS NULL ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 WHEN 'step' THEN 3 WHEN 'stp' THEN 3 ELSE 4 END, a.filename LIMIT 1")
            .bind(&model_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(db_error)?;
        if let Some(primary) = primary {
            sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
                .bind(primary.get::<String, _>("id"))
                .bind(&model_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
    }
    sqlx::query("UPDATE models SET missing_since = NULL, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&model_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    let mut affected_model_ids = vec![model_id];
    if let Some(previous) = previously_assigned_model_id {
        if !affected_model_ids.contains(&previous) {
            affected_model_ids.push(previous);
        }
    }
    transaction.commit().await.map_err(db_error)?;
    Ok(IndexOutcome {
        asset_id,
        affected_model_ids,
    })
}

async fn refresh_geometry_fingerprints(
    root_id: &str,
    root_path: &Path,
    asset_ids: &HashSet<String>,
    pool: &SqlitePool,
) -> Result<(), String> {
    if asset_ids.is_empty() {
        return Ok(());
    }
    let asset_ids = serde_json::to_string(asset_ids).map_err(|error| error.to_string())?;
    let rows = sqlx::query("SELECT a.id, a.relative_path, a.extension, a.partial_fingerprint, a.byte_size, a.modified_at, g.source_fingerprint FROM assets a LEFT JOIN asset_geometry g ON g.asset_id = a.id WHERE a.root_id = ? AND a.missing_since IS NULL AND a.id IN (SELECT value FROM json_each(?))")
        .bind(root_id)
        .bind(asset_ids)
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
    transaction: &mut Transaction<'_, Sqlite>,
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
                .fetch_optional(&mut **transaction)
                .await
                .map_err(db_error)?;
        let id = existing
            .map(|row| row.get("id"))
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT OR IGNORE INTO folders (id, root_id, parent_id, relative_path, name, modified_at) VALUES (?, ?, ?, ?, ?, ?)").bind(&id).bind(root_id).bind(&parent_id).bind(&current).bind(&name).bind(now()).execute(&mut **transaction).await.map_err(db_error)?;
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
    let rows = sqlx::query("SELECT DISTINCT a.id, a.relative_path FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id LEFT JOIN models m ON m.id = ma.model_id WHERE a.root_id = ? AND (m.id IS NULL OR m.trashed_at IS NULL)")
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
    let documentation = project_docs::searchable_text(model_id, pool).await?;
    let notes = format!("{} {}", row.get::<String, _>(1), documentation);
    sqlx::query("DELETE FROM model_search WHERE model_id = ?")
        .bind(model_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO model_search (model_id, display_name, filenames, folder_path, collections, notes, formats) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(model_id).bind(row.get::<String, _>(0)).bind(row.get::<String, _>(3)).bind(row.get::<String, _>(2)).bind(searchable_groups).bind(notes).bind(row.get::<String, _>(4)).execute(pool).await.map_err(db_error)?;
    Ok(())
}

async fn refresh_search_many(model_ids: &HashSet<String>, pool: &SqlitePool) -> Result<(), String> {
    if model_ids.is_empty() {
        return Ok(());
    }
    let ids = serde_json::to_string(model_ids).map_err(|error| error.to_string())?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    sqlx::query("DELETE FROM model_search WHERE model_id IN (SELECT value FROM json_each(?))")
        .bind(&ids)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO model_search (model_id, display_name, filenames, folder_path, collections, notes, formats) SELECT m.id, m.display_name, COALESCE((SELECT group_concat(DISTINCT a.filename) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = m.id), ''), f.relative_path, trim(COALESCE((SELECT group_concat(c.name) FROM collections c JOIN collection_items ci ON ci.collection_id = c.id WHERE ci.model_id = m.id), '') || ' ' || COALESCE((SELECT group_concat(t.name) FROM tags t JOIN model_tags mt ON mt.tag_id = t.id WHERE mt.model_id = m.id), '')), trim(COALESCE(m.notes, '') || ' ' || COALESCE((SELECT group_concat(d.name || ' ' || v.value_json, ' ') FROM project_custom_values v JOIN custom_field_definitions d ON d.id = v.field_id WHERE v.model_id = m.id), '') || ' ' || COALESCE((SELECT group_concat(name || ' ' || COALESCE(url, '') || ' ' || notes, ' ') FROM project_bom_items WHERE model_id = m.id), '') || ' ' || COALESCE((SELECT group_concat(title || ' ' || instructions, ' ') FROM assembly_steps WHERE model_id = m.id), '') || ' ' || COALESCE((SELECT group_concat(caption, ' ') FROM project_photos WHERE model_id = m.id), '')), COALESCE((SELECT group_concat(DISTINCT a.extension) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = m.id), '') FROM models m JOIN folders f ON f.id = m.folder_id WHERE m.id IN (SELECT value FROM json_each(?))")
        .bind(&ids)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
}

async fn cleanup_thumbnail_cache(pool: &SqlitePool, max_bytes: u64) -> Result<(), String> {
    let summary = sqlx::query("SELECT COALESCE(SUM(byte_size), 0) total_bytes, SUM(CASE WHEN byte_size = 0 THEN 1 ELSE 0 END) unknown_sizes FROM thumbnails")
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let total_bytes = summary.get::<i64, _>("total_bytes").max(0) as u64;
    let unknown_sizes = summary.get::<i64, _>("unknown_sizes");
    if unknown_sizes == 0 && total_bytes <= max_bytes {
        return Ok(());
    }
    let rows =
        sqlx::query("SELECT id, cache_path, byte_size FROM thumbnails ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .map_err(db_error)?;
    let mut retained = 0_u64;
    for row in rows {
        let id: String = row.get("id");
        let path = PathBuf::from(row.get::<String, _>("cache_path"));
        let stored_size = row.get::<i64, _>("byte_size").max(0) as u64;
        let size = if stored_size > 0 {
            stored_size
        } else {
            let measured = match std::fs::metadata(&path) {
                Ok(metadata) if metadata.len() > 0 => metadata.len(),
                _ => {
                    sqlx::query("DELETE FROM thumbnails WHERE id = ?")
                        .bind(&id)
                        .execute(pool)
                        .await
                        .map_err(db_error)?;
                    continue;
                }
            };
            let _ = sqlx::query("UPDATE thumbnails SET byte_size = ? WHERE id = ?")
                .bind(measured as i64)
                .bind(&id)
                .execute(pool)
                .await;
            measured
        };
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
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::{str::FromStr, time::Instant};

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

    #[test]
    fn classifies_large_unchanged_manifests_without_file_io() {
        for count in [1_000_usize, 10_000] {
            let assets = (0..count)
                .map(|index| {
                    (
                        format!("models/{index}.stl"),
                        ExistingAsset {
                            byte_size: index as u64 + 128,
                            modified_at: "2026-01-01T00:00:00Z".into(),
                            missing: false,
                            model_id: Some(format!("model-{index}")),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let started = Instant::now();
            let unchanged = (0..count)
                .filter(|index| {
                    let relative = format!("models/{index}.stl");
                    let file = DiscoveredFile {
                        absolute: PathBuf::from(&relative),
                        relative: relative.clone(),
                        size: if *index == count - 1 {
                            *index as u64 + 129
                        } else {
                            *index as u64 + 128
                        },
                        modified: "2026-01-01T00:00:00Z".into(),
                        extension: "stl".into(),
                    };
                    asset_is_unchanged(assets.get(&relative), &file, None, false)
                })
                .count();
            assert_eq!(unchanged, count - 1);
            eprintln!(
                "VOLUM_BENCH manifest files={count} elapsed_us={}",
                started.elapsed().as_micros()
            );
        }
    }

    #[tokio::test]
    #[ignore = "reads a local library and writes only to an explicitly supplied database copy"]
    async fn benchmark_local_warm_scan() {
        let database = std::env::var("VOLUM_BENCH_DATABASE")
            .expect("set VOLUM_BENCH_DATABASE to a disposable SQLite snapshot");
        let root_id = std::env::var("VOLUM_BENCH_ROOT_ID")
            .expect("set VOLUM_BENCH_ROOT_ID to a root in the snapshot");
        let options = SqliteConnectOptions::from_str(&database)
            .unwrap()
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let root_path = PathBuf::from(
            sqlx::query_scalar::<_, String>("SELECT path FROM library_roots WHERE id = ?")
                .bind(&root_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
        );
        let root_name = root_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Library")
            .to_string();
        let started = Instant::now();
        let files = discover(&root_path).unwrap();
        let discovered = files.len();
        let existing_assets = sqlx::query("SELECT a.relative_path, a.byte_size, a.modified_at, a.missing_since, ma.model_id FROM assets a LEFT JOIN model_assets ma ON ma.asset_id = a.id WHERE a.root_id = ?")
            .bind(&root_id)
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("relative_path"),
                    ExistingAsset {
                        byte_size: row.get::<i64, _>("byte_size").max(0) as u64,
                        modified_at: row.get("modified_at"),
                        missing: row.get::<Option<String>, _>("missing_since").is_some(),
                        model_id: row.try_get("model_id").ok(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::with_capacity(discovered);
        let mut folder_cache = HashMap::new();
        let mut changed_models = HashSet::new();
        let mut changed_assets = HashSet::new();
        for file in files {
            seen.insert(file.relative.clone());
            if existing_assets.get(&file.relative).is_some_and(|existing| {
                !existing.missing
                    && existing.byte_size == file.size
                    && existing.modified_at == file.modified
            }) {
                continue;
            }
            let outcome = index_file(
                &root_id,
                &root_path,
                &root_name,
                file,
                &pool,
                &mut folder_cache,
                None,
                None,
            )
            .await
            .unwrap();
            changed_assets.insert(outcome.asset_id);
            changed_models.extend(outcome.affected_model_ids);
        }
        for (relative, existing) in &existing_assets {
            if !existing.missing && !seen.contains(relative) {
                if let Some(model_id) = &existing.model_id {
                    changed_models.insert(model_id.clone());
                }
            }
        }
        reconcile_missing(&root_id, &seen, &pool).await.unwrap();
        if !changed_models.is_empty() {
            refresh_duplicate_hashes(&pool).await.unwrap();
            refresh_geometry_fingerprints(&root_id, &root_path, &changed_assets, &pool)
                .await
                .unwrap();
            refresh_search_many(&changed_models, &pool).await.unwrap();
        }
        cleanup_thumbnail_cache(&pool, 1024 * 1024 * 1024)
            .await
            .unwrap();
        eprintln!(
            "VOLUM_BENCH warm_scan files={discovered} changed={} elapsed_ms={}",
            changed_assets.len(),
            started.elapsed().as_millis()
        );
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
        let mut changed_models = HashSet::new();
        for file in files {
            let outcome = index_file(
                &root_id,
                library.path(),
                "Models",
                file,
                &state.pool,
                &mut cache,
                None,
                None,
            )
            .await
            .unwrap();
            changed_models.extend(outcome.affected_model_ids);
        }
        refresh_search_many(&changed_models, &state.pool)
            .await
            .unwrap();
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
        let aggregate: (i64, Option<String>) =
            sqlx::query_as("SELECT asset_count, asset_modified_at FROM models LIMIT 1")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(model_count, 1);
        assert_eq!(asset_count, 2);
        assert_eq!(search_count, 1);
        assert_eq!(aggregate.0, 2);
        assert!(aggregate.1.is_some());

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
            None,
            None,
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
            None,
            None,
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
