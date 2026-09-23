use crate::state::AppState;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use percent_encoding::percent_decode_str;
use regex::Regex;
use reqwest::{Client, Method, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const BACKUP_FORMAT: &str = "volum-backup";
const BACKUP_FORMAT_VERSION: u32 = 1;
const BACKUP_PREFIX: &str = "volum-backup-";
const KEYRING_SERVICE: &str = "dev.didac.volum.backup";
const MAX_RESTORE_ENTRIES: usize = 50_000;
const MAX_RESTORE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_RESTORE_ARCHIVE_BYTES: u64 = MAX_RESTORE_BYTES + 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const SCHEDULE_RETRY_DELAY: ChronoDuration = ChronoDuration::hours(1);

type CommandResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDestination {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub username: Option<String>,
    pub credential_saved: bool,
    pub schedule: String,
    pub retention_count: i64,
    pub enabled: bool,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDestinationInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_schedule")]
    pub schedule: String,
    #[serde(default = "default_retention")]
    pub retention_count: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRun {
    pub id: String,
    pub destination_id: Option<String>,
    pub destination_name: String,
    pub reason: String,
    pub status: String,
    pub filename: String,
    pub byte_size: Option<i64>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupArchive {
    pub key: String,
    pub filename: String,
    pub byte_size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCounts {
    pub libraries: i64,
    pub models: i64,
    pub collections: i64,
    pub tags: i64,
    pub web_sources: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    path: String,
    byte_size: u64,
    blake3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format: String,
    format_version: u32,
    app_version: String,
    schema_version: i64,
    created_at: String,
    counts: BackupCounts,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub app_version: String,
    pub created_at: String,
    pub counts: BackupCounts,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedRestore {
    pub token: String,
    #[serde(flatten)]
    pub summary: BackupSummary,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingRestore {
    token: String,
    prepared_at: String,
}

#[derive(Debug, Clone)]
struct ArchiveSource {
    path: PathBuf,
    archive_path: String,
    byte_size: u64,
    blake3: String,
}

fn default_schedule() -> String {
    "weekly".into()
}

fn default_retention() -> i64 {
    7
}

fn default_true() -> bool {
    true
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

fn destination_from_row(row: sqlx::sqlite::SqliteRow) -> BackupDestination {
    BackupDestination {
        id: row.get("id"),
        name: row.get("name"),
        kind: row.get("kind"),
        location: row.get("location"),
        username: row.get("username"),
        credential_saved: row.get::<i64, _>("credential_saved") != 0,
        schedule: row.get("schedule"),
        retention_count: row.get("retention_count"),
        enabled: row.get::<i64, _>("enabled") != 0,
        last_attempt_at: row.get("last_attempt_at"),
        last_success_at: row.get("last_success_at"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn run_from_row(row: sqlx::sqlite::SqliteRow) -> BackupRun {
    BackupRun {
        id: row.get("id"),
        destination_id: row.get("destination_id"),
        destination_name: row.get("destination_name"),
        reason: row.get("reason"),
        status: row.get("status"),
        filename: row.get("filename"),
        byte_size: row.get("byte_size"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        error: row.get("error"),
    }
}

async fn get_destination(id: &str, state: &AppState) -> CommandResult<BackupDestination> {
    sqlx::query("SELECT * FROM backup_destinations WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .map(destination_from_row)
        .ok_or_else(|| "Backup destination not found".to_string())
}

#[tauri::command]
pub async fn list_backup_destinations(
    state: State<'_, AppState>,
) -> CommandResult<Vec<BackupDestination>> {
    Ok(
        sqlx::query("SELECT * FROM backup_destinations ORDER BY lower(name), created_at")
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(destination_from_row)
            .collect(),
    )
}

#[tauri::command]
pub async fn save_backup_destination(
    input: BackupDestinationInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<BackupDestination> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err("Enter a backup destination name".into());
    }
    if !matches!(input.kind.as_str(), "folder" | "webdav") {
        return Err("Choose a supported backup destination type".into());
    }
    if !matches!(
        input.schedule.as_str(),
        "manual" | "daily" | "weekly" | "monthly"
    ) {
        return Err("Choose a valid backup schedule".into());
    }
    let retention = input.retention_count.clamp(1, 1000);
    let location = if input.kind == "folder" {
        let path = PathBuf::from(input.location.trim());
        if !path.is_absolute() {
            return Err("Choose an absolute backup folder".into());
        }
        if path.starts_with(&state.data_dir) {
            return Err("Choose a backup folder outside Volum’s app-data folder".into());
        }
        path.to_string_lossy().to_string()
    } else {
        normalize_webdav_location(&input.location)?
    };
    if location.len() > 4096
        || input
            .username
            .as_ref()
            .is_some_and(|value| value.len() > 512)
    {
        return Err("Backup destination details are too long".into());
    }
    let id = if let Some(id) = input.id {
        Uuid::parse_str(&id).map_err(|_| "Invalid backup destination ID".to_string())?;
        id
    } else {
        Uuid::new_v4().to_string()
    };
    let existing = sqlx::query(
        "SELECT created_at, kind, username, credential_saved FROM backup_destinations WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?;
    let created_at = existing
        .as_ref()
        .map(|row| row.get::<String, _>("created_at"))
        .unwrap_or_else(now);
    let mut credential_saved = existing
        .as_ref()
        .map(|row| row.get::<i64, _>("credential_saved") != 0)
        .unwrap_or(false);
    let username = input
        .username
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if input.kind == "webdav" {
        let credential_owner_changed = existing.as_ref().is_some_and(|row| {
            row.get::<String, _>("kind") != "webdav"
                || row.get::<Option<String>, _>("username") != username
        });
        if username.is_none() || credential_owner_changed {
            let _ = delete_password(&id).await;
            credential_saved = false;
        }
        if let Some(password) = input.password.filter(|value| !value.is_empty()) {
            if username.is_none() {
                return Err("Enter a WebDAV username before saving its password".into());
            }
            set_password(&id, password).await?;
            credential_saved = true;
        }
    } else {
        let _ = delete_password(&id).await;
        credential_saved = false;
    }
    let timestamp = now();
    sqlx::query("INSERT INTO backup_destinations (id, name, kind, location, username, credential_saved, schedule, retention_count, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, kind = excluded.kind, location = excluded.location, username = excluded.username, credential_saved = excluded.credential_saved, schedule = excluded.schedule, retention_count = excluded.retention_count, enabled = excluded.enabled, updated_at = excluded.updated_at")
        .bind(&id).bind(name).bind(&input.kind).bind(location).bind(username).bind(i64::from(credential_saved)).bind(&input.schedule).bind(retention).bind(i64::from(input.enabled)).bind(created_at).bind(timestamp)
        .execute(&state.pool).await.map_err(db_error)?;
    let _ = app.emit("backups-changed", ());
    get_destination(&id, &state).await
}

#[tauri::command]
pub async fn delete_backup_destination(
    destination_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    sqlx::query("DELETE FROM backup_destinations WHERE id = ?")
        .bind(&destination_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = delete_password(&destination_id).await;
    let _ = app.emit("backups-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn test_backup_destination(
    destination_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let destination = get_destination(&destination_id, &state).await?;
    if destination.kind == "folder" {
        let directory = PathBuf::from(&destination.location);
        if !directory.is_dir() {
            return Err("The backup folder is currently unavailable".into());
        }
        let probe = directory.join(format!(".volum-write-test-{}", Uuid::new_v4()));
        tokio::fs::write(&probe, b"volum")
            .await
            .map_err(|error| format!("Could not write to the backup folder: {error}"))?;
        tokio::fs::remove_file(probe)
            .await
            .map_err(|error| format!("Could not clean up the backup test: {error}"))?;
        Ok(())
    } else {
        let base = Url::parse(&destination.location).map_err(|error| error.to_string())?;
        let probe_name = format!(".volum-write-test-{}", Uuid::new_v4());
        let partial_url = base
            .join(&format!("{probe_name}.partial"))
            .map_err(|error| error.to_string())?;
        let final_url = base.join(&probe_name).map_err(|error| error.to_string())?;
        let response = webdav_request(&destination, Method::PUT, partial_url.as_str())
            .await?
            .body("volum")
            .send()
            .await
            .map_err(|error| format!("Could not reach WebDAV: {error}"))?;
        if !response.status().is_success() {
            let status = response.status();
            if let Ok(request) =
                webdav_request(&destination, Method::DELETE, partial_url.as_str()).await
            {
                let _ = request.send().await;
            }
            return Err(format!("WebDAV write test returned {status}"));
        }
        let moved = webdav_request(
            &destination,
            Method::from_bytes(b"MOVE").map_err(|error| error.to_string())?,
            partial_url.as_str(),
        )
        .await?
        .header("Destination", final_url.as_str())
        .send()
        .await
        .map_err(|error| format!("Could not finish the WebDAV write test: {error}"))?;
        let _ = webdav_request(&destination, Method::DELETE, partial_url.as_str())
            .await?
            .send()
            .await;
        let _ = webdav_request(&destination, Method::DELETE, final_url.as_str())
            .await?
            .send()
            .await;
        if !moved.status().is_success() {
            return Err(format!(
                "WebDAV does not support the atomic MOVE required by Volum ({})",
                moved.status()
            ));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn list_backup_runs(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<BackupRun>> {
    Ok(
        sqlx::query("SELECT * FROM backup_runs ORDER BY started_at DESC LIMIT ?")
            .bind(limit.unwrap_or(20).clamp(1, 100))
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(run_from_row)
            .collect(),
    )
}

#[tauri::command]
pub async fn run_backup(
    destination_id: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<Vec<BackupRun>> {
    let destinations = if let Some(id) = destination_id {
        vec![get_destination(&id, &state).await?]
    } else {
        sqlx::query("SELECT * FROM backup_destinations WHERE enabled = 1 ORDER BY created_at")
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?
            .into_iter()
            .map(destination_from_row)
            .collect()
    };
    if destinations.is_empty() {
        return Err("Add and enable a backup destination first".into());
    }
    let _guard = state.backup_lock.lock().await;
    let mut runs = Vec::with_capacity(destinations.len());
    for destination in destinations {
        runs.push(perform_backup(&destination, "manual", &state, &app).await);
    }
    let _ = app.emit("backups-changed", ());
    Ok(runs)
}

async fn perform_backup(
    destination: &BackupDestination,
    reason: &str,
    state: &AppState,
    app: &AppHandle,
) -> BackupRun {
    let started_at = now();
    let run_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let filename = format!(
        "{BACKUP_PREFIX}{timestamp}-{}-{}.zip",
        destination_key(&destination.id),
        &run_id[..8]
    );
    let _ = sqlx::query("INSERT INTO backup_runs (id, destination_id, destination_name, reason, status, filename, started_at) VALUES (?, ?, ?, ?, 'running', ?, ?)")
        .bind(&run_id).bind(&destination.id).bind(&destination.name).bind(reason).bind(&filename).bind(&started_at).execute(&state.pool).await;
    let _ = sqlx::query("UPDATE backup_destinations SET last_attempt_at = ?, last_error = NULL, updated_at = ? WHERE id = ?")
        .bind(&started_at).bind(&started_at).bind(&destination.id).execute(&state.pool).await;
    let _ = app.emit("backups-changed", ());

    let staging = state.data_dir.join("backup-staging").join(&run_id);
    let result = async {
        tokio::fs::create_dir_all(&staging)
            .await
            .map_err(|error| error.to_string())?;
        let archive_path = staging.join(&filename);
        create_backup_archive(&archive_path, state).await?;
        let size = tokio::fs::metadata(&archive_path)
            .await
            .map_err(|error| error.to_string())?
            .len();
        if destination.kind == "folder" {
            deliver_to_folder(&archive_path, &filename, destination).await?;
            if let Err(error) = prune_folder_backups(destination).await {
                log::warn!(
                    "Backup retention cleanup failed for {}: {error}",
                    destination.name
                );
            }
        } else {
            deliver_to_webdav(&archive_path, &filename, destination).await?;
            if let Err(error) = prune_webdav_backups(destination).await {
                log::warn!(
                    "Backup retention cleanup failed for {}: {error}",
                    destination.name
                );
            }
        }
        Ok::<u64, String>(size)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;

    let completed_at = now();
    let run = match result {
        Ok(size) => {
            let _ = sqlx::query("UPDATE backup_runs SET status = 'complete', byte_size = ?, completed_at = ? WHERE id = ?")
                .bind(size as i64).bind(&completed_at).bind(&run_id).execute(&state.pool).await;
            let _ = sqlx::query("UPDATE backup_destinations SET last_success_at = ?, last_error = NULL, updated_at = ? WHERE id = ?")
                .bind(&completed_at).bind(&completed_at).bind(&destination.id).execute(&state.pool).await;
            BackupRun {
                id: run_id,
                destination_id: Some(destination.id.clone()),
                destination_name: destination.name.clone(),
                reason: reason.into(),
                status: "complete".into(),
                filename,
                byte_size: Some(size as i64),
                started_at,
                completed_at: Some(completed_at),
                error: None,
            }
        }
        Err(error) => {
            let _ = sqlx::query("UPDATE backup_runs SET status = 'failed', completed_at = ?, error = ? WHERE id = ?")
                .bind(&completed_at).bind(&error).bind(&run_id).execute(&state.pool).await;
            let _ = sqlx::query(
                "UPDATE backup_destinations SET last_error = ?, updated_at = ? WHERE id = ?",
            )
            .bind(&error)
            .bind(&completed_at)
            .bind(&destination.id)
            .execute(&state.pool)
            .await;
            BackupRun {
                id: run_id,
                destination_id: Some(destination.id.clone()),
                destination_name: destination.name.clone(),
                reason: reason.into(),
                status: "failed".into(),
                filename,
                byte_size: None,
                started_at,
                completed_at: Some(completed_at),
                error: Some(error),
            }
        }
    };
    let _ = sqlx::query("DELETE FROM backup_runs WHERE id NOT IN (SELECT id FROM backup_runs ORDER BY started_at DESC LIMIT 200)")
        .execute(&state.pool)
        .await;
    run
}

async fn create_backup_archive(
    destination: &Path,
    state: &AppState,
) -> CommandResult<BackupManifest> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
    }
    let snapshot = destination.with_extension("sqlite3.snapshot");
    if snapshot.exists() {
        tokio::fs::remove_file(&snapshot)
            .await
            .map_err(|error| error.to_string())?;
    }
    sqlx::query("VACUUM INTO ?")
        .bind(snapshot.to_string_lossy().to_string())
        .execute(&state.pool)
        .await
        .map_err(|error| format!("Could not snapshot the Volum database: {error}"))?;
    let (schema_version, counts) = match snapshot_metadata(&snapshot).await {
        Ok(metadata) => metadata,
        Err(error) => {
            let _ = tokio::fs::remove_file(&snapshot).await;
            return Err(error);
        }
    };
    let destination = destination.to_path_buf();
    let media = state.data_dir.join("media");
    let snapshot_for_cleanup = snapshot.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut sources = Vec::new();
        sources.push(archive_source(&snapshot, "database/volum.sqlite3")?);
        if media.is_dir() {
            for entry in WalkDir::new(&media).follow_links(false) {
                let entry = entry.map_err(|error| format!("Could not read user media: {error}"))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let relative = entry
                    .path()
                    .strip_prefix(&media)
                    .map_err(|error| error.to_string())?;
                let archive_path =
                    format!("media/{}", relative.to_string_lossy().replace('\\', "/"));
                sources.push(archive_source(entry.path(), &archive_path)?);
            }
        }
        if sources.len() + 2 > MAX_RESTORE_ENTRIES {
            return Err("There are too many user-media files to create a restorable backup".into());
        }
        let source_bytes = sources
            .iter()
            .try_fold(0_u64, |total, source| total.checked_add(source.byte_size))
            .ok_or_else(|| "The backup source size is invalid".to_string())?;
        if source_bytes > MAX_RESTORE_BYTES {
            return Err("Volum data and user media exceed the 2 GB backup limit".into());
        }
        let files = sources
            .iter()
            .map(|source| ManifestFile {
                path: source.archive_path.clone(),
                byte_size: source.byte_size,
                blake3: source.blake3.clone(),
            })
            .collect::<Vec<_>>();
        let manifest = BackupManifest {
            format: BACKUP_FORMAT.into(),
            format_version: BACKUP_FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").into(),
            schema_version,
            created_at: now(),
            counts,
            files,
        };
        write_backup_zip(&destination, &manifest, &sources)?;
        Ok::<BackupManifest, String>(manifest)
    })
    .await
    .map_err(|error| error.to_string());
    let _ = tokio::fs::remove_file(snapshot_for_cleanup).await;
    result?
}

async fn snapshot_metadata(path: &Path) -> CommandResult<(i64, BackupCounts)> {
    let options = sqlx::sqlite::SqliteConnectOptions::from_str(path.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?
        .foreign_keys(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM backup_runs WHERE status = 'running'")
        .execute(&mut connection)
        .await
        .map_err(db_error)?;
    let schema_version =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?;
    let counts = BackupCounts {
        libraries: sqlx::query_scalar("SELECT COUNT(*) FROM library_roots")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?,
        models: sqlx::query_scalar("SELECT COUNT(*) FROM models")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?,
        collections: sqlx::query_scalar("SELECT COUNT(*) FROM collections")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?,
        tags: sqlx::query_scalar("SELECT COUNT(*) FROM tags")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?,
        web_sources: sqlx::query_scalar("SELECT COUNT(*) FROM web_sources")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?,
    };
    Ok((schema_version, counts))
}

fn archive_source(path: &Path, archive_path: &str) -> CommandResult<ArchiveSource> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    Ok(ArchiveSource {
        path: path.to_path_buf(),
        archive_path: archive_path.into(),
        byte_size: metadata.len(),
        blake3: hash_file(path)?,
    })
}

fn hash_file(path: &Path) -> CommandResult<String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn write_backup_zip(
    destination: &Path,
    manifest: &BackupManifest,
    sources: &[ArchiveSource],
) -> CommandResult<()> {
    let temporary = destination.with_extension("zip.partial");
    let file = File::create(&temporary).map_err(|error| error.to_string())?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    writer
        .start_file("manifest.json", options)
        .map_err(|error| error.to_string())?;
    writer
        .write_all(&serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let checksums = manifest
        .files
        .iter()
        .map(|file| (file.path.clone(), file.blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    writer
        .start_file("checksums.json", options)
        .map_err(|error| error.to_string())?;
    writer
        .write_all(&serde_json::to_vec_pretty(&checksums).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    for source in sources {
        writer
            .start_file(&source.archive_path, options)
            .map_err(|error| error.to_string())?;
        let mut input = File::open(&source.path).map_err(|error| error.to_string())?;
        let mut hasher = blake3::Hasher::new();
        let mut byte_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            byte_size = byte_size.saturating_add(count as u64);
            hasher.update(&buffer[..count]);
            writer
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
        }
        if byte_size != source.byte_size || hasher.finalize().to_hex().as_str() != source.blake3 {
            return Err(format!(
                "Backup source changed while it was being read: {}. Try again.",
                source.archive_path
            ));
        }
    }
    let output = writer.finish().map_err(|error| error.to_string())?;
    output.sync_all().map_err(|error| error.to_string())?;
    std::fs::rename(temporary, destination).map_err(|error| error.to_string())
}

async fn deliver_to_folder(
    source: &Path,
    filename: &str,
    destination: &BackupDestination,
) -> CommandResult<()> {
    let directory = PathBuf::from(&destination.location);
    if !directory.is_dir() {
        return Err("The backup folder is currently unavailable".into());
    }
    let final_path = directory.join(filename);
    let temporary = directory.join(format!(".{filename}.partial"));
    if let Err(error) = tokio::fs::copy(source, &temporary).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("Could not copy the backup: {error}"));
    }
    let file = match tokio::fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .await
    {
        Ok(file) => file,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error.to_string());
        }
    };
    if let Err(error) = file.sync_all().await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.to_string());
    }
    if let Err(error) = tokio::fs::rename(&temporary, &final_path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("Could not finish the backup: {error}"));
    }
    Ok(())
}

async fn deliver_to_webdav(
    source: &Path,
    filename: &str,
    destination: &BackupDestination,
) -> CommandResult<()> {
    let base = Url::parse(&destination.location).map_err(|error| error.to_string())?;
    let final_url = base.join(filename).map_err(|error| error.to_string())?;
    let partial_name = format!(".{filename}.partial");
    let partial_url = base
        .join(&partial_name)
        .map_err(|error| error.to_string())?;
    if let Err(error) = upload_webdav_file(source, partial_url.as_str(), destination).await {
        if let Ok(request) = webdav_request(destination, Method::DELETE, partial_url.as_str()).await
        {
            let _ = request.send().await;
        }
        return Err(error);
    }
    let move_method = Method::from_bytes(b"MOVE").map_err(|error| error.to_string())?;
    let moved = webdav_request(destination, move_method, partial_url.as_str())
        .await?
        .header("Destination", final_url.as_str())
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if moved.status().is_success()
        || moved.status() == StatusCode::CREATED
        || moved.status() == StatusCode::NO_CONTENT
    {
        return Ok(());
    }
    let _ = webdav_request(destination, Method::DELETE, partial_url.as_str())
        .await?
        .send()
        .await;
    Err(format!(
        "WebDAV does not support the atomic MOVE required by Volum ({})",
        moved.status()
    ))
}

async fn upload_webdav_file(
    source: &Path,
    url: &str,
    destination: &BackupDestination,
) -> CommandResult<()> {
    let file = tokio::fs::File::open(source)
        .await
        .map_err(|error| error.to_string())?;
    let size = file
        .metadata()
        .await
        .map_err(|error| error.to_string())?
        .len();
    let response = webdav_request(destination, Method::PUT, url)
        .await?
        .header(reqwest::header::CONTENT_LENGTH, size)
        .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
        .send()
        .await
        .map_err(|error| format!("Could not upload the backup: {error}"))?;
    if response.status().is_success()
        || response.status() == StatusCode::CREATED
        || response.status() == StatusCode::NO_CONTENT
    {
        Ok(())
    } else {
        Err(format!("WebDAV upload returned {}", response.status()))
    }
}

async fn prune_folder_backups(destination: &BackupDestination) -> CommandResult<()> {
    let mut archives = folder_archives(destination).await?;
    archives.retain(|archive| belongs_to_destination(&archive.filename, &destination.id));
    archives.sort_by(|left, right| right.filename.cmp(&left.filename));
    for archive in archives
        .into_iter()
        .skip(destination.retention_count as usize)
    {
        let _ =
            tokio::fs::remove_file(PathBuf::from(&destination.location).join(archive.key)).await;
    }
    Ok(())
}

async fn prune_webdav_backups(destination: &BackupDestination) -> CommandResult<()> {
    let mut archives = webdav_archives(destination).await?;
    archives.retain(|archive| belongs_to_destination(&archive.filename, &destination.id));
    archives.sort_by(|left, right| right.filename.cmp(&left.filename));
    let base = Url::parse(&destination.location).map_err(|error| error.to_string())?;
    for archive in archives
        .into_iter()
        .skip(destination.retention_count as usize)
    {
        if let Ok(url) = base.join(&archive.key) {
            let _ = webdav_request(destination, Method::DELETE, url.as_str())
                .await?
                .send()
                .await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_destination_archives(
    destination_id: String,
    state: State<'_, AppState>,
) -> CommandResult<Vec<BackupArchive>> {
    let destination = get_destination(&destination_id, &state).await?;
    if destination.kind == "folder" {
        folder_archives(&destination).await
    } else {
        webdav_archives(&destination).await
    }
}

async fn folder_archives(destination: &BackupDestination) -> CommandResult<Vec<BackupArchive>> {
    let directory = PathBuf::from(&destination.location);
    let mut output = Vec::new();
    let mut entries = match tokio::fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(output),
        Err(error) => return Err(error.to_string()),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !valid_archive_name(&filename) {
            continue;
        }
        let metadata = entry.metadata().await.map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            continue;
        }
        output.push(BackupArchive {
            key: filename.clone(),
            filename,
            byte_size: Some(metadata.len()),
            modified_at: metadata
                .modified()
                .ok()
                .map(DateTime::<Utc>::from)
                .map(|value| value.to_rfc3339()),
        });
    }
    output.sort_by(|left, right| right.filename.cmp(&left.filename));
    Ok(output)
}

async fn webdav_archives(destination: &BackupDestination) -> CommandResult<Vec<BackupArchive>> {
    let method = Method::from_bytes(b"PROPFIND").map_err(|error| error.to_string())?;
    let mut response = webdav_request(destination, method, &destination.location).await?
        .header("Depth", "1")
        .header(reqwest::header::CONTENT_TYPE, "application/xml")
        .body("<?xml version=\"1.0\"?><propfind xmlns=\"DAV:\"><prop><getcontentlength/><getlastmodified/></prop></propfind>")
        .send().await.map_err(|error| format!("Could not list WebDAV backups: {error}"))?;
    if !(response.status().is_success() || response.status() == StatusCode::MULTI_STATUS) {
        return Err(format!("WebDAV returned {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > 5 * 1024 * 1024)
    {
        return Err("The WebDAV directory listing is too large".into());
    }
    let mut body_bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if body_bytes.len().saturating_add(chunk.len()) > 5 * 1024 * 1024 {
            return Err("The WebDAV directory listing is too large".into());
        }
        body_bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(body_bytes)
        .map_err(|_| "The WebDAV directory listing is not valid UTF-8".to_string())?;
    let hrefs = Regex::new(r"(?is)<(?:[A-Za-z0-9_-]+:)?href[^>]*>(.*?)</(?:[A-Za-z0-9_-]+:)?href>")
        .map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    for captures in hrefs.captures_iter(&body) {
        let href = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default()
            .replace("&amp;", "&");
        let segment = href
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let filename = percent_decode_str(segment).decode_utf8_lossy().to_string();
        if valid_archive_name(&filename) {
            output.push(BackupArchive {
                key: filename.clone(),
                filename,
                byte_size: None,
                modified_at: None,
            });
        }
    }
    output.sort_by(|left, right| right.filename.cmp(&left.filename));
    output.dedup_by(|left, right| left.key == right.key);
    Ok(output)
}

fn valid_archive_name(name: &str) -> bool {
    name.starts_with(BACKUP_PREFIX)
        && name.ends_with(".zip")
        && !name.contains('/')
        && !name.contains('\\')
}

fn destination_key(id: &str) -> String {
    Uuid::parse_str(id)
        .map(|value| value.simple().to_string()[..8].to_string())
        .unwrap_or_else(|_| blake3::hash(id.as_bytes()).to_hex().as_str()[..8].to_string())
}

fn belongs_to_destination(filename: &str, destination_id: &str) -> bool {
    filename.contains(&format!("-{}-", destination_key(destination_id)))
}

#[tauri::command]
pub async fn prepare_restore_from_path(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<PreparedRestore> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("Choose a Volum backup ZIP".into());
    }
    let _guard = state.backup_lock.lock().await;
    prepare_restore(&path, &state).await
}

#[tauri::command]
pub async fn prepare_restore_from_destination(
    destination_id: String,
    key: String,
    state: State<'_, AppState>,
) -> CommandResult<PreparedRestore> {
    if !valid_archive_name(&key) {
        return Err("Invalid backup filename".into());
    }
    let destination = get_destination(&destination_id, &state).await?;
    let _guard = state.backup_lock.lock().await;
    if destination.kind == "folder" {
        prepare_restore(&PathBuf::from(destination.location).join(key), &state).await
    } else {
        let download_dir = state.data_dir.join("restore-downloads");
        tokio::fs::create_dir_all(&download_dir)
            .await
            .map_err(|error| error.to_string())?;
        let download = download_dir.join(format!("{}.zip", Uuid::new_v4()));
        let result = match download_webdav_archive(&destination, &key, &download).await {
            Ok(()) => prepare_restore(&download, &state).await,
            Err(error) => Err(error),
        };
        let _ = tokio::fs::remove_file(download).await;
        result
    }
}

async fn download_webdav_archive(
    destination: &BackupDestination,
    key: &str,
    output: &Path,
) -> CommandResult<()> {
    let url = Url::parse(&destination.location)
        .map_err(|error| error.to_string())?
        .join(key)
        .map_err(|error| error.to_string())?;
    let mut response = webdav_request(destination, Method::GET, url.as_str())
        .await?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("WebDAV download returned {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESTORE_ARCHIVE_BYTES)
    {
        return Err("The backup is too large to restore safely".into());
    }
    let mut file = tokio::fs::File::create(output)
        .await
        .map_err(|error| error.to_string())?;
    let mut total = 0_u64;
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        total = total.saturating_add(chunk.len() as u64);
        if total > MAX_RESTORE_ARCHIVE_BYTES {
            return Err("The backup is too large to restore safely".into());
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    file.sync_all().await.map_err(|error| error.to_string())
}

async fn prepare_restore(path: &Path, state: &AppState) -> CommandResult<PreparedRestore> {
    let token = Uuid::new_v4().to_string();
    let staging = state.data_dir.join("restore-staging").join(&token);
    let archive = path.to_path_buf();
    let staging_for_extract = staging.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        validate_and_extract_backup(&archive, &staging_for_extract)
    })
    .await
    .map_err(|error| error.to_string())?;
    let (manifest, byte_size) = match extracted {
        Ok(result) => result,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error);
        }
    };
    if let Err(error) = validate_staged_database(
        &staging.join("database/volum.sqlite3"),
        manifest.schema_version,
        state,
    )
    .await
    {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    Ok(PreparedRestore {
        token,
        summary: BackupSummary {
            app_version: manifest.app_version,
            created_at: manifest.created_at,
            counts: manifest.counts,
            byte_size,
        },
    })
}

fn validate_and_extract_backup(
    archive_path: &Path,
    staging: &Path,
) -> CommandResult<(BackupManifest, u64)> {
    let archive_size = std::fs::metadata(archive_path)
        .map_err(|error| error.to_string())?
        .len();
    if archive_size > MAX_RESTORE_ARCHIVE_BYTES {
        return Err("The backup is too large to restore safely".into());
    }
    let file = File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Could not read the backup ZIP: {error}"))?;
    if archive.len() > MAX_RESTORE_ENTRIES {
        return Err("The backup contains too many files".into());
    }
    let manifest: BackupManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|_| "The ZIP is not a Volum backup".to_string())?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err("The backup manifest is too large".into());
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Could not read the backup manifest: {error}"))?
    };
    if manifest.format != BACKUP_FORMAT || manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err("This backup format is not supported by this version of Volum".into());
    }
    let checksums: BTreeMap<String, String> = {
        let mut entry = archive
            .by_name("checksums.json")
            .map_err(|_| "The backup checksum index is missing".to_string())?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err("The backup checksum index is too large".into());
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Could not read the backup checksum index: {error}"))?
    };
    if !manifest
        .files
        .iter()
        .any(|file| file.path == "database/volum.sqlite3")
    {
        return Err("The backup does not contain a Volum database".into());
    }
    let declared_total = manifest
        .files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.byte_size))
        .ok_or_else(|| "The backup size is invalid".to_string())?;
    if declared_total > MAX_RESTORE_BYTES {
        return Err("The backup is too large to restore safely".into());
    }
    if manifest
        .files
        .iter()
        .any(|file| !safe_backup_entry(&file.path))
    {
        return Err("The backup manifest contains an unsafe path".into());
    }
    let expected = manifest
        .files
        .iter()
        .map(|file| (file.path.clone(), (file.byte_size, file.blake3.clone())))
        .collect::<BTreeMap<_, _>>();
    if expected.len() != manifest.files.len() {
        return Err("The backup manifest contains duplicate files".into());
    }
    let manifest_checksums = manifest
        .files
        .iter()
        .map(|file| (file.path.clone(), file.blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    if checksums != manifest_checksums {
        return Err("The backup checksum index does not match its manifest".into());
    }
    std::fs::create_dir_all(staging.join("media")).map_err(|error| error.to_string())?;
    let mut seen = BTreeSet::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_string();
        if !seen.insert(name.clone()) {
            return Err(format!("Duplicate file in backup: {name}"));
        }
        if entry.enclosed_name().is_none() {
            return Err("The backup contains an unsafe path".into());
        }
        if name == "manifest.json" || name == "checksums.json" || entry.is_dir() {
            continue;
        }
        let Some((expected_size, expected_hash)) = expected.get(&name) else {
            return Err(format!("Unexpected file in backup: {name}"));
        };
        if entry.size() != *expected_size {
            return Err(format!(
                "Backup file size does not match its manifest: {name}"
            ));
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "The backup contains an unsafe path".to_string())?
            .to_path_buf();
        let output = staging.join(enclosed);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output_file = File::create(&output).map_err(|error| error.to_string())?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut written = 0_u64;
        loop {
            let count = entry.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            written = written.saturating_add(count as u64);
            if written > *expected_size {
                return Err(format!("Backup file exceeds its declared size: {name}"));
            }
            hasher.update(&buffer[..count]);
            output_file
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
        }
        output_file.sync_all().map_err(|error| error.to_string())?;
        if written != *expected_size {
            return Err(format!(
                "Backup file size does not match its manifest: {name}"
            ));
        }
        if hasher.finalize().to_hex().as_str() != expected_hash {
            return Err(format!("Backup checksum failed: {name}"));
        }
    }
    for name in expected.keys() {
        if !staging.join(name).is_file() {
            return Err(format!("Backup file is missing: {name}"));
        }
    }
    Ok((manifest, archive_size))
}

fn safe_backup_entry(name: &str) -> bool {
    let path = Path::new(name);
    let safe_components = path
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    safe_components && (name == "database/volum.sqlite3" || name.starts_with("media/"))
}

async fn validate_staged_database(
    path: &Path,
    manifest_schema: i64,
    state: &AppState,
) -> CommandResult<()> {
    let options = sqlx::sqlite::SqliteConnectOptions::from_str(path.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?
        .read_only(true)
        .foreign_keys(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| format!("Could not open the restored database: {error}"))?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .map_err(db_error)?;
    if !integrity.eq_ignore_ascii_case("ok") {
        return Err(format!(
            "The restored database failed its integrity check: {integrity}"
        ));
    }
    if sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut connection)
        .await
        .map_err(db_error)?
        .is_some()
    {
        return Err("The restored database contains broken relationships".into());
    }
    for table in ["library_roots", "models", "user_overrides"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&mut connection)
        .await
        .map_err(db_error)?;
        if exists != 1 {
            return Err(format!(
                "The restored database is missing the {table} table"
            ));
        }
    }
    let staged_schema: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&mut connection)
            .await
            .map_err(db_error)?;
    let current_schema: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?;
    if staged_schema != manifest_schema {
        return Err("The backup schema does not match its manifest".into());
    }
    if staged_schema > current_schema {
        return Err("This backup was created by a newer version of Volum. Update Volum before restoring it.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn commit_prepared_restore(
    token: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    validate_token(&token)?;
    let _guard = state.backup_lock.lock().await;
    let staging = state.data_dir.join("restore-staging").join(&token);
    if !staging.join("database/volum.sqlite3").is_file() {
        return Err("The prepared restore is no longer available".into());
    }
    let recovery = state.data_dir.join("recovery");
    tokio::fs::create_dir_all(&recovery)
        .await
        .map_err(|error| error.to_string())?;
    let safety = recovery.join(format!(
        "volum-before-restore-{}-{}.zip",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        &Uuid::new_v4().to_string()[..8]
    ));
    create_backup_archive(&safety, &state).await?;
    let marker = PendingRestore {
        token,
        prepared_at: now(),
    };
    write_json_atomic(&state.data_dir.join("restore-pending.json"), &marker)?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_prepared_restore(
    token: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    validate_token(&token)?;
    let _guard = state.backup_lock.lock().await;
    let _ = tokio::fs::remove_dir_all(state.data_dir.join("restore-staging").join(token)).await;
    Ok(())
}

fn validate_token(token: &str) -> CommandResult<()> {
    Uuid::parse_str(token)
        .map(|_| ())
        .map_err(|_| "Invalid restore token".into())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> CommandResult<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

pub fn apply_pending_restore(data_dir: &Path) -> CommandResult<()> {
    let marker_path = data_dir.join("restore-pending.json");
    if !marker_path.is_file() {
        return Ok(());
    }
    let marker: PendingRestore =
        serde_json::from_slice(&std::fs::read(&marker_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    validate_token(&marker.token)?;
    let staging = data_dir.join("restore-staging").join(&marker.token);
    let staged_database = staging.join("database/volum.sqlite3");
    if !staged_database.is_file() {
        return Err("The pending restore database is missing".into());
    }
    let database = data_dir.join("volum.sqlite3");
    let previous = data_dir.join("volum.sqlite3.pre-restore.bak");
    let database_new = data_dir.join("volum.sqlite3.restore-new");
    let media = data_dir.join("media");
    let old_media = data_dir.join("media.pre-restore");
    let media_new = data_dir.join("media.restore-new");
    let staged_media = staging.join("media");

    let _ = std::fs::remove_file(&database_new);
    std::fs::copy(&staged_database, &database_new)
        .map_err(|error| format!("Could not prepare the restored database: {error}"))?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&database_new)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not flush the restored database: {error}"))?;
    let _ = std::fs::remove_dir_all(&media_new);
    copy_directory(&staged_media, &media_new)
        .map_err(|error| format!("Could not prepare restored Volum media: {error}"))?;

    let _ = std::fs::remove_file(data_dir.join("volum.sqlite3-wal"));
    let _ = std::fs::remove_file(data_dir.join("volum.sqlite3-shm"));
    let _ = std::fs::remove_file(&previous);
    if database.exists() {
        std::fs::rename(&database, &previous)
            .map_err(|error| format!("Could not preserve the current database: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&database_new, &database) {
        if previous.exists() {
            let _ = std::fs::rename(&previous, &database);
        }
        return Err(format!("Could not activate the restored database: {error}"));
    }
    let _ = std::fs::remove_dir_all(&old_media);
    if media.exists() {
        if let Err(error) = std::fs::rename(&media, &old_media) {
            let _ = std::fs::remove_file(&database);
            if previous.exists() {
                let _ = std::fs::rename(&previous, &database);
            }
            let _ = std::fs::remove_dir_all(&media_new);
            return Err(format!("Could not preserve current Volum media: {error}"));
        }
    }
    if let Err(error) = std::fs::rename(&media_new, &media) {
        if old_media.exists() {
            let _ = std::fs::rename(&old_media, &media);
        }
        let _ = std::fs::remove_file(&database);
        if previous.exists() {
            let _ = std::fs::rename(&previous, &database);
        }
        return Err(format!("Could not restore Volum media: {error}"));
    }
    std::fs::remove_file(marker_path).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(staging);
    Ok(())
}

pub fn cleanup_stale_work(data_dir: &Path) {
    for directory in [
        "backup-staging",
        "restore-downloads",
        "restore-staging",
        "media.restore-new",
    ] {
        let _ = std::fs::remove_dir_all(data_dir.join(directory));
    }
    let _ = std::fs::remove_file(data_dir.join("volum.sqlite3.restore-new"));
    let recovery = data_dir.join("recovery");
    let Ok(entries) = std::fs::read_dir(recovery) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".partial") || name.ends_with(".snapshot") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> CommandResult<()> {
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| error.to_string())?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            std::fs::copy(entry.path(), &output).map_err(|error| error.to_string())?;
            std::fs::OpenOptions::new()
                .write(true)
                .open(&output)
                .and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
        } else {
            return Err("Restored media contains an unsupported filesystem entry".into());
        }
    }
    Ok(())
}

pub async fn reconcile_backup_credentials(state: &AppState) -> CommandResult<()> {
    let rows = sqlx::query("SELECT id FROM backup_destinations WHERE kind = 'webdav'")
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    for row in rows {
        let id: String = row.get("id");
        let credential_saved = get_password(&id).await.is_ok();
        sqlx::query("UPDATE backup_destinations SET credential_saved = ? WHERE id = ?")
            .bind(i64::from(credential_saved))
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

fn normalize_webdav_location(input: &str) -> CommandResult<String> {
    let mut url = Url::parse(input.trim()).map_err(|_| "Enter a valid WebDAV URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Use an HTTP or HTTPS WebDAV URL without embedded credentials".into());
    }
    url.set_query(None);
    url.set_fragment(None);
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url.to_string())
}

async fn set_password(id: &str, password: String) -> CommandResult<()> {
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        keyring::Entry::new(KEYRING_SERVICE, &id)
            .map_err(|error| error.to_string())?
            .set_password(&password)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn get_password(id: &str) -> CommandResult<String> {
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        keyring::Entry::new(KEYRING_SERVICE, &id)
            .map_err(|error| error.to_string())?
            .get_password()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn delete_password(id: &str) -> CommandResult<()> {
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &id).map_err(|error| error.to_string())?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn webdav_request(
    destination: &BackupDestination,
    method: Method,
    url: &str,
) -> CommandResult<reqwest::RequestBuilder> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client.request(method, url);
    if let Some(username) = destination.username.as_deref() {
        let password = if destination.credential_saved {
            Some(get_password(&destination.id).await.map_err(|_| {
                "The saved WebDAV password is unavailable. Edit the destination and enter it again."
                    .to_string()
            })?)
        } else {
            None
        };
        request = request.basic_auth(username, password);
    }
    Ok(request)
}

fn schedule_due(destination: &BackupDestination, current: DateTime<Utc>) -> bool {
    let interval = match destination.schedule.as_str() {
        "daily" => ChronoDuration::days(1),
        "weekly" => ChronoDuration::weeks(1),
        "monthly" => ChronoDuration::days(30),
        _ => return false,
    };
    let retrying = destination.last_error.is_some();
    let anchor = if retrying {
        destination.last_attempt_at.as_deref()
    } else {
        destination
            .last_success_at
            .as_deref()
            .or(destination.last_attempt_at.as_deref())
    }
    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
    .map(|value| value.with_timezone(&Utc));
    let due_after = if retrying {
        SCHEDULE_RETRY_DELAY
    } else {
        interval
    };
    anchor.is_none_or(|value| current >= value + due_after)
}

pub fn start_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let state = app.state::<AppState>();
            let destinations = sqlx::query("SELECT * FROM backup_destinations WHERE enabled = 1 AND schedule != 'manual' ORDER BY created_at")
                .fetch_all(&state.pool).await.map(|rows| rows.into_iter().map(destination_from_row).collect::<Vec<_>>());
            if let Ok(destinations) = destinations {
                let due = destinations
                    .into_iter()
                    .filter(|destination| schedule_due(destination, Utc::now()))
                    .collect::<Vec<_>>();
                if !due.is_empty() {
                    let _guard = state.backup_lock.lock().await;
                    for destination in due {
                        let _ = perform_backup(&destination, "scheduled", &state, &app).await;
                    }
                    let _ = app.emit("backups-changed", ());
                }
            }
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_webdav_directories() {
        assert_eq!(
            normalize_webdav_location("https://nas.example.test/backups").unwrap(),
            "https://nas.example.test/backups/"
        );
        assert!(normalize_webdav_location("ftp://nas.example.test/backups").is_err());
        assert!(
            normalize_webdav_location("https://user:password@nas.example.test/backups").is_err()
        );
    }

    #[test]
    fn recognizes_only_volum_archives() {
        assert!(valid_archive_name(
            "volum-backup-20260923T120000Z-abcd1234.zip"
        ));
        assert!(!valid_archive_name("../volum-backup-20260923.zip"));
        assert!(!valid_archive_name("models.zip"));
        assert!(belongs_to_destination(
            "volum-backup-20260923T120000Z-123e4567-abcd1234.zip",
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        assert!(!belongs_to_destination(
            "volum-backup-20260923T120000Z-7654321a-abcd1234.zip",
            "123e4567-e89b-12d3-a456-426614174000"
        ));
    }

    #[tokio::test]
    async fn prunes_only_the_current_destinations_old_backups() {
        let directory = tempfile::tempdir().unwrap();
        let destination = BackupDestination {
            id: "123e4567-e89b-12d3-a456-426614174000".into(),
            name: "Backup".into(),
            kind: "folder".into(),
            location: directory.path().to_string_lossy().to_string(),
            username: None,
            credential_saved: false,
            schedule: "daily".into(),
            retention_count: 2,
            enabled: true,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            created_at: now(),
            updated_at: now(),
        };
        let own = [
            "volum-backup-20260920T120000Z-123e4567-aaaaaaaa.zip",
            "volum-backup-20260921T120000Z-123e4567-bbbbbbbb.zip",
            "volum-backup-20260922T120000Z-123e4567-cccccccc.zip",
        ];
        let other = "volum-backup-20260919T120000Z-7654321a-dddddddd.zip";
        for filename in own.into_iter().chain([other]) {
            std::fs::write(directory.path().join(filename), b"backup").unwrap();
        }
        prune_folder_backups(&destination).await.unwrap();
        assert!(!directory.path().join(own[0]).exists());
        assert!(directory.path().join(own[1]).exists());
        assert!(directory.path().join(own[2]).exists());
        assert!(directory.path().join(other).exists());
    }

    #[test]
    fn computes_schedule_due_dates() {
        let destination = BackupDestination {
            id: "id".into(),
            name: "Backup".into(),
            kind: "folder".into(),
            location: "/tmp".into(),
            username: None,
            credential_saved: false,
            schedule: "daily".into(),
            retention_count: 7,
            enabled: true,
            last_attempt_at: None,
            last_success_at: Some("2026-09-20T12:00:00Z".into()),
            last_error: None,
            created_at: now(),
            updated_at: now(),
        };
        assert!(schedule_due(
            &destination,
            DateTime::parse_from_rfc3339("2026-09-21T12:00:01Z")
                .unwrap()
                .with_timezone(&Utc)
        ));
        assert!(!schedule_due(
            &destination,
            DateTime::parse_from_rfc3339("2026-09-20T18:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        ));
        let mut retrying = destination.clone();
        retrying.last_error = Some("NAS unavailable".into());
        retrying.last_attempt_at = Some("2026-09-21T10:00:00Z".into());
        assert!(!schedule_due(
            &retrying,
            DateTime::parse_from_rfc3339("2026-09-21T10:59:59Z")
                .unwrap()
                .with_timezone(&Utc)
        ));
        assert!(schedule_due(
            &retrying,
            DateTime::parse_from_rfc3339("2026-09-21T11:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        ));
    }

    #[tokio::test]
    async fn creates_and_validates_a_portable_backup() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        sqlx::query("INSERT INTO user_overrides (id, entity_type, entity_id, key, value_json, updated_at) VALUES ('preference', 'app', 'global', 'ui_theme', '\"dark\"', ?)")
            .bind(now()).execute(&state.pool).await.unwrap();
        let media = data.path().join("media/covers");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::write(media.join("cover.png"), b"user-owned-media").unwrap();
        let archive = data.path().join("portable.zip");
        let manifest = create_backup_archive(&archive, &state).await.unwrap();
        assert_eq!(manifest.format, BACKUP_FORMAT);
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "database/volum.sqlite3"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "media/covers/cover.png"));

        let extracted = data.path().join("extracted");
        let (validated, _) = validate_and_extract_backup(&archive, &extracted).unwrap();
        assert_eq!(validated.schema_version, manifest.schema_version);
        assert_eq!(
            std::fs::read(extracted.join("media/covers/cover.png")).unwrap(),
            b"user-owned-media"
        );
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(
            extracted
                .join("database/volum.sqlite3")
                .to_string_lossy()
                .as_ref(),
        )
        .unwrap()
        .read_only(true);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        let value: String =
            sqlx::query_scalar("SELECT value_json FROM user_overrides WHERE key = 'ui_theme'")
                .fetch_one(&mut connection)
                .await
                .unwrap();
        assert_eq!(value, "\"dark\"");
    }

    #[tokio::test]
    async fn rejects_a_tampered_checksum_index() {
        let data = tempfile::tempdir().unwrap();
        let state = AppState::initialize(data.path()).await.unwrap();
        let archive = data.path().join("portable.zip");
        create_backup_archive(&archive, &state).await.unwrap();

        let tampered = data.path().join("tampered.zip");
        let mut source = ZipArchive::new(File::open(&archive).unwrap()).unwrap();
        let output = File::create(&tampered).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..source.len() {
            let mut entry = source.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            writer.start_file(&name, options).unwrap();
            if name == "checksums.json" {
                writer.write_all(b"{}").unwrap();
            } else {
                writer.write_all(&bytes).unwrap();
            }
        }
        writer.finish().unwrap();

        let error =
            validate_and_extract_backup(&tampered, &data.path().join("tampered-out")).unwrap_err();
        assert!(error.contains("checksum index"));
    }

    #[test]
    fn rejects_zip_traversal_even_when_not_in_the_manifest() {
        let data = tempfile::tempdir().unwrap();
        let archive = data.path().join("unsafe.zip");
        let manifest = BackupManifest {
            format: BACKUP_FORMAT.into(),
            format_version: BACKUP_FORMAT_VERSION,
            app_version: "0.1.10".into(),
            schema_version: 1,
            created_at: now(),
            counts: BackupCounts::default(),
            files: vec![ManifestFile {
                path: "database/volum.sqlite3".into(),
                byte_size: 0,
                blake3: blake3::hash(b"").to_hex().to_string(),
            }],
        };
        let checksums = BTreeMap::from([(
            "database/volum.sqlite3".to_string(),
            blake3::hash(b"").to_hex().to_string(),
        )]);
        let output = File::create(&archive).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default();
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        writer.start_file("checksums.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&checksums).unwrap())
            .unwrap();
        writer
            .start_file("database/volum.sqlite3", options)
            .unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"escape").unwrap();
        writer.finish().unwrap();

        let error =
            validate_and_extract_backup(&archive, &data.path().join("unsafe-out")).unwrap_err();
        assert!(error.contains("unsafe path"));
        assert!(!data.path().join("escape.txt").exists());
    }

    #[tokio::test]
    async fn applies_a_prepared_restore_and_replaces_user_media() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("app-data");
        let state = AppState::initialize(&data).await.unwrap();
        sqlx::query("INSERT INTO user_overrides (id, entity_type, entity_id, key, value_json, updated_at) VALUES ('preference', 'app', 'global', 'ui_theme', '\"dark\"', ?)")
            .bind(now()).execute(&state.pool).await.unwrap();
        std::fs::create_dir_all(data.join("media/covers")).unwrap();
        std::fs::write(data.join("media/covers/cover.png"), b"original").unwrap();
        let archive = root.path().join("restore.zip");
        create_backup_archive(&archive, &state).await.unwrap();

        sqlx::query("UPDATE user_overrides SET value_json = '\"light\"' WHERE id = 'preference'")
            .execute(&state.pool)
            .await
            .unwrap();
        std::fs::write(data.join("media/covers/cover.png"), b"changed").unwrap();
        let token = Uuid::new_v4().to_string();
        let staging = data.join("restore-staging").join(&token);
        validate_and_extract_backup(&archive, &staging).unwrap();
        write_json_atomic(
            &data.join("restore-pending.json"),
            &PendingRestore {
                token,
                prepared_at: now(),
            },
        )
        .unwrap();
        state.pool.close().await;
        drop(state);

        apply_pending_restore(&data).unwrap();
        let restored = AppState::initialize(&data).await.unwrap();
        let value: String =
            sqlx::query_scalar("SELECT value_json FROM user_overrides WHERE id = 'preference'")
                .fetch_one(&restored.pool)
                .await
                .unwrap();
        assert_eq!(value, "\"dark\"");
        assert_eq!(
            std::fs::read(data.join("media/covers/cover.png")).unwrap(),
            b"original"
        );
        assert!(data.join("volum.sqlite3.pre-restore.bak").is_file());
        assert!(!data.join("restore-pending.json").exists());
    }
}
