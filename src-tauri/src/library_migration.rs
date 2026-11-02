use crate::{
    companion::CompanionController,
    indexing, project_storage,
    remote::{BackendConnectionStatus, RemoteController},
    state::AppState,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Connection, Row, SqliteConnection, SqlitePool,
};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

pub const MIGRATION_FORMAT_VERSION: u32 = 1;
pub const MIGRATION_CHUNK_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MIGRATION_CHUNK_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_MIGRATION_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_MIGRATION_FILES: usize = 250_000;
const MAX_MIGRATION_BYTES: u64 = 64 * 1024 * 1024 * 1024 * 1024;
const CLIENT_STATE_FILENAME: &str = "library-migration-client.json";
const ACTIVATION_FILENAME: &str = "library-migration-activation.json";
const SERVER_JOBS_DIRECTORY: &str = "library-migrations";
const SERVER_LIBRARY_STAGING: &str = ".volum-migrations";

type MigrationResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStats {
    pub projects: u64,
    pub files: u64,
    pub photos: u64,
    pub collections: u64,
    pub tags: u64,
    pub linked_projects: u64,
    pub managed_projects: u64,
    pub missing_files: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreflight {
    pub stats: MigrationStats,
    pub missing: Vec<MigrationMissingFile>,
    pub unreadable_photos: Vec<String>,
    pub can_migrate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationMissingFile {
    pub project_name: String,
    pub filename: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationManifestFile {
    pub id: String,
    pub kind: String,
    pub relative_path: String,
    pub byte_size: u64,
    pub blake3: Option<String>,
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationManifest {
    pub format_version: u32,
    pub id: String,
    pub created_at: String,
    pub schema_version: i64,
    pub allow_missing: bool,
    pub stats: MigrationStats,
    pub files: Vec<MigrationManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientMigrationFile {
    manifest_id: String,
    source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientMigrationState {
    manifest: MigrationManifest,
    sources: Vec<ClientMigrationFile>,
    #[serde(default)]
    server_id: Option<String>,
    #[serde(default = "default_previous_organization_writes")]
    previous_organization_writes: bool,
}

fn default_previous_organization_writes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerMigrationJob {
    manifest: MigrationManifest,
    state: String,
    target_library_path: String,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationJobStatus {
    pub id: String,
    pub state: String,
    pub stats: MigrationStats,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationServerPreflight {
    pub ready: bool,
    pub schema_version: i64,
    pub project_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationUploadStatus {
    pub uploaded_bytes: u64,
    pub byte_size: u64,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMigration {
    pub id: String,
    pub created_at: String,
    pub stats: MigrationStats,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationProgress {
    stage: String,
    message: String,
    completed_bytes: u64,
    total_bytes: u64,
    completed_files: usize,
    total_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationMarker {
    job_id: String,
    phase: String,
}

#[derive(Debug, Clone)]
struct SourceAsset {
    id: String,
    project_id: String,
    source_root_id: String,
    source_relative_path: String,
    source_path: PathBuf,
    destination: String,
    filename: String,
    byte_size: u64,
    missing: bool,
}

#[derive(Debug, Clone)]
struct SourceMedia {
    id: String,
    source_path: PathBuf,
    relative_path: String,
    byte_size: u64,
}

#[derive(Debug, Clone)]
struct ProjectPlan {
    id: String,
    name: String,
    folder: String,
    folder_id: String,
    trashed: bool,
}

#[derive(Debug)]
struct SourcePlan {
    stats: MigrationStats,
    missing: Vec<MigrationMissingFile>,
    unreadable_photos: Vec<String>,
    assets: Vec<SourceAsset>,
    media: Vec<SourceMedia>,
    projects: Vec<ProjectPlan>,
}

#[tauri::command]
pub async fn inspect_library_migration(
    state: State<'_, AppState>,
) -> MigrationResult<MigrationPreflight> {
    inspect_source(&state).await
}

#[tauri::command]
pub async fn get_pending_library_migration(
    state: State<'_, AppState>,
) -> MigrationResult<Option<PendingMigration>> {
    pending_client_migration(&state.data_dir)
}

#[tauri::command]
pub async fn start_library_migration(
    pairing_payload: String,
    allow_missing: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> MigrationResult<BackendConnectionStatus> {
    if pending_client_migration(&state.data_dir)?.is_some() {
        return Err(
            "Resume or abandon the pending library migration before starting another one".into(),
        );
    }
    let server_id = remote.pair_for_migration(&pairing_payload).await?;
    let value = remote
        .migration_json("/api/v1/migrations/preflight", "GET", None)
        .await?;
    let target: MigrationServerPreflight =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if !target.ready {
        return Err(
            "The NAS already contains projects. Volum will not merge two canonical libraries."
                .into(),
        );
    }
    run_client_migration(
        allow_missing,
        None,
        Some(server_id),
        &state,
        &remote,
        &companion,
        &app,
    )
    .await
}

#[tauri::command]
pub async fn resume_library_migration(
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> MigrationResult<BackendConnectionStatus> {
    let client: ClientMigrationState = read_json(&state.data_dir.join(CLIENT_STATE_FILENAME))?;
    run_client_migration(
        client.manifest.allow_missing,
        Some(client),
        None,
        &state,
        &remote,
        &companion,
        &app,
    )
    .await
}

#[tauri::command]
pub async fn abandon_library_migration(
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> MigrationResult<BackendConnectionStatus> {
    let client: ClientMigrationState = read_json(&state.data_dir.join(CLIENT_STATE_FILENAME))?;
    let server_id = client.server_id.as_deref().ok_or_else(|| {
        "This legacy migration cannot be matched to a NAS automatically. Restore the original connection settings before abandoning it.".to_string()
    })?;
    remote.assert_migration_target(server_id).await?;
    let value = remote
        .migration_json(
            &format!("/api/v1/migrations/{}/cancel", client.manifest.id),
            "POST",
            None,
        )
        .await?;
    let status: MigrationJobStatus =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if status.state != "cancelled" {
        return Err("The NAS did not confirm that the migration was cancelled".into());
    }
    companion
        .set_organization_writes(client.previous_organization_writes)
        .await?;
    let roots: Vec<String> =
        sqlx::query_scalar("SELECT id FROM library_roots WHERE status != 'removed'")
            .fetch_all(&state.pool)
            .await
            .map_err(|error| error.to_string())?;
    resume_local_indexing(&state, &roots, &app).await;
    let status = remote.restore_migration_local_mode().await?;
    clear_client_state(&state.data_dir);
    Ok(status)
}

#[tauri::command]
pub async fn rollback_library_migration(
    nas_stopped: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    app: AppHandle,
) -> MigrationResult<BackendConnectionStatus> {
    if !nas_stopped {
        return Err("Stop volumd before restoring the Desktop rollback library".into());
    }
    let status = remote.restore_retired_local_library().await?;
    let roots: Vec<String> =
        sqlx::query_scalar("SELECT id FROM library_roots WHERE status != 'removed'")
            .fetch_all(&state.pool)
            .await
            .map_err(|error| error.to_string())?;
    resume_local_indexing(&state, &roots, &app).await;
    Ok(status)
}

async fn run_client_migration(
    allow_missing: bool,
    existing: Option<ClientMigrationState>,
    new_server_id: Option<String>,
    state: &AppState,
    remote: &RemoteController,
    companion: &CompanionController,
    app: &AppHandle,
) -> MigrationResult<BackendConnectionStatus> {
    if let Some(client) = existing.as_ref() {
        let server_id = client.server_id.as_deref().ok_or_else(|| {
            "This migration was created by an older Volum build and is not bound to a NAS. Abandon it and start a new migration.".to_string()
        })?;
        remote.assert_migration_target(server_id).await?;
    }
    let lock = state.file_operation_lock.clone();
    let _file_guard = lock.lock().await;
    let roots = pause_local_indexing(state).await?;
    let previous_writes = companion.status().await.organization_writes;
    if let Err(error) = companion.set_organization_writes(false).await {
        resume_local_indexing(state, &roots, app).await;
        return Err(error);
    }
    let client = match existing {
        Some(client) => client,
        None => match prepare_client_migration(
            state,
            allow_missing,
            new_server_id.ok_or_else(|| "The migration NAS identity is missing".to_string())?,
            previous_writes,
            app,
        )
        .await
        {
            Ok(client) => client,
            Err(error) => {
                let _ = companion.set_organization_writes(previous_writes).await;
                resume_local_indexing(state, &roots, app).await;
                return Err(error);
            }
        },
    };
    let restore_writes = client.previous_organization_writes;
    match transfer_client_migration(&client, remote, app).await {
        Ok(status) => {
            if status.id != client.manifest.id || status.state != "complete" {
                return Err("The NAS did not provide completion proof for this migration".into());
            }
            companion.set_enabled(false).await?;
            let server_id = client
                .server_id
                .as_deref()
                .ok_or_else(|| "The migration NAS identity is missing".to_string())?;
            let backend = remote.retire_local_library(&status.id, server_id).await?;
            clear_client_state(&state.data_dir);
            Ok(backend)
        }
        Err(error) => {
            let target_state = remote
                .migration_status(&client.manifest.id)
                .await
                .ok()
                .map(|status| status.state);
            if !target_state
                .as_deref()
                .is_some_and(|state| matches!(state, "activating" | "complete"))
            {
                let _ = companion.set_organization_writes(restore_writes).await;
                resume_local_indexing(state, &roots, app).await;
            }
            Err(error)
        }
    }
}

pub async fn inspect_source(state: &AppState) -> MigrationResult<MigrationPreflight> {
    let plan = build_source_plan(&state.pool, &state.data_dir).await?;
    Ok(MigrationPreflight {
        stats: plan.stats,
        can_migrate: plan.unreadable_photos.is_empty(),
        missing: plan.missing,
        unreadable_photos: plan.unreadable_photos,
    })
}

pub fn pending_client_migration(data_dir: &Path) -> MigrationResult<Option<PendingMigration>> {
    let path = data_dir.join(CLIENT_STATE_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }
    let client: ClientMigrationState = read_json(&path)?;
    Ok(Some(PendingMigration {
        id: client.manifest.id,
        created_at: client.manifest.created_at,
        stats: client.manifest.stats,
        server_id: client.server_id,
    }))
}

pub fn has_pending_client_migration(remote_config_path: &Path) -> bool {
    remote_config_path
        .parent()
        .is_some_and(|data_dir| data_dir.join(CLIENT_STATE_FILENAME).is_file())
}

async fn prepare_client_migration(
    state: &AppState,
    allow_missing: bool,
    server_id: String,
    previous_organization_writes: bool,
    app: &AppHandle,
) -> MigrationResult<ClientMigrationState> {
    let preflight = build_source_plan(&state.pool, &state.data_dir).await?;
    if !preflight.unreadable_photos.is_empty() {
        return Err("Some project photos are missing or unreadable. Restore them before moving this library.".into());
    }
    if preflight.stats.missing_files > 0 && !allow_missing {
        return Err(
            "Reconnect or explicitly accept every missing original before moving this library."
                .into(),
        );
    }

    let id = Uuid::new_v4().to_string();
    let directory = state.data_dir.join("library-migration-source").join(&id);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let database = directory.join("volum.sqlite3");
    if database.exists() {
        fs::remove_file(&database).map_err(|error| error.to_string())?;
    }
    emit_progress(
        app,
        "snapshot",
        "Freezing a consistent library snapshot",
        0,
        preflight.stats.total_bytes,
        0,
        preflight.assets.len() + preflight.media.len() + 1,
    );
    sqlx::query("VACUUM INTO ?")
        .bind(database.to_string_lossy().to_string())
        .execute(&state.pool)
        .await
        .map_err(|error| format!("Could not snapshot the Desktop library: {error}"))?;

    let options = SqliteConnectOptions::from_str(database.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| error.to_string())?;
    let plan = build_source_plan(&pool, &state.data_dir).await?;

    let mut manifest_files = Vec::with_capacity(plan.assets.len() + plan.media.len() + 1);
    let mut sources = Vec::with_capacity(plan.assets.len() + plan.media.len() + 1);
    let mut completed_bytes = 0u64;
    let total_files =
        plan.assets.iter().filter(|file| !file.missing).count() + plan.media.len() + 1;
    let mut completed_files = 0usize;
    let mut asset_hashes = HashMap::new();

    for asset in &plan.assets {
        if asset.missing {
            manifest_files.push(MigrationManifestFile {
                id: asset.id.clone(),
                kind: "asset".into(),
                relative_path: asset.destination.clone(),
                byte_size: asset.byte_size,
                blake3: None,
                missing: true,
            });
            continue;
        }
        let hash = hash_file(&asset.source_path)?;
        let actual_size = fs::metadata(&asset.source_path)
            .map_err(|error| error.to_string())?
            .len();
        if actual_size != asset.byte_size {
            return Err(format!(
                "{} changed while the migration was being prepared. Rescan and try again.",
                asset.filename
            ));
        }
        asset_hashes.insert(asset.id.clone(), hash.clone());
        manifest_files.push(MigrationManifestFile {
            id: asset.id.clone(),
            kind: "asset".into(),
            relative_path: asset.destination.clone(),
            byte_size: actual_size,
            blake3: Some(hash),
            missing: false,
        });
        sources.push(ClientMigrationFile {
            manifest_id: asset.id.clone(),
            source_path: asset.source_path.to_string_lossy().to_string(),
        });
        completed_bytes = completed_bytes.saturating_add(actual_size);
        completed_files += 1;
        emit_progress(
            app,
            "hashing",
            &format!("Checking {}", asset.filename),
            completed_bytes,
            plan.stats.total_bytes,
            completed_files,
            total_files,
        );
    }
    for media in &plan.media {
        let hash = hash_file(&media.source_path)?;
        manifest_files.push(MigrationManifestFile {
            id: media.id.clone(),
            kind: "media".into(),
            relative_path: media.relative_path.clone(),
            byte_size: media.byte_size,
            blake3: Some(hash),
            missing: false,
        });
        sources.push(ClientMigrationFile {
            manifest_id: media.id.clone(),
            source_path: media.source_path.to_string_lossy().to_string(),
        });
        completed_bytes = completed_bytes.saturating_add(media.byte_size);
        completed_files += 1;
        emit_progress(
            app,
            "hashing",
            "Checking project photos",
            completed_bytes,
            plan.stats.total_bytes,
            completed_files,
            total_files,
        );
    }

    transform_snapshot(&pool, &plan, &asset_hashes).await?;
    pool.close().await;
    remove_sqlite_sidecars(&database);
    let database_size = fs::metadata(&database)
        .map_err(|error| error.to_string())?
        .len();
    let database_hash = hash_file(&database)?;
    manifest_files.push(MigrationManifestFile {
        id: "database".into(),
        kind: "database".into(),
        relative_path: "database/volum.sqlite3".into(),
        byte_size: database_size,
        blake3: Some(database_hash),
        missing: false,
    });
    sources.push(ClientMigrationFile {
        manifest_id: "database".into(),
        source_path: database.to_string_lossy().to_string(),
    });

    let schema_version = snapshot_schema_version(&database).await?;
    let mut stats = plan.stats;
    stats.total_bytes = manifest_files
        .iter()
        .filter(|file| !file.missing)
        .map(|file| file.byte_size)
        .sum();
    let client = ClientMigrationState {
        manifest: MigrationManifest {
            format_version: MIGRATION_FORMAT_VERSION,
            id,
            created_at: Utc::now().to_rfc3339(),
            schema_version,
            allow_missing,
            stats,
            files: manifest_files,
        },
        sources,
        server_id: Some(server_id),
        previous_organization_writes,
    };
    write_json_atomic(&state.data_dir.join(CLIENT_STATE_FILENAME), &client)?;
    Ok(client)
}

async fn transfer_client_migration(
    client: &ClientMigrationState,
    remote: &RemoteController,
    app: &AppHandle,
) -> MigrationResult<MigrationJobStatus> {
    let manifest_value =
        serde_json::to_value(&client.manifest).map_err(|error| error.to_string())?;
    let mut status = match remote.migration_status(&client.manifest.id).await {
        Ok(existing) if existing.state != "failed" => existing,
        Ok(_) | Err(_) => {
            let status_value = remote
                .migration_json("/api/v1/migrations", "POST", Some(&manifest_value))
                .await?;
            serde_json::from_value(status_value).map_err(|error| error.to_string())?
        }
    };
    if status.state == "complete" {
        let finished = remote
            .migration_json(
                &format!("/api/v1/migrations/{}/finish", client.manifest.id),
                "POST",
                None,
            )
            .await?;
        return serde_json::from_value(finished).map_err(|error| error.to_string());
    }
    if status.state == "activating" {
        return wait_for_server_activation(client, remote).await;
    }
    if status.state == "failed" {
        return Err(status
            .error
            .unwrap_or_else(|| "The NAS rejected this migration".into()));
    }

    let source_by_id = client
        .sources
        .iter()
        .map(|source| (source.manifest_id.as_str(), source))
        .collect::<HashMap<_, _>>();
    let total_files = client
        .manifest
        .files
        .iter()
        .filter(|file| !file.missing)
        .count();
    let mut overall_uploaded = status.uploaded_bytes;
    for (completed_files, file) in client
        .manifest
        .files
        .iter()
        .filter(|file| !file.missing)
        .enumerate()
    {
        let source = source_by_id
            .get(file.id.as_str())
            .ok_or_else(|| format!("Migration source {} is unavailable", file.id))?;
        let file_status_value = remote
            .migration_json(
                &format!(
                    "/api/v1/migrations/{}/files/{}",
                    client.manifest.id, file.id
                ),
                "GET",
                None,
            )
            .await?;
        let file_status: MigrationUploadStatus =
            serde_json::from_value(file_status_value).map_err(|error| error.to_string())?;
        let mut offset = file_status.uploaded_bytes;
        if offset > file.byte_size {
            return Err(format!(
                "The NAS has an invalid partial copy of {}",
                file.relative_path
            ));
        }
        let mut input = tokio::fs::File::open(&source.source_path)
            .await
            .map_err(|error| format!("Could not open {}: {error}", source.source_path))?;
        input
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|error| error.to_string())?;
        let mut buffer = vec![0u8; MIGRATION_CHUNK_BYTES];
        while offset < file.byte_size {
            let remaining = (file.byte_size - offset).min(buffer.len() as u64) as usize;
            let read = input
                .read(&mut buffer[..remaining])
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err(format!(
                    "{} changed or became unavailable during transfer",
                    file.relative_path
                ));
            }
            remote
                .migration_upload_chunk(&client.manifest.id, &file.id, offset, &buffer[..read])
                .await?;
            offset += read as u64;
            overall_uploaded = overall_uploaded.saturating_add(read as u64);
            emit_progress(
                app,
                "transferring",
                &format!("Copying {}", file.relative_path),
                overall_uploaded,
                status.total_bytes,
                completed_files,
                total_files,
            );
        }
    }
    status.uploaded_bytes = overall_uploaded;

    emit_progress(
        app,
        "verifying",
        "The NAS is checking every file",
        status.total_bytes,
        status.total_bytes,
        total_files,
        total_files,
    );
    let verify_value = remote
        .migration_json(
            &format!("/api/v1/migrations/{}/verify", client.manifest.id),
            "POST",
            None,
        )
        .await?;
    status = serde_json::from_value(verify_value).map_err(|error| error.to_string())?;
    let verify_deadline = tokio::time::Instant::now() + Duration::from_secs(6 * 60 * 60);
    while status.state == "verifying" || status.state == "receiving" {
        if tokio::time::Instant::now() >= verify_deadline {
            return Err(
                "The NAS did not finish verification in time. The migration can be resumed.".into(),
            );
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        status = remote.migration_status(&client.manifest.id).await?;
    }
    if status.state == "failed" {
        return Err(status
            .error
            .unwrap_or_else(|| "The NAS could not verify the migration".into()));
    }
    if status.state != "ready" && status.state != "complete" {
        return Err(format!(
            "The migration entered an unexpected state: {}",
            status.state
        ));
    }
    if status.state == "ready" {
        emit_progress(
            app,
            "activating",
            "Making the NAS the canonical library",
            status.total_bytes,
            status.total_bytes,
            total_files,
            total_files,
        );
        let _activate_value = remote
            .migration_json(
                &format!("/api/v1/migrations/{}/activate", client.manifest.id),
                "POST",
                None,
            )
            .await?;
    }
    wait_for_server_activation(client, remote).await
}

async fn wait_for_server_activation(
    client: &ClientMigrationState,
    remote: &RemoteController,
) -> MigrationResult<MigrationJobStatus> {
    let activation_deadline = tokio::time::Instant::now() + Duration::from_secs(5 * 60);
    loop {
        if tokio::time::Instant::now() >= activation_deadline {
            return Err("The NAS is still restarting. Keep this Desktop frozen and choose Resume migration when it is online.".into());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        match remote.migration_status(&client.manifest.id).await {
            Ok(next) if next.state == "complete" => {
                let finished = remote
                    .migration_json(
                        &format!("/api/v1/migrations/{}/finish", client.manifest.id),
                        "POST",
                        None,
                    )
                    .await?;
                return serde_json::from_value(finished).map_err(|error| error.to_string());
            }
            Ok(next) if next.state == "failed" => {
                return Err(next
                    .error
                    .unwrap_or_else(|| "The NAS could not activate the migration".into()))
            }
            Ok(_) | Err(_) => continue,
        }
    }
}

pub async fn server_preflight(state: &AppState) -> MigrationResult<MigrationServerPreflight> {
    let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models")
        .fetch_one(&state.pool)
        .await
        .map_err(|error| error.to_string())?;
    let schema_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&state.pool)
            .await
            .map_err(|error| error.to_string())?;
    Ok(MigrationServerPreflight {
        ready: project_count == 0,
        schema_version,
        project_count,
    })
}

pub async fn create_server_job(
    manifest: MigrationManifest,
    state: &AppState,
) -> MigrationResult<MigrationJobStatus> {
    validate_manifest(&manifest)?;
    let preflight = server_preflight(state).await?;
    if !preflight.ready {
        return Err(
            "The NAS already contains projects. Volum never merges two canonical libraries.".into(),
        );
    }
    if preflight.schema_version != manifest.schema_version {
        return Err(format!("Desktop and NAS database versions differ (Desktop {}, NAS {}). Update both before migrating.", manifest.schema_version, preflight.schema_version));
    }
    let target_library_path: String = sqlx::query_scalar("SELECT path FROM library_roots WHERE root_kind = 'managed' AND status != 'removed' ORDER BY created_at LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Configure the NAS managed library before migrating".to_string())?;
    let library_bytes: u64 = manifest
        .files
        .iter()
        .filter(|file| file.kind == "asset" && !file.missing)
        .map(|file| file.byte_size)
        .sum();
    let config_bytes: u64 = manifest
        .files
        .iter()
        .filter(|file| file.kind != "asset" && !file.missing)
        .map(|file| file.byte_size)
        .sum();
    let activation_database_copy: u64 = manifest
        .files
        .iter()
        .filter(|file| file.kind == "database" && !file.missing)
        .map(|file| file.byte_size)
        .sum();
    let required_config_bytes = config_bytes
        .checked_add(activation_database_copy)
        .ok_or_else(|| "The migration is too large".to_string())?;
    let library_available = fs2::available_space(&target_library_path)
        .map_err(|error| format!("Could not check NAS library capacity: {error}"))?;
    let config_available = fs2::available_space(&state.data_dir)
        .map_err(|error| format!("Could not check NAS configuration capacity: {error}"))?;
    if library_bytes > library_available {
        return Err(format!(
            "The NAS library needs {} more bytes of free space",
            library_bytes - library_available
        ));
    }
    if required_config_bytes > config_available {
        return Err(format!(
            "The NAS configuration volume needs {} more bytes of free space",
            required_config_bytes - config_available
        ));
    }
    let path = server_job_path(&state.data_dir, &manifest.id);
    if path.is_file() {
        let mut existing: ServerMigrationJob = read_json(&path)?;
        if existing.manifest != manifest {
            return Err("This migration identity is already in use by another manifest".into());
        }
        if existing.state == "failed" {
            reset_failed_server_job(&mut existing, &state.data_dir)?;
        }
        return server_job_status(&existing, &state.data_dir);
    }
    reject_other_active_jobs(&state.data_dir, &manifest.id)?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| "Invalid migration path".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::create_dir_all(
        server_job_directory(&state.data_dir, &manifest.id).join("staging/database"),
    )
    .map_err(|error| error.to_string())?;
    fs::create_dir_all(server_job_directory(&state.data_dir, &manifest.id).join("staging/media"))
        .map_err(|error| error.to_string())?;
    let library_stage = server_library_stage(Path::new(&target_library_path), &manifest.id);
    ensure_server_library_layout(&library_stage)?;
    let now = Utc::now().to_rfc3339();
    let job = ServerMigrationJob {
        manifest,
        state: "receiving".into(),
        target_library_path,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        error: None,
    };
    for file in job
        .manifest
        .files
        .iter()
        .filter(|file| !file.missing && file.byte_size == 0)
    {
        let destination = server_staged_file(&state.data_dir, &job, file)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::File::create(destination)
            .and_then(|output| output.sync_all())
            .map_err(|error| error.to_string())?;
    }
    write_json_atomic(&path, &job)?;
    server_job_status(&job, &state.data_dir)
}

fn reset_failed_server_job(job: &mut ServerMigrationJob, data_dir: &Path) -> MigrationResult<()> {
    if job.state != "failed" {
        return Err("Only a failed migration can be reset".into());
    }
    let job_directory = server_job_directory(data_dir, &job.manifest.id);
    let config_stage = job_directory.join("staging");
    let library_stage = server_library_stage(Path::new(&job.target_library_path), &job.manifest.id);
    let _ = fs::remove_dir_all(&config_stage);
    let _ = fs::remove_dir_all(library_stage.parent().unwrap_or(library_stage.as_path()));
    fs::create_dir_all(config_stage.join("database")).map_err(|error| error.to_string())?;
    fs::create_dir_all(config_stage.join("media")).map_err(|error| error.to_string())?;
    ensure_server_library_layout(&library_stage)?;
    for file in job
        .manifest
        .files
        .iter()
        .filter(|file| !file.missing && file.byte_size == 0)
    {
        let destination = server_staged_file(data_dir, job, file)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::File::create(destination)
            .and_then(|output| output.sync_all())
            .map_err(|error| error.to_string())?;
    }
    job.state = "receiving".into();
    job.error = None;
    job.completed_at = None;
    job.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&server_job_path(data_dir, &job.manifest.id), job)
}

pub fn cancel_server_job(data_dir: &Path, id: &str) -> MigrationResult<MigrationJobStatus> {
    validate_uuid(id)?;
    let path = server_job_path(data_dir, id);
    let mut job: ServerMigrationJob = read_json(&path)?;
    if matches!(job.state.as_str(), "activating" | "complete") {
        return Err("A migration that is activating or complete cannot be abandoned".into());
    }
    if job.state != "cancelled" {
        let _ = fs::remove_dir_all(server_job_directory(data_dir, id).join("staging"));
        let library_stage = server_library_stage(Path::new(&job.target_library_path), id);
        let _ = fs::remove_dir_all(library_stage.parent().unwrap_or(library_stage.as_path()));
        job.state = "cancelled".into();
        job.error = None;
        job.updated_at = Utc::now().to_rfc3339();
        write_json_atomic(&path, &job)?;
    }
    server_job_status(&job, data_dir)
}

pub fn get_server_job(data_dir: &Path, id: &str) -> MigrationResult<MigrationJobStatus> {
    validate_uuid(id)?;
    let job: ServerMigrationJob = read_json(&server_job_path(data_dir, id))?;
    server_job_status(&job, data_dir)
}

pub fn server_upload_status(
    data_dir: &Path,
    id: &str,
    file_id: &str,
) -> MigrationResult<MigrationUploadStatus> {
    validate_uuid(id)?;
    let job: ServerMigrationJob = read_json(&server_job_path(data_dir, id))?;
    let file = manifest_file(&job, file_id)?;
    let path = server_staged_file(data_dir, &job, file)?;
    let uploaded_bytes = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    Ok(MigrationUploadStatus {
        uploaded_bytes,
        byte_size: file.byte_size,
        complete: uploaded_bytes == file.byte_size,
    })
}

pub fn append_server_chunk(
    data_dir: &Path,
    id: &str,
    file_id: &str,
    offset: u64,
    bytes: &[u8],
) -> MigrationResult<MigrationUploadStatus> {
    if bytes.is_empty() || bytes.len() > MAX_MIGRATION_CHUNK_BYTES {
        return Err("The migration chunk size is invalid".into());
    }
    validate_uuid(id)?;
    let path = server_job_path(data_dir, id);
    let mut job: ServerMigrationJob = read_json(&path)?;
    if job.state != "receiving" {
        return Err("This migration is not accepting files".into());
    }
    let file = manifest_file(&job, file_id)?.clone();
    if file.missing {
        return Err("A missing source file cannot be uploaded".into());
    }
    let destination = server_staged_file(data_dir, &job, &file)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let current = fs::metadata(&destination)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if current != offset {
        return Err(format!("Resume this file at byte {current}"));
    }
    if current.saturating_add(bytes.len() as u64) > file.byte_size {
        return Err("The uploaded file exceeds its declared size".into());
    }
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&destination)
        .map_err(|error| error.to_string())?;
    output.write_all(bytes).map_err(|error| error.to_string())?;
    output.sync_data().map_err(|error| error.to_string())?;
    let uploaded_bytes = current + bytes.len() as u64;
    job.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&path, &job)?;
    Ok(MigrationUploadStatus {
        uploaded_bytes,
        byte_size: file.byte_size,
        complete: uploaded_bytes == file.byte_size,
    })
}

pub async fn verify_server_job(data_dir: &Path, id: &str) -> MigrationResult<MigrationJobStatus> {
    validate_uuid(id)?;
    let path = server_job_path(data_dir, id);
    let mut job: ServerMigrationJob = read_json(&path)?;
    if job.state == "ready" || job.state == "complete" {
        return server_job_status(&job, data_dir);
    }
    if job.state != "receiving" && job.state != "verifying" {
        return Err("This migration cannot be verified in its current state".into());
    }
    job.state = "verifying".into();
    job.error = None;
    job.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&path, &job)?;
    let result = verify_server_files_and_database(data_dir, &job).await;
    let mut latest: ServerMigrationJob = read_json(&path)?;
    match result {
        Ok(()) => {
            latest.state = "ready".into();
            latest.error = None;
        }
        Err(error) => {
            latest.state = "failed".into();
            latest.error = Some(error);
        }
    }
    latest.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&path, &latest)?;
    server_job_status(&latest, data_dir)
}

pub fn prepare_server_activation(data_dir: &Path, id: &str) -> MigrationResult<MigrationJobStatus> {
    validate_uuid(id)?;
    let path = server_job_path(data_dir, id);
    let mut job: ServerMigrationJob = read_json(&path)?;
    if job.state == "complete" || job.state == "activating" {
        return server_job_status(&job, data_dir);
    }
    if job.state != "ready" {
        return Err("Verify the complete migration before activating it".into());
    }
    let marker = ActivationMarker {
        job_id: id.to_string(),
        phase: "prepared".into(),
    };
    write_json_atomic(&data_dir.join(ACTIVATION_FILENAME), &marker)?;
    job.state = "activating".into();
    job.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&path, &job)?;
    server_job_status(&job, data_dir)
}

pub fn apply_pending_activation(data_dir: &Path, library_dir: &Path) -> MigrationResult<()> {
    let marker_path = data_dir.join(ACTIVATION_FILENAME);
    if !marker_path.is_file() {
        return Ok(());
    }
    let mut marker: ActivationMarker = read_json(&marker_path)?;
    validate_uuid(&marker.job_id)?;
    let job_path = server_job_path(data_dir, &marker.job_id);
    let mut job: ServerMigrationJob = read_json(&job_path)?;
    let stage_library = server_library_stage(library_dir, &marker.job_id);
    let stage_config = server_job_directory(data_dir, &marker.job_id).join("staging");
    let rollback_config = data_dir.join("migration-rollbacks").join(&marker.job_id);
    let rollback_library = library_dir.join(".volum-rollbacks").join(&marker.job_id);
    fs::create_dir_all(&rollback_config).map_err(|error| error.to_string())?;
    fs::create_dir_all(&rollback_library).map_err(|error| error.to_string())?;

    if marker.phase == "prepared" {
        activate_directory_child(&stage_config, data_dir, &rollback_config, "media")?;
        for child in ["Projects", "Inbox", "Trash", ".volum-library.json"] {
            activate_directory_child(&stage_library, library_dir, &rollback_library, child)?;
        }
        marker.phase = "filesActive".into();
        write_json_atomic(&marker_path, &marker)?;
    }

    if marker.phase == "filesActive" {
        let staged_database = stage_config.join("database/volum.sqlite3");
        if !staged_database.is_file() {
            return Err("The verified migration database is missing".into());
        }
        let active_database = data_dir.join("volum.sqlite3");
        let rollback_database = rollback_config.join("volum.sqlite3");
        let new_database = data_dir.join("volum.sqlite3.migration-new");
        if !rollback_database.exists() {
            let _ = fs::remove_file(&new_database);
            fs::copy(&staged_database, &new_database).map_err(|error| error.to_string())?;
            OpenOptions::new()
                .write(true)
                .open(&new_database)
                .and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
            let _ = fs::remove_file(data_dir.join("volum.sqlite3-wal"));
            let _ = fs::remove_file(data_dir.join("volum.sqlite3-shm"));
            if active_database.exists() {
                fs::rename(&active_database, &rollback_database)
                    .map_err(|error| error.to_string())?;
            }
            fs::rename(&new_database, &active_database).map_err(|error| error.to_string())?;
        } else if !active_database.exists() {
            fs::copy(&staged_database, &active_database).map_err(|error| error.to_string())?;
        }
        marker.phase = "databaseActive".into();
        write_json_atomic(&marker_path, &marker)?;
    }

    job.state = "complete".into();
    job.error = None;
    job.completed_at = Some(Utc::now().to_rfc3339());
    job.updated_at = Utc::now().to_rfc3339();
    write_json_atomic(&job_path, &job)?;
    let _ = fs::remove_file(marker_path);
    let _ = fs::remove_dir_all(stage_library.parent().unwrap_or(&stage_library));
    let _ = fs::remove_dir_all(stage_config);
    Ok(())
}

pub async fn pause_local_indexing(state: &AppState) -> MigrationResult<Vec<String>> {
    let roots: Vec<String> =
        sqlx::query_scalar("SELECT id FROM library_roots WHERE status != 'removed'")
            .fetch_all(&state.pool)
            .await
            .map_err(|error| error.to_string())?;
    for root in &roots {
        indexing::pause(root, state).await?;
    }
    state
        .watchers
        .lock()
        .map_err(|_| "Could not freeze filesystem watchers".to_string())?
        .clear();
    Ok(roots)
}

pub async fn resume_local_indexing(state: &AppState, roots: &[String], app: &AppHandle) {
    for root in roots {
        let _ = indexing::start(root.clone(), state, Some(app.clone())).await;
    }
}

pub fn clear_client_state(data_dir: &Path) {
    let state_path = data_dir.join(CLIENT_STATE_FILENAME);
    if let Ok(client) = read_json::<ClientMigrationState>(&state_path) {
        if validate_uuid(&client.manifest.id).is_ok() {
            let _ = fs::remove_dir_all(
                data_dir
                    .join("library-migration-source")
                    .join(client.manifest.id),
            );
        }
    }
    let _ = fs::remove_file(state_path);
}

fn emit_progress(
    app: &AppHandle,
    stage: &str,
    message: &str,
    completed_bytes: u64,
    total_bytes: u64,
    completed_files: usize,
    total_files: usize,
) {
    let _ = app.emit(
        "library-migration-progress",
        MigrationProgress {
            stage: stage.into(),
            message: message.into(),
            completed_bytes,
            total_bytes,
            completed_files,
            total_files,
        },
    );
}

async fn build_source_plan(pool: &SqlitePool, data_dir: &Path) -> MigrationResult<SourcePlan> {
    let project_rows = sqlx::query("SELECT id, display_name, storage_mode, trashed_at FROM models ORDER BY lower(display_name), id")
        .fetch_all(pool).await.map_err(|error| error.to_string())?;
    let mut projects = Vec::with_capacity(project_rows.len());
    let mut linked_projects = 0u64;
    let mut used_folders = HashSet::new();
    for row in project_rows {
        let id: String = row.get("id");
        let name: String = row.get("display_name");
        let trashed = row.get::<Option<String>, _>("trashed_at").is_some();
        if row.get::<String, _>("storage_mode") == "linked" {
            linked_projects += 1;
        }
        let mut component = format!("{}-{}", safe_component(&name, 72), short_id(&id));
        let mut suffix = 2;
        while !used_folders.insert(component.to_ascii_lowercase()) {
            component = format!("{}-{}-{suffix}", safe_component(&name, 64), short_id(&id));
            suffix += 1;
        }
        let folder = format!(
            "{}/{}",
            if trashed { "Trash" } else { "Projects" },
            component
        );
        projects.push(ProjectPlan {
            folder_id: id.clone(),
            id,
            name,
            folder,
            trashed,
        });
    }
    let project_by_id = projects
        .iter()
        .map(|project| (project.id.as_str(), project))
        .collect::<HashMap<_, _>>();
    let asset_rows = sqlx::query("SELECT a.id, a.root_id, a.filename, a.relative_path, a.byte_size, a.missing_since, r.path root_path, ma.model_id FROM assets a JOIN library_roots r ON r.id = a.root_id JOIN model_assets ma ON ma.asset_id = a.id ORDER BY ma.model_id, lower(a.filename), a.id")
        .fetch_all(pool).await.map_err(|error| error.to_string())?;
    let database_asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    if database_asset_count.max(0) as usize != asset_rows.len() {
        return Err("Some indexed files are not attached to a project. Rescan the library before migrating it.".into());
    }
    let mut assets = Vec::with_capacity(asset_rows.len());
    let mut missing = Vec::new();
    let mut used_asset_names = HashMap::<String, HashSet<String>>::new();
    let mut total_bytes = 0u64;
    for row in asset_rows {
        let id: String = row.get("id");
        let project_id: String = row.get("model_id");
        let project = project_by_id
            .get(project_id.as_str())
            .ok_or_else(|| "An asset belongs to an unknown project".to_string())?;
        let root = PathBuf::from(row.get::<String, _>("root_path"));
        let relative: String = row.get("relative_path");
        let source_path = checked_source_path(&root, &relative)?;
        let original_filename: String = row.get("filename");
        let names = used_asset_names.entry(project_id.clone()).or_default();
        let mut filename = safe_filename(&original_filename);
        if !names.insert(filename.to_ascii_lowercase()) {
            filename = disambiguated_filename(&filename, &id);
            names.insert(filename.to_ascii_lowercase());
        }
        let database_missing = row.get::<Option<String>, _>("missing_since").is_some();
        let metadata = fs::metadata(&source_path)
            .ok()
            .filter(|metadata| metadata.is_file());
        let actual_missing = metadata.is_none();
        let byte_size = metadata
            .as_ref()
            .map(|metadata| metadata.len())
            .unwrap_or_else(|| row.get::<i64, _>("byte_size").max(0) as u64);
        let is_missing = database_missing || actual_missing;
        if is_missing {
            missing.push(MigrationMissingFile {
                project_name: project.name.clone(),
                filename: original_filename.clone(),
                source: source_path.to_string_lossy().to_string(),
            });
        } else {
            total_bytes = total_bytes
                .checked_add(byte_size)
                .ok_or_else(|| "The library is too large to migrate".to_string())?;
        }
        assets.push(SourceAsset {
            id,
            project_id,
            source_root_id: row.get("root_id"),
            source_relative_path: relative,
            source_path,
            destination: format!("{}/{}", project.folder, filename),
            filename,
            byte_size,
            missing: is_missing,
        });
    }

    let photo_rows =
        sqlx::query("SELECT id, media_path, thumbnail_path FROM project_photos ORDER BY id")
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())?;
    let photo_count = photo_rows.len() as u64;
    let mut media = Vec::new();
    let mut unreadable_photos = Vec::new();
    let mut seen_media = HashSet::new();
    for row in photo_rows {
        let photo_id: String = row.get("id");
        for (variant, relative) in [
            ("original", row.get::<String, _>("media_path")),
            ("thumbnail", row.get::<String, _>("thumbnail_path")),
        ] {
            if !seen_media.insert(relative.clone()) {
                continue;
            }
            let source_path = checked_data_path(data_dir, &relative)?;
            let Some(metadata) = fs::metadata(&source_path)
                .ok()
                .filter(|metadata| metadata.is_file())
            else {
                unreadable_photos.push(relative);
                continue;
            };
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "The library is too large to migrate".to_string())?;
            media.push(SourceMedia {
                id: format!("photo-{photo_id}-{variant}"),
                source_path,
                relative_path: relative,
                byte_size: metadata.len(),
            });
        }
    }
    let collections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM collections")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    let tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    let managed_projects = projects.len() as u64 - linked_projects;
    let stats = MigrationStats {
        projects: projects.len() as u64,
        files: assets.len() as u64,
        photos: photo_count,
        collections: collections.max(0) as u64,
        tags: tags.max(0) as u64,
        linked_projects,
        managed_projects,
        missing_files: missing.len() as u64,
        total_bytes,
    };
    Ok(SourcePlan {
        stats,
        missing,
        unreadable_photos,
        assets,
        media,
        projects,
    })
}

async fn transform_snapshot(
    pool: &SqlitePool,
    plan: &SourcePlan,
    hashes: &HashMap<String, String>,
) -> MigrationResult<()> {
    let mut connection = pool.acquire().await.map_err(|error| error.to_string())?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    let result: MigrationResult<()> = async {
        for table in ["thumbnails", "viewer_meshes", "backup_runs", "backup_destinations", "ignored_asset_paths", "file_operation_jobs", "watched_import_folders", "user_overrides", "asset_origins"] {
            sqlx::query(&format!("DELETE FROM {table}")).execute(&mut *connection).await.map_err(|error| error.to_string())?;
        }
        sqlx::query("DELETE FROM folders").execute(&mut *connection).await.map_err(|error| error.to_string())?;
        sqlx::query("DELETE FROM library_roots").execute(&mut *connection).await.map_err(|error| error.to_string())?;
        let root_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES (?, '/library', 'Volum NAS', 'online', 'managed', ?, ?)")
            .bind(&root_id).bind(&now).bind(&now).execute(&mut *connection).await.map_err(|error| error.to_string())?;
        for project in &plan.projects {
            let name = project.folder.rsplit('/').next().unwrap_or("Project");
            sqlx::query("INSERT INTO folders (id, root_id, parent_id, relative_path, name, modified_at, status) VALUES (?, ?, NULL, ?, ?, ?, 'available')")
                .bind(&project.folder_id).bind(&root_id).bind(&project.folder).bind(name).bind(&now)
                .execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("UPDATE models SET folder_id = ?, storage_mode = 'managed', trash_original_relative_path = ? WHERE id = ?")
                .bind(&project.folder_id).bind((project.trashed).then(|| project.folder.replacen("Trash/", "Projects/", 1))).bind(&project.id)
                .execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("UPDATE model_search SET folder_path = ? WHERE model_id = ?")
                .bind(&project.folder).bind(&project.id).execute(&mut *connection).await
                .map_err(|error| error.to_string())?;
        }
        for asset in &plan.assets {
            let hash = hashes.get(&asset.id);
            sqlx::query("UPDATE assets SET root_id = ?, folder_id = ?, relative_path = ?, filename = ?, byte_size = ?, content_hash = ?, missing_since = ? WHERE id = ?")
                .bind(&root_id).bind(&asset.project_id).bind(&asset.destination).bind(&asset.filename).bind(asset.byte_size as i64).bind(hash)
                .bind(asset.missing.then(|| now.clone())).bind(&asset.id)
                .execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("INSERT INTO asset_origins (asset_id, source_kind, original_path, import_action, original_hash, imported_at) VALUES (?, 'desktop-migration', NULL, 'copy', ?, ?)")
                .bind(&asset.id).bind(hash).bind(&now).execute(&mut *connection).await.map_err(|error| error.to_string())?;
            sqlx::query("UPDATE web_sources SET root_id = ?, relative_path = ? WHERE root_id = ? AND relative_path = ?")
                .bind(&root_id).bind(&asset.destination).bind(&asset.source_root_id).bind(&asset.source_relative_path)
                .execute(&mut *connection).await.map_err(|error| error.to_string())?;
        }
        sqlx::query("UPDATE web_sources SET root_id = NULL, relative_path = NULL WHERE root_id IS NOT NULL AND root_id != ?")
            .bind(&root_id).execute(&mut *connection).await.map_err(|error| error.to_string())?;
        sqlx::query("DELETE FROM sync_changes").execute(&mut *connection).await.map_err(|error| error.to_string())?;
        Ok(())
    }.await;
    match result {
        Ok(()) => sqlx::query("COMMIT")
            .execute(&mut *connection)
            .await
            .map_err(|error| error.to_string())?,
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            return Err(error);
        }
    };
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    if !integrity.eq_ignore_ascii_case("ok") {
        return Err(format!(
            "The migration snapshot failed its integrity check: {integrity}"
        ));
    }
    if sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("The migration snapshot contains broken relationships".into());
    }
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&mut *connection)
        .await;
    Ok(())
}

async fn verify_server_files_and_database(
    data_dir: &Path,
    job: &ServerMigrationJob,
) -> MigrationResult<()> {
    for file in job.manifest.files.iter().filter(|file| !file.missing) {
        let path = server_staged_file(data_dir, job, file)?;
        let metadata = fs::metadata(&path)
            .map_err(|_| format!("{} was not completely uploaded", file.relative_path))?;
        if metadata.len() != file.byte_size {
            return Err(format!("{} has the wrong size", file.relative_path));
        }
        let expected = file
            .blake3
            .as_deref()
            .ok_or_else(|| "A migration checksum is missing".to_string())?;
        if hash_file(&path)? != expected {
            return Err(format!(
                "{} failed checksum verification",
                file.relative_path
            ));
        }
    }
    let database_file = job
        .manifest
        .files
        .iter()
        .find(|file| file.kind == "database")
        .ok_or_else(|| "The migration database is missing".to_string())?;
    let database = server_staged_file(data_dir, job, database_file)?;
    let options = SqliteConnectOptions::from_str(database.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("Could not open the migration database: {error}"))?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    if !integrity.eq_ignore_ascii_case("ok") {
        pool.close().await;
        return Err(format!(
            "The migration database failed its integrity check: {integrity}"
        ));
    }
    if sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&pool)
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        pool.close().await;
        return Err("The migration database contains broken relationships".into());
    }
    let projects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let assets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let photos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_photos")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let collections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM collections")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    if projects.max(0) as u64 != job.manifest.stats.projects
        || assets.max(0) as u64 != job.manifest.stats.files
        || photos.max(0) as u64 != job.manifest.stats.photos
        || collections.max(0) as u64 != job.manifest.stats.collections
        || tags.max(0) as u64 != job.manifest.stats.tags
    {
        pool.close().await;
        return Err("The migration database counts do not match its manifest".into());
    }
    let stage_library = server_library_stage(Path::new(&job.target_library_path), &job.manifest.id);
    ensure_server_library_layout(&stage_library)?;
    sqlx::query("UPDATE library_roots SET path = ?, display_name = ?, status = 'online' WHERE root_kind = 'managed'")
        .bind(stage_library.to_string_lossy().to_string()).bind(Path::new(&job.target_library_path).file_name().and_then(|name| name.to_str()).unwrap_or("Volum NAS"))
        .execute(&pool).await.map_err(|error| error.to_string())?;
    let project_ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM models WHERE storage_mode = 'managed'")
            .fetch_all(&pool)
            .await
            .map_err(|error| error.to_string())?;
    for project_id in project_ids {
        project_storage::write_project_marker(&project_id, &pool).await?;
    }
    write_managed_library_marker(&stage_library)?;
    sqlx::query("UPDATE library_roots SET path = ?, display_name = ?, status = 'online' WHERE root_kind = 'managed'")
        .bind(&job.target_library_path).bind(Path::new(&job.target_library_path).file_name().and_then(|name| name.to_str()).unwrap_or("Volum NAS"))
        .execute(&pool).await.map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM sync_changes")
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await;
    pool.close().await;
    remove_sqlite_sidecars(&database);
    Ok(())
}

fn validate_manifest(manifest: &MigrationManifest) -> MigrationResult<()> {
    if manifest.format_version != MIGRATION_FORMAT_VERSION {
        return Err("This Desktop uses an unsupported migration format".into());
    }
    validate_uuid(&manifest.id)?;
    if manifest.files.is_empty() || manifest.files.len() > MAX_MIGRATION_FILES {
        return Err("The migration contains an unsupported number of files".into());
    }
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut total = 0u64;
    let mut databases = 0usize;
    for file in &manifest.files {
        if file.id.is_empty() || file.id.len() > 200 || !ids.insert(file.id.clone()) {
            return Err("The migration contains duplicate or invalid file identities".into());
        }
        if !matches!(file.kind.as_str(), "asset" | "media" | "database") {
            return Err("The migration contains an unsupported file kind".into());
        }
        if !file
            .id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        {
            return Err("The migration contains an invalid file identity".into());
        }
        safe_relative_path(&file.relative_path)?;
        let relative = Path::new(&file.relative_path);
        if (file.kind == "database" && file.relative_path != "database/volum.sqlite3")
            || (file.kind == "media" && !relative.starts_with("media"))
            || (file.kind == "asset"
                && !relative.starts_with("Projects")
                && !relative.starts_with("Trash"))
        {
            return Err("The migration contains a file outside its allowed destination".into());
        }
        if !paths.insert(format!(
            "{}:{}",
            file.kind,
            file.relative_path.to_ascii_lowercase()
        )) {
            return Err("The migration contains duplicate destination paths".into());
        }
        if file.kind == "database" {
            databases += 1;
            if file.missing {
                return Err("The migration database cannot be missing".into());
            }
        }
        if file.missing && !manifest.allow_missing {
            return Err("The migration contains unapproved missing originals".into());
        }
        if !file.missing
            && file.blake3.as_deref().is_none_or(|hash| {
                hash.len() != 64 || !hash.chars().all(|character| character.is_ascii_hexdigit())
            })
        {
            return Err("The migration contains an invalid checksum".into());
        }
        total = total
            .checked_add(if file.missing { 0 } else { file.byte_size })
            .ok_or_else(|| "The migration is too large".to_string())?;
    }
    if databases != 1 || total > MAX_MIGRATION_BYTES {
        return Err("The migration manifest is incomplete or too large".into());
    }
    if total != manifest.stats.total_bytes {
        return Err("The migration byte count does not match its manifest".into());
    }
    Ok(())
}

fn server_job_status(
    job: &ServerMigrationJob,
    data_dir: &Path,
) -> MigrationResult<MigrationJobStatus> {
    let mut uploaded = 0u64;
    let total = job
        .manifest
        .files
        .iter()
        .filter(|file| !file.missing)
        .map(|file| file.byte_size)
        .sum();
    for file in job.manifest.files.iter().filter(|file| !file.missing) {
        let size = fs::metadata(server_staged_file(data_dir, job, file)?)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        uploaded = uploaded.saturating_add(size.min(file.byte_size));
    }
    Ok(MigrationJobStatus {
        id: job.manifest.id.clone(),
        state: job.state.clone(),
        stats: job.manifest.stats.clone(),
        uploaded_bytes: uploaded,
        total_bytes: total,
        completed_at: job.completed_at.clone(),
        error: job.error.clone(),
    })
}

fn manifest_file<'a>(
    job: &'a ServerMigrationJob,
    file_id: &str,
) -> MigrationResult<&'a MigrationManifestFile> {
    job.manifest
        .files
        .iter()
        .find(|file| file.id == file_id)
        .ok_or_else(|| "The migration file was not declared".into())
}

fn server_staged_file(
    data_dir: &Path,
    job: &ServerMigrationJob,
    file: &MigrationManifestFile,
) -> MigrationResult<PathBuf> {
    let relative = safe_relative_path(&file.relative_path)?;
    match file.kind.as_str() {
        "database" => {
            Ok(server_job_directory(data_dir, &job.manifest.id)
                .join("staging/database/volum.sqlite3"))
        }
        "media" => {
            let relative = relative
                .strip_prefix("media")
                .map_err(|_| "Project media must stay under the media directory".to_string())?;
            Ok(server_job_directory(data_dir, &job.manifest.id)
                .join("staging/media")
                .join(relative))
        }
        "asset" => Ok(
            server_library_stage(Path::new(&job.target_library_path), &job.manifest.id)
                .join(relative),
        ),
        _ => Err("Unsupported migration file kind".into()),
    }
}

fn server_job_directory(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join(SERVER_JOBS_DIRECTORY).join(id)
}
fn server_job_path(data_dir: &Path, id: &str) -> PathBuf {
    server_job_directory(data_dir, id).join("job.json")
}
fn server_library_stage(library: &Path, id: &str) -> PathBuf {
    library
        .join(SERVER_LIBRARY_STAGING)
        .join(id)
        .join("library")
}

fn reject_other_active_jobs(data_dir: &Path, requested_id: &str) -> MigrationResult<()> {
    let directory = data_dir.join(SERVER_JOBS_DIRECTORY);
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path().join("job.json");
        if !path.is_file() {
            continue;
        }
        let job: ServerMigrationJob = read_json(&path)?;
        if job.manifest.id != requested_id
            && !matches!(job.state.as_str(), "complete" | "failed" | "cancelled")
        {
            return Err("Another Desktop library migration is already active".into());
        }
    }
    Ok(())
}

fn ensure_server_library_layout(root: &Path) -> MigrationResult<()> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    for child in ["Projects", "Inbox", "Trash"] {
        fs::create_dir_all(root.join(child)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn write_managed_library_marker(root: &Path) -> MigrationResult<()> {
    fs::write(
        root.join(".volum-library.json"),
        b"{\n  \"formatVersion\": 1,\n  \"kind\": \"volum-managed-library\"\n}\n",
    )
    .map_err(|error| error.to_string())
}

fn activate_directory_child(
    stage: &Path,
    active: &Path,
    rollback: &Path,
    child: &str,
) -> MigrationResult<()> {
    let source = stage.join(child);
    let destination = active.join(child);
    let preserved = rollback.join(child);
    if source.exists() {
        if destination.exists() && !preserved.exists() {
            if let Some(parent) = preserved.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::rename(&destination, &preserved).map_err(|error| {
                format!("Could not preserve {}: {error}", destination.display())
            })?;
        }
        if !destination.exists() {
            fs::rename(&source, &destination).map_err(|error| {
                format!("Could not activate {}: {error}", destination.display())
            })?;
        }
    } else if !destination.exists() {
        return Err(format!("The staged migration item {child} is missing"));
    }
    Ok(())
}

async fn snapshot_schema_version(path: &Path) -> MigrationResult<i64> {
    let options = SqliteConnectOptions::from_str(path.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?
        .foreign_keys(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await
        .map_err(|error| error.to_string())
}

fn checked_source_path(root: &Path, relative: &str) -> MigrationResult<PathBuf> {
    let relative = safe_relative_path(relative)?;
    let joined = root.join(relative);
    if joined.exists() {
        let canonical_root = root
            .canonicalize()
            .map_err(|_| format!("Library root {} is unavailable", root.display()))?;
        let canonical = joined
            .canonicalize()
            .map_err(|_| format!("{} is unavailable", joined.display()))?;
        if !canonical.starts_with(canonical_root) {
            return Err("A project file escapes its library root".into());
        }
        Ok(canonical)
    } else {
        Ok(joined)
    }
}

fn checked_data_path(data_dir: &Path, relative: &str) -> MigrationResult<PathBuf> {
    let relative = safe_relative_path(relative)?;
    if !relative.starts_with("media") {
        return Err("Project media is outside Volum app data".into());
    }
    let joined = data_dir.join(relative);
    if joined.exists() {
        let canonical_data = data_dir
            .canonicalize()
            .map_err(|_| "Volum app data is unavailable".to_string())?;
        let canonical = joined
            .canonicalize()
            .map_err(|_| format!("{} is unavailable", joined.display()))?;
        if !canonical.starts_with(canonical_data) {
            return Err("Project media escapes Volum app data".into());
        }
        Ok(canonical)
    } else {
        Ok(joined)
    }
}

fn safe_relative_path(value: &str) -> MigrationResult<PathBuf> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("The migration contains an unsafe relative path".into());
    }
    Ok(path.to_path_buf())
}

fn safe_component(value: &str, max: usize) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['.', '-', ' '])
        .to_string();
    let mut result = cleaned.chars().take(max).collect::<String>();
    while result.ends_with(['.', ' ']) {
        result.pop();
    }
    if result.is_empty() {
        "Project".into()
    } else {
        result
    }
}

fn safe_filename(value: &str) -> String {
    let path = Path::new(value);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.stl");
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| safe_component(value, 140))
        .unwrap_or_else(|| "model".into());
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .take(16)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty());
    extension
        .map(|extension| format!("{stem}.{extension}"))
        .unwrap_or(stem)
}

fn disambiguated_filename(filename: &str, id: &str) -> String {
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    match path.extension().and_then(|value| value.to_str()) {
        Some(extension) => format!("{}-{}.{}", stem, short_id(id), extension),
        None => format!("{}-{}", stem, short_id(id)),
    }
}

fn short_id(id: &str) -> String {
    id.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
}

fn hash_file(path: &Path) -> MigrationResult<String> {
    let mut input = fs::File::open(path)
        .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn validate_uuid(value: &str) -> MigrationResult<()> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "The migration identity is invalid".into())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> MigrationResult<T> {
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> MigrationResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    OpenOptions::new()
        .write(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn remove_sqlite_sidecars(database: &Path) {
    let _ = fs::remove_file(PathBuf::from(format!("{}-wal", database.to_string_lossy())));
    let _ = fs::remove_file(PathBuf::from(format!("{}-shm", database.to_string_lossy())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_migration_paths() {
        assert!(safe_relative_path("Projects/Robot/arm.stl").is_ok());
        assert!(safe_relative_path("../escape.stl").is_err());
        assert!(safe_relative_path("/etc/passwd").is_err());
    }

    #[test]
    fn disambiguates_duplicate_filenames_without_losing_extension() {
        assert_eq!(
            disambiguated_filename("arm.stl", "12345678-abcd"),
            "arm-12345678.stl"
        );
        assert_eq!(safe_filename("braç final.stl"), "bra- final.stl");
    }

    #[tokio::test]
    async fn server_jobs_resume_verify_and_activate_atomically() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("config");
        let library = root.path().join("library");
        fs::create_dir_all(&library).unwrap();
        let state = AppState::initialize(&data).await.unwrap();
        project_storage::configure_managed_library_impl(
            library.to_string_lossy().to_string(),
            &state,
            None,
        )
        .await
        .unwrap();
        let source_database = root.path().join("source.sqlite3");
        sqlx::query("VACUUM INTO ?")
            .bind(source_database.to_string_lossy().to_string())
            .execute(&state.pool)
            .await
            .unwrap();
        let database_bytes = fs::read(&source_database).unwrap();
        let id = Uuid::new_v4().to_string();
        let schema_version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let stats = MigrationStats {
            projects: 0,
            files: 0,
            photos: 0,
            collections: 0,
            tags: 0,
            linked_projects: 0,
            managed_projects: 0,
            missing_files: 0,
            total_bytes: database_bytes.len() as u64,
        };
        let manifest = MigrationManifest {
            format_version: MIGRATION_FORMAT_VERSION,
            id: id.clone(),
            created_at: Utc::now().to_rfc3339(),
            schema_version,
            allow_missing: false,
            stats,
            files: vec![MigrationManifestFile {
                id: "database".into(),
                kind: "database".into(),
                relative_path: "database/volum.sqlite3".into(),
                byte_size: database_bytes.len() as u64,
                blake3: Some(blake3::hash(&database_bytes).to_hex().to_string()),
                missing: false,
            }],
        };
        let created = create_server_job(manifest.clone(), &state).await.unwrap();
        assert_eq!(created.state, "receiving");
        let split = database_bytes.len() / 2;
        let first =
            append_server_chunk(&data, &id, "database", 0, &database_bytes[..split]).unwrap();
        assert_eq!(first.uploaded_bytes, split as u64);
        let resumed = server_upload_status(&data, &id, "database").unwrap();
        assert_eq!(resumed.uploaded_bytes, split as u64);
        let job_path = server_job_path(&data, &id);
        let mut failed: ServerMigrationJob = read_json(&job_path).unwrap();
        failed.state = "failed".into();
        failed.error = Some("simulated interrupted verification".into());
        write_json_atomic(&job_path, &failed).unwrap();
        let reset = create_server_job(manifest.clone(), &state).await.unwrap();
        assert_eq!(reset.state, "receiving");
        assert_eq!(reset.uploaded_bytes, 0);
        append_server_chunk(&data, &id, "database", 0, &database_bytes).unwrap();
        let verified = verify_server_job(&data, &id).await.unwrap();
        assert_eq!(verified.state, "ready");
        let activating = prepare_server_activation(&data, &id).unwrap();
        assert_eq!(activating.state, "activating");
        state.pool.close().await;
        apply_pending_activation(&data, &library).unwrap();
        let restored = AppState::initialize(&data).await.unwrap();
        let managed_path: String =
            sqlx::query_scalar("SELECT path FROM library_roots WHERE root_kind = 'managed'")
                .fetch_one(&restored.pool)
                .await
                .unwrap();
        assert_eq!(
            PathBuf::from(managed_path).canonicalize().unwrap(),
            library.canonicalize().unwrap()
        );
        assert_eq!(get_server_job(&data, &id).unwrap().state, "complete");
        assert!(data
            .join("migration-rollbacks")
            .join(&id)
            .join("volum.sqlite3")
            .is_file());
        assert!(cancel_server_job(&data, &id).is_err());

        let cancelled_id = Uuid::new_v4().to_string();
        let mut cancelled_manifest = manifest;
        cancelled_manifest.id = cancelled_id.clone();
        let cancelled_created = create_server_job(cancelled_manifest, &restored)
            .await
            .unwrap();
        assert_eq!(cancelled_created.state, "receiving");
        append_server_chunk(
            &data,
            &cancelled_id,
            "database",
            0,
            &database_bytes[..split],
        )
        .unwrap();
        let cancelled = cancel_server_job(&data, &cancelled_id).unwrap();
        assert_eq!(cancelled.state, "cancelled");
        assert_eq!(cancelled.uploaded_bytes, 0);
        restored.pool.close().await;
    }

    #[tokio::test]
    async fn snapshot_transform_keeps_metadata_and_converts_linked_files_to_managed() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("config");
        let linked = root.path().join("linked");
        fs::create_dir_all(linked.join("Robot")).unwrap();
        fs::write(linked.join("Robot/arm.stl"), b"solid arm\nendsolid arm\n").unwrap();
        fs::create_dir_all(data.join("media/photos")).unwrap();
        fs::create_dir_all(data.join("media/photo-thumbnails")).unwrap();
        fs::write(data.join("media/photos/photo.jpg"), b"photo").unwrap();
        fs::write(data.join("media/photo-thumbnails/photo.jpg"), b"thumb").unwrap();
        let state = AppState::initialize(&data).await.unwrap();
        let now = Utc::now().to_rfc3339();
        let project_id = Uuid::new_v4().to_string();
        let asset_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, root_kind, created_at, updated_at) VALUES ('linked-root', ?, 'Linked', 'online', 'linked', ?, ?)")
            .bind(linked.to_string_lossy().to_string()).bind(&now).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO folders (id, root_id, relative_path, name, status) VALUES ('folder', 'linked-root', 'Robot', 'Robot', 'available')")
            .execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO models (id, folder_id, display_name, notes, favorite, grouping_key, added_at, updated_at, storage_mode) VALUES (?, 'folder', 'Robot arm', 'Assembly notes', 1, 'robot', ?, ?, 'linked')")
            .bind(&project_id).bind(&now).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at, parse_status, metadata_json) VALUES (?, 'linked-root', 'folder', 'Robot/arm.stl', 'arm.stl', 'stl', 23, ?, 'parsed', '{}')")
            .bind(&asset_id).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO model_assets (model_id, asset_id, role, sort_order) VALUES (?, ?, 'printable', 0)")
            .bind(&project_id).bind(&asset_id).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO web_sources (id, provider, canonical_url, title, status, root_id, relative_path, fetched_at, created_at, updated_at) VALUES ('source', 'makerworld', 'https://makerworld.com/en/models/123', 'Robot arm', 'imported', 'linked-root', 'Robot/arm.stl', ?, ?, ?)")
            .bind(&now).bind(&now).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO tags (id, name, color, created_at, updated_at) VALUES ('tag', 'Workshop', '#ff5a36', ?, ?)")
            .bind(&now).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO model_tags (model_id, tag_id, added_at) VALUES (?, 'tag', ?)")
            .bind(&project_id)
            .bind(&now)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO collections (id, name, symbol, color, created_at, updated_at) VALUES ('collection', 'Robots', 'box', '#ff5a36', ?, ?)")
            .bind(&now).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO collection_items (collection_id, model_id, position, added_at) VALUES ('collection', ?, 0, ?)")
            .bind(&project_id).bind(&now).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO project_photos (id, model_id, media_path, thumbnail_path, original_filename, mime_type, byte_size, width, height, created_at, updated_at) VALUES ('photo', ?, 'media/photos/photo.jpg', 'media/photo-thumbnails/photo.jpg', 'photo.jpg', 'image/jpeg', 5, 10, 10, ?, ?)")
            .bind(&project_id).bind(&now).bind(&now).execute(&state.pool).await.unwrap();

        let snapshot = root.path().join("snapshot.sqlite3");
        sqlx::query("VACUUM INTO ?")
            .bind(snapshot.to_string_lossy().to_string())
            .execute(&state.pool)
            .await
            .unwrap();
        let options = SqliteConnectOptions::from_str(snapshot.to_string_lossy().as_ref())
            .unwrap()
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let plan = build_source_plan(&pool, &data).await.unwrap();
        assert_eq!(plan.stats.projects, 1);
        assert_eq!(plan.stats.linked_projects, 1);
        assert_eq!(plan.stats.photos, 1);
        assert!(plan.missing.is_empty());
        let hash = hash_file(&linked.join("Robot/arm.stl")).unwrap();
        transform_snapshot(
            &pool,
            &plan,
            &HashMap::from([(asset_id.clone(), hash.clone())]),
        )
        .await
        .unwrap();
        let root_kind: String = sqlx::query_scalar("SELECT root_kind FROM library_roots")
            .fetch_one(&pool)
            .await
            .unwrap();
        let storage_mode: String =
            sqlx::query_scalar("SELECT storage_mode FROM models WHERE id = ?")
                .bind(&project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let notes: String = sqlx::query_scalar("SELECT notes FROM models WHERE id = ?")
            .bind(&project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let relative: String = sqlx::query_scalar("SELECT relative_path FROM assets WHERE id = ?")
            .bind(&asset_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let content_hash: String =
            sqlx::query_scalar("SELECT content_hash FROM assets WHERE id = ?")
                .bind(&asset_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let web_source_path: String =
            sqlx::query_scalar("SELECT relative_path FROM web_sources WHERE id = 'source'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_tags WHERE model_id = ?")
            .bind(&project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let collections: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM collection_items WHERE model_id = ?")
                .bind(&project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(root_kind, "managed");
        assert_eq!(storage_mode, "managed");
        assert_eq!(notes, "Assembly notes");
        assert!(relative.starts_with("Projects/Robot arm-"));
        assert_eq!(web_source_path, relative);
        assert_eq!(content_hash, hash);
        assert_eq!(tags, 1);
        assert_eq!(collections, 1);
        pool.close().await;
        state.pool.close().await;
    }
}
