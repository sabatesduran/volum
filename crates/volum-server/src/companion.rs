use crate::{
    commands,
    domain::{
        Asset, AssetMetadata, Collection, CollectionInput, CostEstimate, CostEstimateInput,
        Material, MaterialInput, ModelDetail, ModelQuery, ModelSummary, Page, ProjectDocumentation,
        RelatedModel, SavedSearch, SavedSearchInput, Tag, TagInput, WebSource,
    },
    events::EventEmitter,
    indexing, library_migration, project_docs, project_storage,
    state::AppState,
    web_admin::WebAdminState,
};
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::HashSet,
    convert::Infallible,
    io::{Read, SeekFrom},
    net::{IpAddr, SocketAddr},
    path::{Path as FilePath, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{Mutex, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

const DEFAULT_PORT: u16 = 43_821;
const PAIRING_LIFETIME_SECONDS: i64 = 300;
const PROTOCOL_VERSION: u32 = 1;
const MAX_PAIRING_BODY_BYTES: usize = 16 * 1024;
const MAX_SEARCH_CHARACTERS: usize = 256;
const MAX_CURSOR_BYTES: usize = 4 * 1024;
const MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;
const MAX_PHOTO_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;
const MAX_UPLOAD_FILENAME_CHARS: usize = 180;
const MAX_MUTATION_BODY_BYTES: usize = 128 * 1024;
const MAX_SYNC_SELECTION_ITEMS: usize = 10_000;
const MAX_SYNC_SELECTION_SCOPES: usize = 256;
const MAX_SYNC_SELECTION_ID_CHARS: usize = 200;
const MAX_VIEWER_MESH_BYTES: usize = 30 * 1024 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 128;
const REQUEST_TIMEOUT_SECONDS: u64 = 15 * 60;
const PAIRING_REQUEST_TIMEOUT_SECONDS: u64 = 15;
const SUPPORTED_UPLOAD_EXTENSIONS: &[&str] = &["stl", "3mf", "obj", "step", "stp"];
const SUPPORTED_PHOTO_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

type CompanionResult<T> = Result<T, String>;

#[derive(Clone)]
pub struct CompanionController {
    library: AppState,
    app: Option<EventEmitter>,
    inner: Arc<Mutex<ControllerInner>>,
    start_lock: Arc<Mutex<()>>,
}

struct ControllerInner {
    config: CompanionConfig,
    config_path: PathBuf,
    pairing: Option<PairingSession>,
    server: Option<RunningServer>,
    tailnet_ip: Option<String>,
    dns_name: Option<String>,
    base_url: Option<String>,
    last_error: Option<String>,
    headless: bool,
}

struct PairingSession {
    secret: String,
    expires_at: DateTime<Utc>,
    role: String,
}

struct RunningServer {
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanionConfig {
    #[serde(default)]
    enabled: bool,
    server_id: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    devices: Vec<DeviceRecord>,
    #[serde(default)]
    organization_writes: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    photo_uploads: Option<bool>,
    #[serde(default = "default_network_mode")]
    network_mode: String,
    #[serde(default)]
    bind_address: Option<String>,
    #[serde(default)]
    public_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceRecord {
    id: String,
    name: String,
    platform: String,
    token_hash: String,
    paired_at: String,
    last_seen_at: Option<String>,
    #[serde(default = "default_device_role")]
    role: String,
    #[serde(default)]
    migration_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionStatus {
    pub enabled: bool,
    pub running: bool,
    pub server_id: String,
    pub tailnet_ip: Option<String>,
    pub dns_name: Option<String>,
    pub base_url: Option<String>,
    pub port: u16,
    pub devices: Vec<CompanionDevice>,
    pub organization_writes: bool,
    pub photo_uploads: bool,
    pub network_mode: String,
    pub bind_address: Option<String>,
    pub public_url: Option<String>,
    pub transport: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionPairingInfo {
    pub server_id: String,
    pub server_name: String,
    pub base_url: String,
    pub secret: String,
    pub expires_at: String,
    pub transport: String,
    pub qr_payload: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HeadlessServerOptions {
    pub bind: SocketAddr,
    pub public_url: String,
    pub organization_writes: bool,
    pub photo_uploads: bool,
    pub web_admin: Option<WebAdminState>,
    pub web_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct ApiState {
    library: AppState,
    app: Option<EventEmitter>,
    controller: Arc<Mutex<ControllerInner>>,
    upload_slot: Arc<Semaphore>,
    viewer_slot: Arc<Semaphore>,
    migration_lock: Arc<Mutex<()>>,
    web_admin: Option<WebAdminState>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerIdentity {
    server_id: String,
    name: String,
    protocol_min: u32,
    protocol_max: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerStatus {
    server_id: String,
    name: String,
    protocol_version: u32,
    project_count: i64,
    library_revision: String,
    capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerCapabilities {
    read_library: bool,
    organization_writes: bool,
    photo_uploads: bool,
    file_uploads: bool,
    viewer_meshes: bool,
    plate_thumbnails: bool,
    cost_estimates: bool,
    related_projects: bool,
    live_events: bool,
    original_files: bool,
    incremental_sync: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairRequest {
    secret: String,
    device_name: String,
    platform: String,
}

#[derive(Debug, Deserialize)]
struct MigrationOffsetQuery {
    offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    server_id: String,
    device_id: String,
    token: String,
    protocol_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebLoginRequest {
    password: String,
    device_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebBootstrapResponse {
    configured: bool,
    server_id: String,
    server_name: String,
    public_url: String,
    secure_transport: bool,
    protocol_version: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLoginResponse {
    server_id: String,
    server_name: String,
    device_id: String,
    token: String,
    protocol_version: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionProjectSummary {
    id: String,
    display_name: String,
    folder_id: String,
    folder_name: String,
    primary_asset_id: Option<String>,
    primary_extension: String,
    favorite: bool,
    added_at: String,
    modified_at: String,
    last_opened_at: Option<String>,
    missing: bool,
    asset_count: i64,
    cover_photo_id: Option<String>,
    dimensions_mm: Option<[f64; 3]>,
    relative_folder: String,
    bundle_mode: String,
    storage_mode: String,
}

impl From<ModelSummary> for CompanionProjectSummary {
    fn from(value: ModelSummary) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            folder_id: value.folder_id,
            folder_name: value.folder_name,
            primary_asset_id: value.primary_asset_id,
            primary_extension: value.primary_extension,
            favorite: value.favorite,
            added_at: value.added_at,
            modified_at: value.modified_at,
            last_opened_at: value.last_opened_at,
            missing: value.missing,
            asset_count: value.asset_count,
            cover_photo_id: value.cover_photo_id,
            dimensions_mm: value.dimensions_mm,
            relative_folder: value.relative_folder,
            bundle_mode: value.bundle_mode,
            storage_mode: value.storage_mode,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionPage<T> {
    items: Vec<T>,
    total: i64,
    next_cursor: Option<String>,
}

impl From<Page<ModelSummary>> for CompanionPage<CompanionProjectSummary> {
    fn from(value: Page<ModelSummary>) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            next_cursor: value.next_cursor,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionAsset {
    id: String,
    filename: String,
    extension: String,
    byte_size: i64,
    modified_at: String,
    parse_status: String,
    metadata: AssetMetadata,
    missing: bool,
    role: String,
}

impl From<Asset> for CompanionAsset {
    fn from(value: Asset) -> Self {
        Self {
            id: value.id,
            filename: value.filename,
            extension: value.extension,
            byte_size: value.byte_size,
            modified_at: value.modified_at,
            parse_status: value.parse_status,
            metadata: value.metadata,
            missing: value.missing,
            role: value.role,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionWebSource {
    provider: String,
    canonical_url: String,
    title: String,
    creator: Option<String>,
    description: Option<String>,
    license: Option<String>,
    image_url: Option<String>,
    filament_grams: Option<f64>,
}

impl From<WebSource> for CompanionWebSource {
    fn from(value: WebSource) -> Self {
        Self {
            provider: value.preview.provider,
            canonical_url: value.preview.canonical_url,
            title: value.preview.title,
            creator: value.preview.creator,
            description: value.preview.description,
            license: value.preview.license,
            image_url: value.preview.image_url,
            filament_grams: value.preview.filament_grams,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionProjectDetail {
    #[serde(flatten)]
    summary: CompanionProjectSummary,
    notes: String,
    root_name: String,
    root_id: String,
    assets: Vec<CompanionAsset>,
    collection_ids: Vec<String>,
    tags: Vec<Tag>,
    estimate: Option<CostEstimate>,
    web_source: Option<CompanionWebSource>,
    revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionDocumentation {
    #[serde(flatten)]
    documentation: ProjectDocumentation,
    revision: String,
}

impl From<ModelDetail> for CompanionProjectDetail {
    fn from(value: ModelDetail) -> Self {
        Self {
            summary: value.summary.into(),
            notes: value.notes,
            root_name: value.root_name,
            root_id: value.root_id,
            assets: value.assets.into_iter().map(Into::into).collect(),
            collection_ids: value.collection_ids,
            tags: value.tags,
            estimate: value.estimate,
            web_source: value.web_source.map(Into::into),
            revision: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionFolder {
    id: String,
    root_id: String,
    parent_id: Option<String>,
    relative_path: String,
    name: String,
    model_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionRoot {
    id: String,
    display_name: String,
    status: String,
    last_scan_at: Option<String>,
    model_count: i64,
    root_kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncManifest {
    server_id: String,
    cursor: i64,
    assets: Vec<SyncAsset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncAsset {
    id: String,
    project_id: String,
    project_name: String,
    filename: String,
    byte_size: i64,
    modified_at: String,
    content_hash: Option<String>,
    mirror_path: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectiveSyncManifestRequest {
    #[serde(default)]
    project_ids: Vec<String>,
    #[serde(default)]
    excluded_project_ids: Vec<String>,
    #[serde(default)]
    folder_ids: Vec<String>,
    #[serde(default)]
    collection_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangesQuery {
    #[serde(default)]
    after: i64,
    limit: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderQuery {
    root_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DuplicateQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncChange {
    sequence: i64,
    entity_type: String,
    entity_id: String,
    change_kind: String,
    revision: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncChanges {
    items: Vec<SyncChange>,
    cursor: i64,
    has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetContentUpdate {
    asset_id: String,
    byte_size: u64,
    content_hash: String,
    modified_at: String,
}

#[derive(Debug, Deserialize)]
struct PhotoQuery {
    #[serde(default = "default_true")]
    thumbnail: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadQuery {
    filename: String,
    project_id: Option<String>,
    project_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResponse {
    project_id: String,
    asset_id: String,
    filename: String,
    byte_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhotoUploadQuery {
    filename: String,
    #[serde(default)]
    caption: String,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoUpdateRequest {
    caption: String,
    sort_order: f64,
    is_cover: bool,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CostEstimateRequest {
    asset_id: Option<String>,
    material_id: Option<String>,
    plastic_g: f64,
    quantity: i64,
    source: String,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectUpdateRequest {
    display_name: Option<String>,
    favorite: Option<bool>,
    notes: Option<String>,
    tag_ids: Option<Vec<String>>,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateProjectRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTagRequest {
    name: String,
    color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CustomValueRequest {
    value: Option<serde_json::Value>,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BomItemRequest {
    id: Option<String>,
    kind: String,
    linked_asset_id: Option<String>,
    name: String,
    quantity: f64,
    unit: String,
    url: Option<String>,
    notes: String,
    sort_order: Option<f64>,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssemblyStepRequest {
    id: Option<String>,
    title: String,
    instructions: String,
    sort_order: Option<f64>,
    #[serde(default)]
    bom_item_ids: Vec<String>,
    #[serde(default)]
    photo_ids: Vec<String>,
    if_unmodified_since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RevisionRequest {
    if_unmodified_since: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViewerMeshQuery {
    plate_index: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: &'static str,
    message: String,
    retryable: bool,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retryable: bool,
}

impl ApiError {
    fn unauthorized(message: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
            retryable: false,
        }
    }

    fn from_library(error: String) -> Self {
        if error.contains("not found") || error.contains("not available") {
            Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: "The requested Volum item is unavailable.".into(),
                retryable: false,
            }
        } else if error.contains("Invalid") {
            Self {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_request",
                message: error,
                retryable: false,
            }
        } else {
            Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "request_failed",
                message: "Volum could not complete this request.".into(),
                retryable: true,
            }
        }
    }

    fn invalid_upload(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_upload",
            message: message.into(),
            retryable: false,
        }
    }

    fn from_upload_import(error: String) -> Self {
        if error.contains("Project not found") {
            Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: "The selected project is unavailable.".into(),
                retryable: false,
            }
        } else if error.contains("managed library") {
            Self {
                status: StatusCode::CONFLICT,
                code: "managed_library_required",
                message: "Configure a managed library on Volum Desktop before uploading files."
                    .into(),
                retryable: false,
            }
        } else if error.contains("managed project") || error.contains("Trash") {
            Self {
                status: StatusCode::CONFLICT,
                code: "upload_target_unavailable",
                message: error,
                retryable: false,
            }
        } else {
            log::warn!("Companion upload import failed: {error}");
            Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "upload_failed",
                message: "Volum Desktop could not import the uploaded file.".into(),
                retryable: true,
            }
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "edit_conflict",
            message: message.into(),
            retryable: true,
        }
    }

    fn invalid_mutation(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_change",
            message: message.into(),
            retryable: false,
        }
    }

    fn writes_disabled() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "organization_writes_disabled",
            message: "Enable mobile organization changes in Volum Desktop settings first.".into(),
            retryable: false,
        }
    }

    fn photo_uploads_disabled() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "photo_uploads_disabled",
            message: "Enable mobile photo uploads in the canonical Volum host settings first."
                .into(),
            retryable: false,
        }
    }

    fn migration(error: String) -> Self {
        let (status, code, retryable) = if error.contains("already contains")
            || error.contains("already active")
            || error.contains("current state")
            || error.contains("before activating")
        {
            (StatusCode::CONFLICT, "migration_conflict", false)
        } else if error.contains("unavailable") || error.contains("missing") {
            (StatusCode::NOT_FOUND, "migration_item_missing", true)
        } else if error.contains("invalid")
            || error.contains("unsupported")
            || error.contains("unsafe")
            || error.contains("exceeds")
            || error.contains("duplicate")
        {
            (StatusCode::BAD_REQUEST, "invalid_migration", false)
        } else {
            (StatusCode::UNPROCESSABLE_ENTITY, "migration_failed", true)
        };
        Self {
            status,
            code,
            message: error,
            retryable,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ApiErrorBody {
                    code: self.code,
                    message: self.message,
                    retryable: self.retryable,
                },
            }),
        )
            .into_response()
    }
}

impl CompanionController {
    pub fn new(
        data_dir: &FilePath,
        library: AppState,
        app: Option<EventEmitter>,
    ) -> CompanionResult<Self> {
        let config_path = data_dir.join("companion.json");
        let config = load_config(&config_path)?;
        let upload_staging = data_dir.join("companion-uploads");
        if upload_staging.exists() {
            if let Err(error) = std::fs::remove_dir_all(&upload_staging) {
                log::warn!("Could not clean stale Companion uploads: {error}");
            }
        }
        Ok(Self {
            library,
            app,
            inner: Arc::new(Mutex::new(ControllerInner {
                config,
                config_path,
                pairing: None,
                server: None,
                tailnet_ip: None,
                dns_name: None,
                base_url: None,
                last_error: None,
                headless: false,
            })),
            start_lock: Arc::new(Mutex::new(())),
        })
    }

    pub async fn start_if_enabled(&self) {
        let enabled = self.inner.lock().await.config.enabled;
        if enabled {
            let _ = self.start().await;
        }
    }

    async fn start(&self) -> CompanionResult<()> {
        let _start_guard = self.start_lock.lock().await;
        let stale_server = {
            let mut inner = self.inner.lock().await;
            match inner.server.as_ref() {
                Some(server) if !server.task.is_finished() => return Ok(()),
                Some(_) => inner.server.take(),
                None => None,
            }
        };
        if let Some(server) = stale_server {
            let _ = server.task.await;
        }
        let config = self.inner.lock().await.config.clone();
        let (ip, dns_name, public_url) = if config.network_mode == "custom" {
            let bind_address = config
                .bind_address
                .as_deref()
                .ok_or_else(|| "Choose a private address for Companion Access".to_string())?;
            let ip = parse_private_bind_address(bind_address)?;
            let public_url = validate_public_url(
                config
                    .public_url
                    .as_deref()
                    .ok_or_else(|| "Enter the address clients will use".to_string())?,
            )
            .await?;
            (ip, None, Some(public_url))
        } else {
            let network = match tokio::task::spawn_blocking(discover_tailnet).await {
                Ok(Ok(network)) => network,
                Ok(Err(error)) => return self.fail_start(error).await,
                Err(error) => return self.fail_start(error.to_string()).await,
            };
            let ip: IpAddr = network
                .ip
                .parse()
                .map_err(|_| "Tailscale returned an invalid address".to_string())?;
            (ip, network.dns_name, None)
        };
        let preferred_port = config.port;
        let mut listener = None;
        let mut selected_port = preferred_port;
        let last_port = if config.network_mode == "custom" {
            preferred_port
        } else {
            preferred_port.saturating_add(9)
        };
        for port in preferred_port..=last_port {
            match tokio::net::TcpListener::bind(SocketAddr::new(ip, port)).await {
                Ok(bound) => {
                    selected_port = port;
                    listener = Some(bound);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => continue,
                Err(error) => {
                    return self
                        .fail_start(format!("Could not bind Companion Access: {error}"))
                        .await
                }
            }
        }
        let listener = match listener {
            Some(listener) => listener,
            None => {
                return self
                    .fail_start("No companion port is available".into())
                    .await
            }
        };
        let base_url = public_url.unwrap_or_else(|| format_base_url(ip, selected_port));
        let state = ApiState {
            library: self.library.clone(),
            app: self.app.clone(),
            controller: self.inner.clone(),
            upload_slot: Arc::new(Semaphore::new(1)),
            viewer_slot: Arc::new(Semaphore::new(1)),
            migration_lock: Arc::new(Mutex::new(())),
            web_admin: None,
        };
        let router = companion_router(state, None);
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        {
            let mut inner = self.inner.lock().await;
            if !inner.config.enabled {
                return Ok(());
            }
            inner.config.port = selected_port;
            persist_config(&inner.config_path, &inner.config)?;
            inner.tailnet_ip = (inner.config.network_mode == "automatic").then(|| ip.to_string());
            inner.dns_name = dns_name;
            inner.base_url = Some(base_url);
            inner.last_error = None;
            inner.headless = false;
        }
        let controller = self.inner.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
            {
                log::error!("Companion server stopped: {error}");
                controller.lock().await.last_error =
                    Some("Companion Access stopped unexpectedly".into());
            }
        });
        let mut inner = self.inner.lock().await;
        inner.server = Some(RunningServer { cancellation, task });
        Ok(())
    }

    pub(crate) async fn start_headless(
        &self,
        options: HeadlessServerOptions,
    ) -> CompanionResult<()> {
        let _start_guard = self.start_lock.lock().await;
        let stale_server = {
            let mut inner = self.inner.lock().await;
            match inner.server.as_ref() {
                Some(server) if !server.task.is_finished() => return Ok(()),
                Some(_) => inner.server.take(),
                None => None,
            }
        };
        if let Some(server) = stale_server {
            let _ = server.task.await;
        }
        let listener = tokio::net::TcpListener::bind(options.bind)
            .await
            .map_err(|error| format!("Could not bind volumd to {}: {error}", options.bind))?;
        let local_addr = listener
            .local_addr()
            .map_err(|error| format!("Could not inspect the volumd listener: {error}"))?;
        let base_url = validate_public_url(&options.public_url).await?;
        {
            let mut inner = self.inner.lock().await;
            inner.config.enabled = true;
            inner.config.port = local_addr.port();
            inner.config.organization_writes = options.organization_writes;
            inner.config.photo_uploads = Some(options.photo_uploads);
            persist_config(&inner.config_path, &inner.config)?;
            inner.tailnet_ip = match local_addr.ip() {
                IpAddr::V4(ip) if ip.is_unspecified() => None,
                IpAddr::V6(ip) if ip.is_unspecified() => None,
                ip => Some(ip.to_string()),
            };
            inner.dns_name = None;
            inner.base_url = Some(base_url);
            inner.last_error = None;
            inner.headless = true;
        }
        let state = ApiState {
            library: self.library.clone(),
            app: self.app.clone(),
            controller: self.inner.clone(),
            upload_slot: Arc::new(Semaphore::new(1)),
            viewer_slot: Arc::new(Semaphore::new(1)),
            migration_lock: Arc::new(Mutex::new(())),
            web_admin: options.web_admin,
        };
        let router = companion_router(state, options.web_dir);
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let controller = self.inner.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            {
                log::error!("volumd stopped: {error}");
                controller.lock().await.last_error = Some("volumd stopped unexpectedly".into());
            }
        });
        self.inner.lock().await.server = Some(RunningServer { cancellation, task });
        Ok(())
    }

    async fn fail_start<T>(&self, error: String) -> CompanionResult<T> {
        self.inner.lock().await.last_error = Some(error.clone());
        Err(error)
    }

    pub(crate) async fn stop(&self) {
        let _start_guard = self.start_lock.lock().await;
        let running = {
            let mut inner = self.inner.lock().await;
            inner.pairing = None;
            inner.base_url = None;
            inner.tailnet_ip = None;
            inner.dns_name = None;
            inner.server.take()
        };
        if let Some(running) = running {
            running.cancellation.cancel();
            let mut task = running.task;
            if tokio::time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
            {
                log::warn!("Companion server did not stop before the shutdown timeout");
                task.abort();
            }
        }
    }

    pub async fn set_enabled(&self, enabled: bool) -> CompanionResult<CompanionStatus> {
        {
            let mut inner = self.inner.lock().await;
            inner.config.enabled = enabled;
            inner.last_error = None;
            persist_config(&inner.config_path, &inner.config)?;
        }
        if enabled {
            let _ = self.start().await;
        } else {
            self.stop().await;
        }
        Ok(self.status().await)
    }

    pub async fn status(&self) -> CompanionStatus {
        let inner = self.inner.lock().await;
        CompanionStatus {
            enabled: inner.config.enabled,
            running: inner
                .server
                .as_ref()
                .is_some_and(|server| !server.task.is_finished()),
            server_id: inner.config.server_id.clone(),
            tailnet_ip: inner.tailnet_ip.clone(),
            dns_name: inner.dns_name.clone(),
            base_url: inner.base_url.clone(),
            port: inner.config.port,
            devices: inner.config.devices.iter().map(public_device).collect(),
            organization_writes: inner.config.organization_writes,
            photo_uploads: inner.config.photo_uploads.unwrap_or(false),
            network_mode: inner.config.network_mode.clone(),
            bind_address: inner.config.bind_address.clone(),
            public_url: inner.config.public_url.clone(),
            transport: inner
                .base_url
                .as_deref()
                .map(|value| transport_for_url(value).to_string()),
            last_error: inner.last_error.clone(),
        }
    }

    pub async fn set_organization_writes(&self, enabled: bool) -> CompanionResult<CompanionStatus> {
        let mut inner = self.inner.lock().await;
        inner.config.organization_writes = enabled;
        persist_config(&inner.config_path, &inner.config)?;
        drop(inner);
        Ok(self.status().await)
    }

    pub async fn set_photo_uploads(&self, enabled: bool) -> CompanionResult<CompanionStatus> {
        let mut inner = self.inner.lock().await;
        inner.config.photo_uploads = Some(enabled);
        persist_config(&inner.config_path, &inner.config)?;
        drop(inner);
        Ok(self.status().await)
    }

    pub async fn set_network(
        &self,
        mode: &str,
        bind_address: Option<String>,
        public_url: Option<String>,
    ) -> CompanionResult<CompanionStatus> {
        if !matches!(mode, "automatic" | "custom") {
            return Err("The Companion Access network mode is invalid".into());
        }
        let (bind_address, public_url) = if mode == "custom" {
            let bind = bind_address
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Choose a private address for Companion Access".to_string())?;
            parse_private_bind_address(bind)?;
            let public = public_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Enter the address clients will use".to_string())?;
            let normalized = validate_public_url(public).await?;
            (Some(bind.to_string()), Some(normalized))
        } else {
            (None, None)
        };
        let enabled = {
            let mut inner = self.inner.lock().await;
            inner.config.network_mode = mode.to_string();
            inner.config.bind_address = bind_address;
            inner.config.public_url = public_url;
            inner.last_error = None;
            persist_config(&inner.config_path, &inner.config)?;
            inner.config.enabled
        };
        if enabled {
            self.stop().await;
            let _ = self.start().await;
        }
        Ok(self.status().await)
    }

    pub async fn create_pairing(&self) -> CompanionResult<CompanionPairingInfo> {
        let mut inner = self.inner.lock().await;
        create_pairing_locked(&mut inner, "client")
    }

    pub(crate) async fn revoke_web_admin_devices(&self) -> CompanionResult<()> {
        let mut inner = self.inner.lock().await;
        let previous = inner.config.devices.len();
        inner.config.devices.retain(|device| device.role != "admin");
        if previous != inner.config.devices.len() {
            persist_config(&inner.config_path, &inner.config)?;
        }
        Ok(())
    }

    pub async fn revoke_device(&self, device_id: &str) -> CompanionResult<CompanionStatus> {
        let mut inner = self.inner.lock().await;
        let previous = inner.config.devices.len();
        inner.config.devices.retain(|device| device.id != device_id);
        if previous == inner.config.devices.len() {
            return Err("Companion device not found".into());
        }
        persist_config(&inner.config_path, &inner.config)?;
        drop(inner);
        Ok(self.status().await)
    }
}

fn companion_router(state: ApiState, web_dir: Option<PathBuf>) -> Router {
    let protected = Router::new()
        .route("/api/v1/status", get(api_status))
        .route(
            "/api/v1/projects",
            get(list_projects)
                .post(create_project)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}",
            get(get_project)
                .patch(update_project)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/trash",
            put(trash_project).delete(delete_project_permanently),
        )
        .route("/api/v1/projects/{id}/restore", post(restore_project))
        .route(
            "/api/v1/projects/{id}/documentation",
            get(get_documentation),
        )
        .route(
            "/api/v1/projects/{id}/photos",
            post(upload_project_photo)
                .layer(DefaultBodyLimit::max(MAX_PHOTO_UPLOAD_BYTES as usize)),
        )
        .route(
            "/api/v1/projects/{id}/photos/{photo_id}",
            axum::routing::patch(update_project_photo)
                .delete(delete_project_photo)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/estimate",
            put(save_cost_estimate).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route("/api/v1/projects/{id}/related", get(list_related_projects))
        .route(
            "/api/v1/projects/{id}/custom-fields/{field_id}",
            put(set_custom_value).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/bom",
            post(save_bom_item).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/bom/{item_id}",
            delete(delete_bom_item).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/assembly-steps",
            post(save_assembly_step).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/projects/{id}/assembly-steps/{step_id}",
            delete(delete_assembly_step).layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route("/api/v1/projects/{id}/cover", get(get_project_cover))
        .route("/api/v1/assets/{id}/thumbnail", get(get_asset_thumbnail))
        .route(
            "/api/v1/assets/{id}/plates/{plate_index}/thumbnail",
            get(get_plate_thumbnail),
        )
        .route("/api/v1/assets/{id}/viewer-mesh", get(get_viewer_mesh))
        .route(
            "/api/v1/assets/{id}/content",
            get(get_asset_content)
                .head(head_asset_content)
                .put(put_asset_content)
                .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES as usize)),
        )
        .route("/api/v1/photos/{id}", get(get_photo))
        .route("/api/v1/roots", get(list_roots))
        .route("/api/v1/folders", get(list_folders))
        .route("/api/v1/duplicates/stats", get(duplicate_stats))
        .route("/api/v1/duplicates", get(duplicate_groups))
        .route("/api/v1/web-sources", get(list_web_sources))
        .route(
            "/api/v1/sync/manifest",
            get(sync_manifest)
                .post(selective_sync_manifest)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route("/api/v1/sync/changes", get(sync_changes))
        .route(
            "/api/v1/collections",
            get(list_collections)
                .post(create_collection)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/collections/{id}",
            axum::routing::patch(update_collection)
                .delete(delete_collection)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/collections/{collection_id}/projects/{project_id}",
            put(add_project_to_collection).delete(remove_project_from_collection),
        )
        .route(
            "/api/v1/tags",
            get(list_tags)
                .post(create_tag)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/tags/{id}",
            axum::routing::patch(update_tag)
                .delete(delete_tag)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/saved-searches",
            get(list_saved_searches)
                .post(save_saved_search)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route("/api/v1/saved-searches/{id}", delete(delete_saved_search))
        .route(
            "/api/v1/custom-fields",
            get(list_custom_fields)
                .post(save_custom_field)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route("/api/v1/custom-fields/{id}", delete(delete_custom_field))
        .route(
            "/api/v1/materials",
            get(list_materials)
                .post(save_material)
                .layer(DefaultBodyLimit::max(MAX_MUTATION_BODY_BYTES)),
        )
        .route(
            "/api/v1/uploads",
            post(upload_project_file).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES as usize)),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let migrations = Router::new()
        .route("/api/v1/migrations/preflight", get(migration_preflight))
        .route(
            "/api/v1/migrations",
            post(create_library_migration).layer(DefaultBodyLimit::max(
                library_migration::MAX_MIGRATION_MANIFEST_BYTES,
            )),
        )
        .route("/api/v1/migrations/{id}", get(get_library_migration))
        .route(
            "/api/v1/migrations/{id}/files/{file_id}",
            get(get_library_migration_file)
                .put(upload_library_migration_file)
                .layer(DefaultBodyLimit::max(
                    library_migration::MAX_MIGRATION_CHUNK_BYTES,
                )),
        )
        .route(
            "/api/v1/migrations/{id}/verify",
            post(verify_library_migration),
        )
        .route(
            "/api/v1/migrations/{id}/activate",
            post(activate_library_migration),
        )
        .route(
            "/api/v1/migrations/{id}/finish",
            post(finish_library_migration),
        )
        .route(
            "/api/v1/migrations/{id}/cancel",
            post(cancel_library_migration),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_migration,
        ));
    let mut router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route("/identity", get(identity))
        .route(
            "/pair",
            post(pair)
                .layer::<_, Infallible>(DefaultBodyLimit::max(MAX_PAIRING_BODY_BYTES))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(PAIRING_REQUEST_TIMEOUT_SECONDS),
                )),
        )
        .merge(protected)
        .merge(migrations);
    if state.web_admin.is_some() {
        let web_admin = Router::new()
            .route("/api/v1/admin/devices", get(list_admin_devices))
            .route("/api/v1/admin/devices/{id}", delete(revoke_admin_device))
            .route("/api/v1/admin/pairings", post(create_admin_pairing))
            .route(
                "/api/v1/admin/migration-pairings",
                post(create_admin_migration_pairing),
            )
            .route("/api/v1/web/logout", post(web_logout))
            .layer(middleware::from_fn_with_state(state.clone(), require_admin));
        router = router
            .route("/api/v1/web/bootstrap", get(web_bootstrap))
            .route(
                "/api/v1/web/login",
                post(web_login).layer(DefaultBodyLimit::max(MAX_PAIRING_BODY_BYTES)),
            )
            .merge(web_admin);
    }
    if let Some(directory) = web_dir {
        let index = directory.join("index.html");
        router = router.fallback_service(ServeDir::new(directory).fallback(ServeFile::new(index)));
    }
    let router = router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
        ))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
        .layer(middleware::from_fn(add_security_headers))
        .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
            "x-request-id",
        )))
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::new(
            HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ));
    router.with_state::<()>(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn readiness(State(state): State<ApiState>) -> (StatusCode, &'static str) {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.library.pool)
        .await
    {
        Ok(1) => (StatusCode::OK, "ready"),
        Ok(_) | Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "not ready"),
    }
}

async fn identity(State(state): State<ApiState>) -> Json<ServerIdentity> {
    let inner = state.controller.lock().await;
    Json(ServerIdentity {
        server_id: inner.config.server_id.clone(),
        name: server_name(&inner),
        protocol_min: PROTOCOL_VERSION,
        protocol_max: PROTOCOL_VERSION,
    })
}

async fn web_bootstrap(
    State(state): State<ApiState>,
) -> Result<Json<WebBootstrapResponse>, ApiError> {
    let admin = state.web_admin.as_ref().ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        code: "web_unavailable",
        message: "The Volum web application is not enabled.".into(),
        retryable: false,
    })?;
    let inner = state.controller.lock().await;
    let public_url = inner.base_url.clone().unwrap_or_default();
    Ok(Json(WebBootstrapResponse {
        configured: admin.configured(),
        server_id: inner.config.server_id.clone(),
        server_name: server_name(&inner),
        secure_transport: public_url.starts_with("https://"),
        public_url,
        protocol_version: PROTOCOL_VERSION,
    }))
}

async fn web_login(
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Json(request): Json<WebLoginRequest>,
) -> Result<Json<WebLoginResponse>, ApiError> {
    let admin = state.web_admin.clone().ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        code: "web_unavailable",
        message: "The Volum web application is not enabled.".into(),
        retryable: false,
    })?;
    if !admin.configured() {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "admin_not_configured",
            message: "Set VOLUM_ADMIN_PASSWORD and restart volumd before signing in.".into(),
            retryable: false,
        });
    }
    if admin.login_permitted(address.ip()).await.is_err() {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "login_throttled",
            message: "Too many sign-in attempts. Wait a minute and try again.".into(),
            retryable: true,
        });
    }
    let device_name = request.device_name.trim();
    if device_name.is_empty()
        || device_name.chars().count() > 80
        || device_name.chars().any(char::is_control)
        || request.password.len() > 4096
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_login",
            message: "Enter valid administrator details.".into(),
            retryable: false,
        });
    }
    let password = request.password;
    let verifier = admin.clone();
    let verified = tokio::task::spawn_blocking(move || verifier.verify(&password))
        .await
        .unwrap_or(false);
    if !verified {
        admin.record_failure(address.ip()).await;
        return Err(ApiError::unauthorized(
            "The administrator password is not correct.",
        ));
    }
    admin.record_success(address.ip()).await;

    let token = random_secret();
    let device_id = Uuid::new_v4().to_string();
    let mut inner = state.controller.lock().await;
    inner.config.devices.push(DeviceRecord {
        id: device_id.clone(),
        name: device_name.to_string(),
        platform: "Web".into(),
        token_hash: token_hash(&token),
        paired_at: Utc::now().to_rfc3339(),
        last_seen_at: None,
        role: "admin".into(),
        migration_id: None,
    });
    if persist_config(&inner.config_path, &inner.config).is_err() {
        inner.config.devices.pop();
        return Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "login_failed",
            message: "Volum could not save this browser session.".into(),
            retryable: true,
        });
    }
    Ok(Json(WebLoginResponse {
        server_id: inner.config.server_id.clone(),
        server_name: server_name(&inner),
        device_id,
        token,
        protocol_version: PROTOCOL_VERSION,
    }))
}

async fn list_admin_devices(State(state): State<ApiState>) -> Json<Vec<CompanionDevice>> {
    let inner = state.controller.lock().await;
    Json(inner.config.devices.iter().map(public_device).collect())
}

async fn create_admin_pairing(
    State(state): State<ApiState>,
) -> Result<Json<CompanionPairingInfo>, ApiError> {
    let mut inner = state.controller.lock().await;
    create_pairing_locked(&mut inner, "client")
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn create_admin_migration_pairing(
    State(state): State<ApiState>,
) -> Result<Json<CompanionPairingInfo>, ApiError> {
    let mut inner = state.controller.lock().await;
    create_pairing_locked(&mut inner, "migration")
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn revoke_admin_device(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let mut inner = state.controller.lock().await;
    let previous = inner.config.devices.len();
    inner.config.devices.retain(|device| device.id != id);
    if previous == inner.config.devices.len() {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "device_not_found",
            message: "The paired device no longer exists.".into(),
            retryable: false,
        });
    }
    persist_config(&inner.config_path, &inner.config).map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn web_logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let token = bearer_token(&headers)
        .ok_or_else(|| ApiError::unauthorized("This browser is not signed in."))?;
    let requested_hash = token_hash(token);
    let mut inner = state.controller.lock().await;
    inner
        .config
        .devices
        .retain(|device| device.token_hash != requested_hash);
    persist_config(&inner.config_path, &inner.config).map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_security_headers(request: Request, next: Next) -> Response {
    let private_response = request.uri().path().starts_with("/api/")
        || matches!(request.uri().path(), "/identity" | "/pair");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'self'; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; connect-src 'self'; script-src 'self'; font-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    if private_response {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

async fn pair(
    State(state): State<ApiState>,
    Json(request): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiError> {
    let device_name = request.device_name.trim();
    let platform = request.platform.trim();
    if device_name.is_empty()
        || device_name.len() > 80
        || platform.is_empty()
        || platform.len() > 32
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_device",
            message: "Enter a valid device name and platform.".into(),
            retryable: false,
        });
    }
    let mut inner = state.controller.lock().await;
    let pairing = inner.pairing.as_mut().ok_or_else(|| ApiError {
        status: StatusCode::GONE,
        code: "pairing_expired",
        message: "Create a new pairing code on Volum Desktop.".into(),
        retryable: false,
    })?;
    if pairing.expires_at < Utc::now() {
        inner.pairing = None;
        return Err(ApiError {
            status: StatusCode::GONE,
            code: "pairing_expired",
            message: "Create a new pairing code on Volum Desktop.".into(),
            retryable: false,
        });
    }
    if pairing.secret != request.secret {
        return Err(ApiError::unauthorized("The pairing code is not valid."));
    }
    let token = random_secret();
    let device_id = Uuid::new_v4().to_string();
    let role = pairing.role.clone();
    inner.config.devices.push(DeviceRecord {
        id: device_id.clone(),
        name: device_name.to_string(),
        platform: platform.to_string(),
        token_hash: token_hash(&token),
        paired_at: Utc::now().to_rfc3339(),
        last_seen_at: None,
        role,
        migration_id: None,
    });
    if persist_config(&inner.config_path, &inner.config).is_err() {
        inner.config.devices.pop();
        return Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "pairing_failed",
            message: "Volum could not save this paired device.".into(),
            retryable: true,
        });
    }
    inner.pairing = None;
    Ok(Json(PairResponse {
        server_id: inner.config.server_id.clone(),
        device_id,
        token,
        protocol_version: PROTOCOL_VERSION,
    }))
}

async fn require_auth(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    authorize_headers(&state, request.headers(), None).await?;
    Ok(next.run(request).await)
}

async fn require_admin(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    authorize_headers(&state, request.headers(), Some("admin")).await?;
    Ok(next.run(request).await)
}

async fn require_migration(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let path = request.uri().path();
    let migration_id = if request.method() == Method::GET {
        path.strip_prefix("/api/v1/migrations/")
    } else if request.method() == Method::POST {
        path.strip_prefix("/api/v1/migrations/")
            .and_then(|path| path.strip_suffix("/finish"))
    } else {
        None
    };
    let allow_completed_client = migration_id.is_some_and(|id| Uuid::parse_str(id).is_ok());
    authorize_headers(
        &state,
        request.headers(),
        Some(if allow_completed_client {
            "migration-or-client"
        } else {
            "migration"
        }),
    )
    .await?;
    Ok(next.run(request).await)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}

async fn authorize_headers(
    state: &ApiState,
    headers: &HeaderMap,
    required_role: Option<&str>,
) -> Result<(), ApiError> {
    let token = bearer_token(headers)
        .ok_or_else(|| ApiError::unauthorized("Pair this device with Volum Desktop."))?;
    let requested_hash = token_hash(token);
    let mut inner = state.controller.lock().await;
    let now = Utc::now();
    let Some(device) = inner
        .config
        .devices
        .iter_mut()
        .find(|device| device.token_hash == requested_hash)
    else {
        return Err(ApiError::unauthorized(
            "This device is not paired or has been revoked.",
        ));
    };
    if required_role.is_none() && device.role == "migration" {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "migration_scope_only",
            message: "This temporary token can be used only for the library migration.".into(),
            retryable: false,
        });
    }
    let role_matches = match required_role {
        None => device.role != "migration",
        Some("migration-or-client") => matches!(device.role.as_str(), "migration" | "client"),
        Some(role) => device.role == role,
    };
    if !role_matches {
        let migration_required = matches!(required_role, Some("migration" | "migration-or-client"));
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: if migration_required {
                "migration_token_required"
            } else {
                "administrator_required"
            },
            message: if migration_required {
                "Create and use a one-time library migration code from the Volum administrator."
                    .into()
            } else {
                "Sign in as the Volum administrator to manage devices.".into()
            },
            retryable: false,
        });
    }
    let should_persist = device
        .last_seen_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_none_or(|seen| {
            now.signed_duration_since(seen.with_timezone(&Utc))
                .num_seconds()
                >= 60
        });
    if should_persist {
        device.last_seen_at = Some(now.to_rfc3339());
        let _ = persist_config(&inner.config_path, &inner.config);
    }
    drop(inner);
    Ok(())
}

async fn require_headless_migration(state: &ApiState) -> Result<(), ApiError> {
    if state.controller.lock().await.headless {
        Ok(())
    } else {
        Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "nas_migration_required",
            message:
                "Library migration targets a dedicated volumd server, not another Desktop host."
                    .into(),
            retryable: false,
        })
    }
}

fn migration_bearer_hash(headers: &HeaderMap) -> Result<String, ApiError> {
    bearer_token(headers)
        .map(token_hash)
        .ok_or_else(|| ApiError::unauthorized("The migration token is missing."))
}

async fn check_migration_job_claim(
    state: &ApiState,
    headers: &HeaderMap,
    migration_id: &str,
) -> Result<String, ApiError> {
    let requested_hash = migration_bearer_hash(headers)?;
    let inner = state.controller.lock().await;
    let device = inner
        .config
        .devices
        .iter()
        .find(|device| device.token_hash == requested_hash && device.role == "migration")
        .ok_or_else(|| ApiError::unauthorized("The migration device is no longer paired."))?;
    if device
        .migration_id
        .as_deref()
        .is_some_and(|claimed| claimed != migration_id)
        || inner.config.devices.iter().any(|candidate| {
            candidate.token_hash != requested_hash
                && candidate.migration_id.as_deref() == Some(migration_id)
        })
    {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "migration_job_forbidden",
            message: "This transfer code belongs to another library migration.".into(),
            retryable: false,
        });
    }
    Ok(requested_hash)
}

async fn claim_migration_job(
    state: &ApiState,
    token_hash: &str,
    migration_id: &str,
) -> Result<(), ApiError> {
    let mut inner = state.controller.lock().await;
    let device = inner
        .config
        .devices
        .iter_mut()
        .find(|device| device.token_hash == token_hash && device.role == "migration")
        .ok_or_else(|| ApiError::unauthorized("The migration device is no longer paired."))?;
    device.migration_id = Some(migration_id.to_string());
    persist_config(&inner.config_path, &inner.config).map_err(ApiError::migration)
}

async fn require_migration_job_owner(
    state: &ApiState,
    headers: &HeaderMap,
    migration_id: &str,
    allow_completed_client: bool,
) -> Result<String, ApiError> {
    let requested_hash = migration_bearer_hash(headers)?;
    let inner = state.controller.lock().await;
    let owns_job = inner.config.devices.iter().any(|device| {
        device.token_hash == requested_hash
            && (device.role == "migration" || (allow_completed_client && device.role == "client"))
            && device.migration_id.as_deref() == Some(migration_id)
    });
    if !owns_job {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "migration_job_forbidden",
            message: "This transfer code does not own the requested library migration.".into(),
            retryable: false,
        });
    }
    Ok(requested_hash)
}

async fn migration_preflight(
    State(state): State<ApiState>,
) -> Result<Json<library_migration::MigrationServerPreflight>, ApiError> {
    require_headless_migration(&state).await?;
    library_migration::server_preflight(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::migration)
}

async fn create_library_migration(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(manifest): Json<library_migration::MigrationManifest>,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    let _guard = state.migration_lock.lock().await;
    let migration_id = manifest.id.clone();
    let requested_hash = check_migration_job_claim(&state, &headers, &migration_id).await?;
    let status = library_migration::create_server_job(manifest, &state.library)
        .await
        .map_err(ApiError::migration)?;
    claim_migration_job(&state, &requested_hash, &migration_id).await?;
    Ok(Json(status))
}

async fn get_library_migration(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, true).await?;
    library_migration::get_server_job(&state.library.data_dir, &id)
        .map(Json)
        .map_err(ApiError::migration)
}

async fn get_library_migration_file(
    State(state): State<ApiState>,
    Path((id, file_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationUploadStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, false).await?;
    library_migration::server_upload_status(&state.library.data_dir, &id, &file_id)
        .map(Json)
        .map_err(ApiError::migration)
}

async fn upload_library_migration_file(
    State(state): State<ApiState>,
    Path((id, file_id)): Path<(String, String)>,
    Query(query): Query<MigrationOffsetQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<library_migration::MigrationUploadStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, false).await?;
    let _guard = state.migration_lock.lock().await;
    library_migration::append_server_chunk(
        &state.library.data_dir,
        &id,
        &file_id,
        query.offset,
        &body,
    )
    .map(Json)
    .map_err(ApiError::migration)
}

async fn verify_library_migration(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, false).await?;
    let data_dir = state.library.data_dir.clone();
    let task_id = id.clone();
    let lock = state.migration_lock.clone();
    tokio::spawn(async move {
        let _guard = lock.lock().await;
        if let Err(error) = library_migration::verify_server_job(&data_dir, &task_id).await {
            log::warn!("Library migration verification failed: {error}");
        }
    });
    tokio::task::yield_now().await;
    library_migration::get_server_job(&state.library.data_dir, &id)
        .map(Json)
        .map_err(ApiError::migration)
}

async fn activate_library_migration(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, false).await?;
    let status = {
        let _guard = state.migration_lock.lock().await;
        library_migration::prepare_server_activation(&state.library.data_dir, &id)
            .map_err(ApiError::migration)?
    };
    #[cfg(not(test))]
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        std::process::exit(0);
    });
    Ok(Json(status))
}

async fn finish_library_migration(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    let requested_hash = require_migration_job_owner(&state, &headers, &id, true).await?;
    let status = library_migration::get_server_job(&state.library.data_dir, &id)
        .map_err(ApiError::migration)?;
    if status.state != "complete" {
        return Err(ApiError::conflict(
            "The NAS must finish activating before the migration device becomes a normal client.",
        ));
    }
    let mut inner = state.controller.lock().await;
    let device = inner
        .config
        .devices
        .iter_mut()
        .find(|device| device.token_hash == requested_hash)
        .ok_or_else(|| ApiError::unauthorized("The migration device is no longer paired."))?;
    if device.role == "client" {
        return Ok(Json(status));
    }
    if device.role != "migration" {
        return Err(ApiError::unauthorized(
            "This token does not own the migration.",
        ));
    }
    device.role = "client".into();
    persist_config(&inner.config_path, &inner.config).map_err(ApiError::migration)?;
    Ok(Json(status))
}

async fn cancel_library_migration(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<library_migration::MigrationJobStatus>, ApiError> {
    require_headless_migration(&state).await?;
    require_migration_job_owner(&state, &headers, &id, false).await?;
    let _guard = state.migration_lock.lock().await;
    library_migration::cancel_server_job(&state.library.data_dir, &id)
        .map(Json)
        .map_err(ApiError::migration)
}

async fn api_status(State(state): State<ApiState>) -> Result<Json<ServerStatus>, ApiError> {
    let project_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE trashed_at IS NULL")
            .fetch_one(&state.library.pool)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
    let library_revision: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) FROM sync_changes")
            .fetch_one(&state.library.pool)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
    let inner = state.controller.lock().await;
    Ok(Json(ServerStatus {
        server_id: inner.config.server_id.clone(),
        name: server_name(&inner),
        protocol_version: PROTOCOL_VERSION,
        project_count,
        library_revision: library_revision.to_string(),
        capabilities: ServerCapabilities {
            read_library: true,
            organization_writes: inner.config.organization_writes,
            photo_uploads: inner.config.photo_uploads.unwrap_or(false),
            file_uploads: inner.config.organization_writes,
            viewer_meshes: true,
            plate_thumbnails: true,
            cost_estimates: true,
            related_projects: true,
            live_events: false,
            original_files: true,
            incremental_sync: true,
        },
    }))
}

async fn require_organization_writes(state: &ApiState) -> Result<(), ApiError> {
    if state.controller.lock().await.config.organization_writes {
        Ok(())
    } else {
        Err(ApiError::writes_disabled())
    }
}

async fn require_photo_uploads(state: &ApiState) -> Result<(), ApiError> {
    if state
        .controller
        .lock()
        .await
        .config
        .photo_uploads
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(ApiError::photo_uploads_disabled())
    }
}

async fn list_projects(
    State(state): State<ApiState>,
    Query(mut query): Query<ModelQuery>,
) -> Result<Json<CompanionPage<CompanionProjectSummary>>, ApiError> {
    if query
        .search
        .as_deref()
        .is_some_and(|search| search.chars().count() > MAX_SEARCH_CHARACTERS)
        || query
            .cursor
            .as_deref()
            .is_some_and(|cursor| cursor.len() > MAX_CURSOR_BYTES)
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request",
            message: "The project query is too long.".into(),
            retryable: false,
        });
    }
    query.limit = Some(query.limit.unwrap_or(48).clamp(1, 100));
    query.offset = None;
    query.trashed = Some(query.trashed.unwrap_or(false));
    let page = commands::list_models_impl(query, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    Ok(Json(page.into()))
}

async fn get_project(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    load_project_detail(&id, &state).await.map(Json)
}

async fn create_project(
    State(state): State<ApiState>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    require_organization_writes(&state).await?;
    project_storage::validate_project_name(&request.name).map_err(ApiError::invalid_mutation)?;
    let input = project_storage::CreateManagedProjectInput {
        name: request.name,
        file_paths: Vec::new(),
        asset_ids: Vec::new(),
        transfer_mode: "copy".into(),
    };
    let _guard = state.library.file_operation_lock.lock().await;
    let (project_id, _) = project_storage::create_managed_project_internal(&input, &state.library)
        .await
        .map_err(ApiError::from_upload_import)?;
    load_project_detail(&project_id, &state).await.map(Json)
}

async fn trash_project(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    project_storage::trash_project_impl(&id, &state.library, state.app.as_ref())
        .await
        .map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_project(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    project_storage::restore_project_from_trash_impl(&id, &state.library, state.app.as_ref())
        .await
        .map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_project_permanently(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    project_storage::delete_project_permanently_impl(&id, &state.library, state.app.as_ref())
        .await
        .map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_project_detail(
    id: &str,
    state: &ApiState,
) -> Result<CompanionProjectDetail, ApiError> {
    let project = commands::get_model_impl(id.to_string(), &state.library)
        .await
        .map_err(ApiError::from_library)?;
    let revision: String =
        sqlx::query_scalar("SELECT updated_at FROM models WHERE id = ? AND trashed_at IS NULL")
            .bind(id)
            .fetch_optional(&state.library.pool)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?
            .ok_or_else(|| ApiError::from_library("Project not found".into()))?;
    let mut response: CompanionProjectDetail = project.into();
    response.revision = revision;
    Ok(response)
}

async fn update_project(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<ProjectUpdateRequest>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    require_organization_writes(&state).await?;
    let ProjectUpdateRequest {
        display_name,
        favorite,
        notes,
        tag_ids,
        if_unmodified_since,
    } = request;
    if display_name.is_none() && favorite.is_none() && notes.is_none() && tag_ids.is_none() {
        return Err(ApiError::invalid_mutation(
            "Choose a project change to save.",
        ));
    }
    if if_unmodified_since.trim().is_empty() {
        return Err(ApiError::invalid_mutation(
            "Refresh this project before editing it.",
        ));
    }
    if notes
        .as_ref()
        .is_some_and(|notes| notes.chars().count() > 100_000)
    {
        return Err(ApiError::invalid_mutation(
            "Notes may contain up to 100,000 characters.",
        ));
    }
    if tag_ids.as_ref().is_some_and(|ids| ids.len() > 100) {
        return Err(ApiError::invalid_mutation(
            "A project may have at most 100 tags.",
        ));
    }
    if let Some(name) = display_name {
        if favorite.is_some() || notes.is_some() || tag_ids.is_some() {
            return Err(ApiError::invalid_mutation(
                "Rename the project separately from other changes.",
            ));
        }
        project_storage::rename_project_if_revision(
            &id,
            &name,
            &if_unmodified_since,
            &state.library,
            state.app.as_ref(),
        )
        .await
        .map_err(|error| {
            if error == project_storage::PROJECT_REVISION_CONFLICT {
                ApiError::conflict("This project changed on another device. Refresh and try again.")
            } else if error.contains("must")
                || error.contains("cannot")
                || error.contains("already exists")
            {
                ApiError::invalid_mutation(error)
            } else {
                ApiError::from_library(error)
            }
        })?;
        return load_project_detail(&id, &state).await.map(Json);
    }

    let mut tag_ids = tag_ids;
    if let Some(ids) = &mut tag_ids {
        ids.sort();
        ids.dedup();
    }
    let timestamp = Utc::now().to_rfc3339();
    let mut transaction = state
        .library
        .pool
        .begin()
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let result = sqlx::query(
        "UPDATE models SET favorite = COALESCE(?, favorite), notes = COALESCE(?, notes), updated_at = ? WHERE id = ? AND trashed_at IS NULL AND updated_at = ?",
    )
    .bind(favorite.map(i64::from))
    .bind(notes)
    .bind(&timestamp)
    .bind(&id)
    .bind(&if_unmodified_since)
    .execute(&mut *transaction)
    .await
    .map_err(|error| ApiError::from_library(error.to_string()))?;
    if result.rows_affected() == 0 {
        transaction
            .rollback()
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM models WHERE id = ? AND trashed_at IS NULL)",
        )
        .bind(&id)
        .fetch_one(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
        return Err(if exists {
            ApiError::conflict("This project changed on another device. Refresh and try again.")
        } else {
            ApiError::from_library("Project not found".into())
        });
    }
    if let Some(tag_ids) = tag_ids {
        let encoded = serde_json::to_string(&tag_ids)
            .map_err(|error| ApiError::invalid_mutation(error.to_string()))?;
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tags WHERE id IN (SELECT value FROM json_each(?))",
        )
        .bind(encoded)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
        if existing != tag_ids.len() as i64 {
            transaction
                .rollback()
                .await
                .map_err(|error| ApiError::from_library(error.to_string()))?;
            return Err(ApiError::invalid_mutation(
                "One or more selected tags no longer exist.",
            ));
        }
        sqlx::query("DELETE FROM model_tags WHERE model_id = ?")
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
        for tag_id in tag_ids {
            sqlx::query("INSERT INTO model_tags (model_id, tag_id, added_at) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(tag_id)
                .bind(&timestamp)
                .execute(&mut *transaction)
                .await
                .map_err(|error| ApiError::from_library(error.to_string()))?;
        }
    }
    transaction
        .commit()
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    if let Err(error) = indexing::refresh_search(&id, &state.library.pool).await {
        log::warn!("Could not refresh search after a mobile project edit: {error}");
    }
    load_project_detail(&id, &state).await.map(Json)
}

async fn get_documentation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    load_documentation(&id, &state).await.map(Json)
}

async fn load_documentation(
    id: &str,
    state: &ApiState,
) -> Result<CompanionDocumentation, ApiError> {
    let documentation = project_docs::documentation_for_project(id, &state.library.pool)
        .await
        .map_err(ApiError::from_library)?;
    let revision: String =
        sqlx::query_scalar("SELECT updated_at FROM models WHERE id = ? AND trashed_at IS NULL")
            .bind(id)
            .fetch_optional(&state.library.pool)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?
            .ok_or_else(|| ApiError::from_library("Project not found".into()))?;
    Ok(CompanionDocumentation {
        documentation,
        revision,
    })
}

async fn upload_project_photo(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(query): Query<PhotoUploadQuery>,
    request: Request,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_photo_uploads(&state).await?;
    let filename = validate_photo_filename(&query.filename)?;
    let caption = query.caption.trim();
    if caption.chars().count() > 500 {
        return Err(ApiError::invalid_mutation(
            "Photo captions may contain up to 500 characters.",
        ));
    }
    let byte_size = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| ApiError {
            status: StatusCode::LENGTH_REQUIRED,
            code: "upload_size_required",
            message: "The photo size is required before uploading.".into(),
            retryable: false,
        })?;
    if byte_size == 0 || byte_size > MAX_PHOTO_UPLOAD_BYTES {
        return Err(ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "photo_too_large",
            message: "Project photos must be non-empty and at most 25 MB.".into(),
            retryable: false,
        });
    }
    let _permit = state
        .upload_slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "upload_busy",
            message: "Another Companion upload is being imported. Try again shortly.".into(),
            retryable: true,
        })?;
    let claimed = claim_project_revision(&id, &query.if_unmodified_since, &state).await?;
    let upload_directory = state
        .library
        .data_dir
        .join("companion-uploads")
        .join(Uuid::new_v4().to_string());
    if let Err(error) = tokio::fs::create_dir_all(&upload_directory).await {
        release_project_revision(&id, &claimed, &query.if_unmodified_since, &state).await;
        log::warn!("Could not prepare Companion photo staging: {error}");
        return Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "upload_failed",
            message: "Volum Desktop could not prepare this photo upload.".into(),
            retryable: true,
        });
    }
    let upload_path = upload_directory.join(&filename);
    if let Err(error) = receive_upload(
        request.into_body(),
        &upload_path,
        byte_size,
        MAX_PHOTO_UPLOAD_BYTES,
    )
    .await
    {
        let _ = tokio::fs::remove_dir_all(&upload_directory).await;
        release_project_revision(&id, &claimed, &query.if_unmodified_since, &state).await;
        return Err(error);
    }
    let result = {
        let _guard = state.library.file_operation_lock.lock().await;
        project_docs::store_project_photo(
            &id,
            &upload_path,
            Some(&filename),
            caption,
            None,
            None,
            &state.library,
        )
        .await
    };
    let _ = tokio::fs::remove_dir_all(&upload_directory).await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &query.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn update_project_photo(
    State(state): State<ApiState>,
    Path((id, photo_id)): Path<(String, String)>,
    Json(request): Json<PhotoUpdateRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    ensure_documentation_owner("project_photos", &photo_id, &id, &state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = project_docs::update_project_photo_impl(
        project_docs::ProjectPhotoUpdate {
            id: photo_id,
            caption: request.caption,
            sort_order: request.sort_order,
            is_cover: request.is_cover,
        },
        &state.library,
    )
    .await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn delete_project_photo(
    State(state): State<ApiState>,
    Path((id, photo_id)): Path<(String, String)>,
    Json(request): Json<RevisionRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    ensure_documentation_owner("project_photos", &photo_id, &id, &state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = {
        let _guard = state.library.file_operation_lock.lock().await;
        project_docs::delete_project_photo_impl(&photo_id, &state.library).await
    };
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn save_cost_estimate(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<CostEstimateRequest>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    require_organization_writes(&state).await?;
    if !request.plastic_g.is_finite()
        || request.plastic_g < 0.0
        || request.plastic_g > 1_000_000.0
        || !(1..=100_000).contains(&request.quantity)
        || !matches!(request.source.as_str(), "manual" | "3mf" | "web")
    {
        return Err(ApiError::invalid_mutation(
            "Enter a valid material usage, quantity, and estimate source.",
        ));
    }
    if let Some(asset_id) = request.asset_id.as_deref() {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM model_assets WHERE model_id = ? AND asset_id = ?)",
        )
        .bind(&id)
        .bind(asset_id)
        .fetch_one(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
        if !owned {
            return Err(ApiError::invalid_mutation(
                "The selected file does not belong to this project.",
            ));
        }
    }
    if let Some(material_id) = request.material_id.as_deref() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM materials WHERE id = ?)")
                .bind(material_id)
                .fetch_one(&state.library.pool)
                .await
                .map_err(|error| ApiError::from_library(error.to_string()))?;
        if !exists {
            return Err(ApiError::invalid_mutation(
                "The selected material is no longer available.",
            ));
        }
    }
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = commands::save_cost_estimate_impl(
        CostEstimateInput {
            model_id: id.clone(),
            asset_id: request.asset_id,
            material_id: request.material_id,
            plastic_g: request.plastic_g,
            quantity: request.quantity,
            source: request.source,
        },
        &state.library,
    )
    .await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_project_detail(&id, &state).await.map(Json)
}

async fn list_related_projects(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<RelatedModel>>, ApiError> {
    commands::list_related_models_impl(&id, &state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn claim_project_revision(
    id: &str,
    expected: &str,
    state: &ApiState,
) -> Result<String, ApiError> {
    if expected.trim().is_empty() {
        return Err(ApiError::invalid_mutation(
            "Refresh this project before editing it.",
        ));
    }
    let claimed_revision = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE models SET updated_at = ? WHERE id = ? AND trashed_at IS NULL AND updated_at = ?",
    )
    .bind(&claimed_revision)
    .bind(id)
    .bind(expected)
    .execute(&state.library.pool)
    .await
    .map_err(|error| ApiError::from_library(error.to_string()))?;
    if result.rows_affected() != 0 {
        return Ok(claimed_revision);
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM models WHERE id = ? AND trashed_at IS NULL)",
    )
    .bind(id)
    .fetch_one(&state.library.pool)
    .await
    .map_err(|error| ApiError::from_library(error.to_string()))?;
    Err(if exists {
        ApiError::conflict("This project changed on another device. Refresh and try again.")
    } else {
        ApiError::from_library("Project not found".into())
    })
}

async fn release_project_revision(id: &str, claimed: &str, previous: &str, state: &ApiState) {
    if let Err(error) =
        sqlx::query("UPDATE models SET updated_at = ? WHERE id = ? AND updated_at = ?")
            .bind(previous)
            .bind(id)
            .bind(claimed)
            .execute(&state.library.pool)
            .await
    {
        log::warn!("Could not release a failed mobile edit claim: {error}");
    }
}

fn documentation_write_error(error: String) -> ApiError {
    if error.to_ascii_lowercase().contains("not found") {
        return ApiError::from_library(error);
    }
    const SAFE_VALIDATION_TERMS: &[&str] = &[
        "must",
        "need",
        "cannot",
        "invalid",
        "belongs to another",
        "enter a valid",
        "only http",
        "may contain",
        "may have",
        "does not belong",
    ];
    if SAFE_VALIDATION_TERMS
        .iter()
        .any(|term| error.to_ascii_lowercase().contains(term))
    {
        ApiError::invalid_mutation(error)
    } else {
        log::warn!("Mobile documentation write failed: {error}");
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "request_failed",
            message: "Volum Desktop could not save this project change.".into(),
            retryable: true,
        }
    }
}

async fn set_custom_value(
    State(state): State<ApiState>,
    Path((id, field_id)): Path<(String, String)>,
    Json(request): Json<CustomValueRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = project_docs::set_project_custom_value_impl(
        project_docs::ProjectCustomValueInput {
            model_id: id.clone(),
            field_id,
            value: request.value,
        },
        &state.library,
    )
    .await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn save_bom_item(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<BomItemRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = project_docs::save_project_bom_item_impl(
        project_docs::ProjectBomItemInput {
            id: request.id,
            model_id: id.clone(),
            kind: request.kind,
            linked_asset_id: request.linked_asset_id,
            name: request.name,
            quantity: request.quantity,
            unit: request.unit,
            url: request.url,
            notes: request.notes,
            sort_order: request.sort_order,
        },
        &state.library,
    )
    .await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn delete_bom_item(
    State(state): State<ApiState>,
    Path((id, item_id)): Path<(String, String)>,
    Json(request): Json<RevisionRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    ensure_documentation_owner("project_bom_items", &item_id, &id, &state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    if let Err(error) = project_docs::delete_project_bom_item_impl(&item_id, &state.library).await {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn save_assembly_step(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<AssemblyStepRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    let result = project_docs::save_assembly_step_impl(
        project_docs::AssemblyStepInput {
            id: request.id,
            model_id: id.clone(),
            title: request.title,
            instructions: request.instructions,
            sort_order: request.sort_order,
            bom_item_ids: request.bom_item_ids,
            photo_ids: request.photo_ids,
        },
        &state.library,
    )
    .await;
    if let Err(error) = result {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn delete_assembly_step(
    State(state): State<ApiState>,
    Path((id, step_id)): Path<(String, String)>,
    Json(request): Json<RevisionRequest>,
) -> Result<Json<CompanionDocumentation>, ApiError> {
    require_organization_writes(&state).await?;
    ensure_documentation_owner("assembly_steps", &step_id, &id, &state).await?;
    let claimed = claim_project_revision(&id, &request.if_unmodified_since, &state).await?;
    if let Err(error) = project_docs::delete_assembly_step_impl(&step_id, &state.library).await {
        release_project_revision(&id, &claimed, &request.if_unmodified_since, &state).await;
        return Err(documentation_write_error(error));
    }
    load_documentation(&id, &state).await.map(Json)
}

async fn ensure_documentation_owner(
    table: &str,
    item_id: &str,
    project_id: &str,
    state: &ApiState,
) -> Result<(), ApiError> {
    let query = match table {
        "project_photos" => "SELECT model_id FROM project_photos WHERE id = ?",
        "project_bom_items" => "SELECT model_id FROM project_bom_items WHERE id = ?",
        "assembly_steps" => "SELECT model_id FROM assembly_steps WHERE id = ?",
        _ => return Err(ApiError::from_library("Item not found".into())),
    };
    let owner: Option<String> = sqlx::query_scalar(query)
        .bind(item_id)
        .fetch_optional(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    if owner.as_deref() == Some(project_id) {
        Ok(())
    } else {
        Err(ApiError::from_library("Item not found".into()))
    }
}

async fn get_project_cover(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let row = sqlx::query("SELECT m.primary_asset_id, (SELECT pp.id FROM project_photos pp WHERE pp.model_id = m.id AND pp.is_cover = 1 LIMIT 1) cover_photo_id FROM models m WHERE m.id = ? AND m.trashed_at IS NULL")
        .bind(id)
        .fetch_optional(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?
        .ok_or_else(|| ApiError::from_library("Project not found".into()))?;
    if let Some(photo_id) = row.get::<Option<String>, _>("cover_photo_id") {
        let (bytes, mime_type) = project_docs::project_photo_bytes(&photo_id, true, &state.library)
            .await
            .map_err(ApiError::from_library)?;
        return binary_response(bytes, &mime_type, &request_headers);
    }
    let asset_id = row
        .get::<Option<String>, _>("primary_asset_id")
        .ok_or_else(|| ApiError::from_library("Project cover not available".into()))?;
    let bytes = commands::thumbnail_bytes_impl(&asset_id, &state.library)
        .await
        .map_err(ApiError::from_library)?
        .ok_or_else(|| ApiError::from_library("Project cover not available".into()))?;
    binary_response(bytes, "image/png", &request_headers)
}

async fn get_asset_thumbnail(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bytes = commands::thumbnail_bytes_impl(&id, &state.library)
        .await
        .map_err(ApiError::from_library)?
        .ok_or_else(|| ApiError::from_library("Asset thumbnail not available".into()))?;
    binary_response(bytes, "image/png", &request_headers)
}

async fn get_plate_thumbnail(
    State(state): State<ApiState>,
    Path((id, plate_index)): Path<(String, u32)>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bytes = commands::plate_thumbnail_bytes(&id, plate_index, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    binary_response(bytes, "image/png", &request_headers)
}

async fn get_viewer_mesh(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(query): Query<ViewerMeshQuery>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _viewer_slot = state.viewer_slot.acquire().await.map_err(|_| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "viewer_unavailable",
        message: "The model viewer is temporarily unavailable.".into(),
        retryable: true,
    })?;
    let bytes = commands::viewer_mesh_bytes(&id, query.plate_index, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    if bytes.len() > MAX_VIEWER_MESH_BYTES {
        return Err(ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "viewer_mesh_too_large",
            message: "This model is too detailed for the mobile viewer.".into(),
            retryable: false,
        });
    }
    binary_response(bytes, "application/octet-stream", &request_headers)
}

async fn get_photo(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(query): Query<PhotoQuery>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (bytes, mime_type) =
        project_docs::project_photo_bytes(&id, query.thumbnail, &state.library)
            .await
            .map_err(ApiError::from_library)?;
    binary_response(bytes, &mime_type, &request_headers)
}

async fn get_asset_content(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    asset_content_response(&id, &headers, false, &state).await
}

async fn head_asset_content(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    asset_content_response(&id, &headers, true, &state).await
}

async fn put_asset_content(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Json<AssetContentUpdate>, ApiError> {
    require_organization_writes(&state).await?;
    let expected = request
        .headers()
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::conflict("Refresh the mirrored file before uploading it."))?
        .trim()
        .trim_matches('"')
        .to_string();
    let content_length = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| ApiError::invalid_upload("The replacement file size is required."))?;
    if content_length == 0 || content_length > MAX_UPLOAD_BYTES {
        return Err(ApiError::invalid_upload(
            "Replacement files must be non-empty and at most 500 MB.",
        ));
    }
    let row = sqlx::query("SELECT a.root_id, a.relative_path, m.storage_mode, r.path root_path FROM assets a JOIN model_assets ma ON ma.asset_id = a.id JOIN models m ON m.id = ma.model_id JOIN library_roots r ON r.id = a.root_id WHERE a.id = ? AND a.missing_since IS NULL AND m.trashed_at IS NULL LIMIT 1")
        .bind(&id)
        .fetch_optional(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?
        .ok_or_else(|| ApiError::from_library("Asset not found".into()))?;
    if row.get::<String, _>("storage_mode") != "managed" {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "managed_asset_required",
            message: "Only files in the server's managed library can be changed from a mirror."
                .into(),
            retryable: false,
        });
    }
    let root_id: String = row.get("root_id");
    let relative_path: String = row.get("relative_path");
    let root_path = PathBuf::from(row.get::<String, _>("root_path"));
    let path = commands::asset_path(&id, &state.library)
        .await
        .map_err(ApiError::from_library)?
        .0;
    let hash_path = path.clone();
    let current_hash = tokio::task::spawn_blocking(move || hash_file(&hash_path))
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?
        .map_err(ApiError::from_library)?;
    if current_hash != expected {
        return Err(ApiError::conflict(
            "The server file changed after this mirror was downloaded.",
        ));
    }
    let temporary = path.with_extension(format!("volum-upload-{}.part", Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let mut stream = request.into_body().into_data_stream();
    let mut received = 0_u64;
    let mut hasher = blake3::Hasher::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|_| ApiError::invalid_upload("The replacement upload was interrupted."))?;
        received = received.saturating_add(chunk.len() as u64);
        if received > content_length || received > MAX_UPLOAD_BYTES {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(ApiError::invalid_upload(
                "The replacement file is larger than declared.",
            ));
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
    }
    file.flush()
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    drop(file);
    if received != content_length {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(ApiError::invalid_upload(
            "The replacement upload was incomplete.",
        ));
    }
    let _guard = state.library.file_operation_lock.lock().await;
    let verify_path = path.clone();
    let latest_hash = tokio::task::spawn_blocking(move || hash_file(&verify_path))
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?
        .map_err(ApiError::from_library)?;
    if latest_hash != expected {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(ApiError::conflict(
            "The server file changed while the replacement was uploading.",
        ));
    }
    replace_file(&temporary, &path)
        .await
        .map_err(ApiError::from_library)?;
    indexing::index_imported_files(
        &root_id,
        &root_path,
        std::slice::from_ref(&relative_path),
        &state.library.pool,
    )
    .await
    .map_err(ApiError::from_library)?;
    let content_hash = hasher.finalize().to_hex().to_string();
    let modified_at: String = sqlx::query_scalar("SELECT modified_at FROM assets WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    sqlx::query("UPDATE assets SET content_hash = ? WHERE id = ?")
        .bind(&content_hash)
        .bind(&id)
        .execute(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    if let Some(app) = &state.app {
        app.emit("models-changed", ());
    }
    Ok(Json(AssetContentUpdate {
        asset_id: id,
        byte_size: received,
        content_hash,
        modified_at,
    }))
}

async fn replace_file(source: &FilePath, destination: &FilePath) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        tokio::fs::rename(source, destination)
            .await
            .map_err(|error| format!("Could not activate the replacement file: {error}"))
    }
    #[cfg(windows)]
    {
        let backup = destination.with_extension(format!("volum-replace-{}.bak", Uuid::new_v4()));
        tokio::fs::rename(destination, &backup)
            .await
            .map_err(|error| format!("Could not prepare the existing file: {error}"))?;
        if let Err(error) = tokio::fs::rename(source, destination).await {
            let _ = tokio::fs::rename(&backup, destination).await;
            return Err(format!("Could not activate the replacement file: {error}"));
        }
        let _ = tokio::fs::remove_file(backup).await;
        Ok(())
    }
}

async fn asset_content_response(
    asset_id: &str,
    request_headers: &HeaderMap,
    head_only: bool,
    state: &ApiState,
) -> Result<Response, ApiError> {
    let row = sqlx::query("SELECT a.filename, a.byte_size, a.content_hash, a.missing_since FROM assets a WHERE a.id = ?")
        .bind(asset_id)
        .fetch_optional(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?
        .ok_or_else(|| ApiError::from_library("Asset not found".into()))?;
    if row.get::<Option<String>, _>("missing_since").is_some() {
        return Err(ApiError::from_library("Asset is not available".into()));
    }
    let filename: String = row.get("filename");
    let path = commands::asset_path(asset_id, &state.library)
        .await
        .map_err(ApiError::from_library)?
        .0;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| ApiError::from_library("Asset is not available".into()))?;
    let byte_size = metadata.len();
    if row.get::<i64, _>("byte_size").max(0) as u64 != byte_size {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "asset_changed",
            message: "This file changed while Volum was preparing it. Try again shortly.".into(),
            retryable: true,
        });
    }
    let content_hash = match row.get::<Option<String>, _>("content_hash") {
        Some(hash) if !hash.is_empty() => hash,
        _ => {
            let hash_path = path.clone();
            let hash = tokio::task::spawn_blocking(move || hash_file(&hash_path))
                .await
                .map_err(|error| ApiError::from_library(error.to_string()))?
                .map_err(ApiError::from_library)?;
            sqlx::query("UPDATE assets SET content_hash = ? WHERE id = ? AND content_hash IS NULL")
                .bind(&hash)
                .bind(asset_id)
                .execute(&state.library.pool)
                .await
                .map_err(|error| ApiError::from_library(error.to_string()))?;
            hash
        }
    };
    let etag = format!("\"{content_hash}\"");
    if request_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
        && request_headers.get(header::RANGE).is_none()
    {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(&etag)
                .map_err(|_| ApiError::from_library("Invalid asset identity".into()))?,
        );
        return Ok(response);
    }
    let if_range_matches = request_headers
        .get(header::IF_RANGE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.trim() == etag);
    let range = if if_range_matches {
        request_headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(|value| parse_byte_range(value, byte_size))
            .transpose()?
    } else {
        None
    };
    let (start, end, status) = range
        .map(|(start, end)| (start, end, StatusCode::PARTIAL_CONTENT))
        .unwrap_or((0, byte_size.saturating_sub(1), StatusCode::OK));
    let response_length = if byte_size == 0 { 0 } else { end - start + 1 };
    let body = if head_only || response_length == 0 {
        Body::empty()
    } else {
        let mut file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| ApiError::from_library("Asset is not available".into()))?;
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|error| ApiError::from_library(error.to_string()))?;
        Body::from_stream(tokio_util::io::ReaderStream::new(
            file.take(response_length),
        ))
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&response_length.to_string())
            .map_err(|_| ApiError::from_library("Invalid asset size".into()))?,
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag)
            .map_err(|_| ApiError::from_library("Invalid asset identity".into()))?,
    );
    let encoded_filename =
        percent_encoding::utf8_percent_encode(&filename, percent_encoding::NON_ALPHANUMERIC);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename*=UTF-8''{encoded_filename}"))
            .map_err(|_| ApiError::from_library("Invalid asset filename".into()))?,
    );
    if status == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{byte_size}"))
                .map_err(|_| ApiError::from_library("Invalid asset range".into()))?,
        );
    }
    Ok(response)
}

fn parse_byte_range(value: &str, byte_size: u64) -> Result<(u64, u64), ApiError> {
    let invalid = || ApiError {
        status: StatusCode::RANGE_NOT_SATISFIABLE,
        code: "invalid_range",
        message: "The requested file range is not available.".into(),
        retryable: false,
    };
    let range = value.strip_prefix("bytes=").ok_or_else(invalid)?;
    if range.contains(',') || byte_size == 0 {
        return Err(invalid());
    }
    let (start, end) = range.split_once('-').ok_or_else(invalid)?;
    let (start, end) = if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| invalid())?;
        if suffix == 0 {
            return Err(invalid());
        }
        (
            byte_size.saturating_sub(suffix.min(byte_size)),
            byte_size - 1,
        )
    } else {
        let start = start.parse::<u64>().map_err(|_| invalid())?;
        let end = if end.is_empty() {
            byte_size - 1
        } else {
            end.parse::<u64>()
                .map_err(|_| invalid())?
                .min(byte_size - 1)
        };
        (start, end)
    };
    if start >= byte_size || end < start {
        return Err(invalid());
    }
    Ok((start, end))
}

fn hash_file(path: &FilePath) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

async fn list_roots(State(state): State<ApiState>) -> Result<Json<Vec<CompanionRoot>>, ApiError> {
    let rows = sqlx::query("SELECT r.id, r.display_name, r.status, r.last_scan_at, r.root_kind, COUNT(DISTINCT CASE WHEN m.missing_since IS NULL AND m.trashed_at IS NULL THEN m.id END) model_count FROM library_roots r LEFT JOIN folders f ON f.root_id = r.id LEFT JOIN models m ON m.folder_id = f.id WHERE r.status != 'removed' GROUP BY r.id ORDER BY CASE r.root_kind WHEN 'managed' THEN 0 ELSE 1 END, r.created_at")
        .fetch_all(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    Ok(Json(
        rows.into_iter()
            .map(|row| CompanionRoot {
                id: row.get("id"),
                display_name: row.get("display_name"),
                status: row.get("status"),
                last_scan_at: row.get("last_scan_at"),
                model_count: row.get("model_count"),
                root_kind: row.get("root_kind"),
            })
            .collect(),
    ))
}

async fn sync_manifest(State(state): State<ApiState>) -> Result<Json<SyncManifest>, ApiError> {
    build_sync_manifest(&state, None).await.map(Json)
}

async fn selective_sync_manifest(
    State(state): State<ApiState>,
    Json(request): Json<SelectiveSyncManifestRequest>,
) -> Result<Json<SyncManifest>, ApiError> {
    validate_sync_selection(&request)?;
    let mut project_ids = request.project_ids.into_iter().collect::<HashSet<_>>();
    for folder_id in request.folder_ids {
        project_ids.extend(
            project_ids_for_sync_scope(
                ModelQuery {
                    folder_id: Some(folder_id),
                    ..ModelQuery::default()
                },
                &state.library,
            )
            .await?,
        );
    }
    for collection_id in request.collection_ids {
        project_ids.extend(
            project_ids_for_sync_scope(
                ModelQuery {
                    collection_id: Some(collection_id),
                    ..ModelQuery::default()
                },
                &state.library,
            )
            .await?,
        );
    }
    for project_id in request.excluded_project_ids {
        project_ids.remove(&project_id);
    }
    build_sync_manifest(&state, Some(&project_ids))
        .await
        .map(Json)
}

fn validate_sync_selection(request: &SelectiveSyncManifestRequest) -> Result<(), ApiError> {
    let ids = request
        .project_ids
        .iter()
        .chain(&request.excluded_project_ids)
        .chain(&request.folder_ids)
        .chain(&request.collection_ids)
        .collect::<Vec<_>>();
    if ids.len() > MAX_SYNC_SELECTION_ITEMS
        || request.folder_ids.len() + request.collection_ids.len() > MAX_SYNC_SELECTION_SCOPES
        || ids
            .iter()
            .any(|id| id.trim().is_empty() || id.chars().count() > MAX_SYNC_SELECTION_ID_CHARS)
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request",
            message: "The local-copy selection is invalid or too large.".into(),
            retryable: false,
        });
    }
    Ok(())
}

async fn project_ids_for_sync_scope(
    mut query: ModelQuery,
    state: &AppState,
) -> Result<HashSet<String>, ApiError> {
    query.limit = Some(500);
    query.sort = Some("name".into());
    query.trashed = Some(false);
    let mut ids = HashSet::new();
    loop {
        let page = commands::list_models_impl(query.clone(), state)
            .await
            .map_err(ApiError::from_library)?;
        ids.extend(page.items.into_iter().map(|project| project.id));
        let Some(cursor) = page.next_cursor else {
            break;
        };
        query.cursor = Some(cursor);
    }
    Ok(ids)
}

async fn build_sync_manifest(
    state: &ApiState,
    project_ids: Option<&HashSet<String>>,
) -> Result<SyncManifest, ApiError> {
    let cursor: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) FROM sync_changes")
        .fetch_one(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let selected_json = project_ids
        .map(|ids| serde_json::to_string(&ids.iter().collect::<Vec<_>>()))
        .transpose()
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let project_filter = if project_ids.is_some() {
        " AND ma.model_id IN (SELECT value FROM json_each(?))"
    } else {
        ""
    };
    let statement = format!("SELECT a.id, ma.model_id project_id, m.display_name project_name, a.filename, a.byte_size, a.modified_at, a.content_hash FROM assets a JOIN model_assets ma ON ma.asset_id = a.id JOIN models m ON m.id = ma.model_id WHERE a.missing_since IS NULL AND m.trashed_at IS NULL{project_filter} ORDER BY lower(m.display_name), lower(a.filename), a.id");
    let mut query = sqlx::query(&statement);
    if let Some(selected_json) = selected_json {
        query = query.bind(selected_json);
    }
    let rows = query
        .fetch_all(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let assets = rows
        .into_iter()
        .map(|row| {
            let id: String = row.get("id");
            let project_id: String = row.get("project_id");
            let project_name: String = row.get("project_name");
            let filename: String = row.get("filename");
            SyncAsset {
                mirror_path: mirror_asset_path(&project_name, &project_id, &filename),
                id,
                project_id,
                project_name,
                filename,
                byte_size: row.get("byte_size"),
                modified_at: row.get("modified_at"),
                content_hash: row.get("content_hash"),
            }
        })
        .collect();
    let server_id = state.controller.lock().await.config.server_id.clone();
    Ok(SyncManifest {
        server_id,
        cursor,
        assets,
    })
}

async fn sync_changes(
    State(state): State<ApiState>,
    Query(query): Query<ChangesQuery>,
) -> Result<Json<SyncChanges>, ApiError> {
    if query.after < 0 {
        return Err(ApiError::invalid_mutation("The sync cursor is invalid."));
    }
    let limit = query.limit.unwrap_or(250).clamp(1, 1000);
    let rows = sqlx::query("SELECT sequence, entity_type, entity_id, change_kind, revision, created_at FROM sync_changes WHERE sequence > ? ORDER BY sequence LIMIT ?")
        .bind(query.after)
        .bind(limit + 1)
        .fetch_all(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_library(error.to_string()))?;
    let has_more = rows.len() as i64 > limit;
    let items = rows
        .into_iter()
        .take(limit as usize)
        .map(|row| SyncChange {
            sequence: row.get("sequence"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            change_kind: row.get("change_kind"),
            revision: row.get("revision"),
            created_at: row.get("created_at"),
        })
        .collect::<Vec<_>>();
    let cursor = items
        .last()
        .map(|item| item.sequence)
        .unwrap_or(query.after);
    Ok(Json(SyncChanges {
        items,
        cursor,
        has_more,
    }))
}

fn mirror_asset_path(project_name: &str, project_id: &str, filename: &str) -> String {
    let project = safe_mirror_component(project_name, "Project");
    let suffix = project_id
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    let filename = safe_mirror_component(filename, "asset");
    format!("Projects/{project}--{suffix}/{filename}")
}

fn safe_mirror_component(value: &str, fallback: &str) -> String {
    let cleaned = value
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
        .collect::<String>()
        .trim_matches([' ', '.'])
        .chars()
        .take(160)
        .collect::<String>();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

async fn upload_project_file(
    State(state): State<ApiState>,
    Query(query): Query<UploadQuery>,
    request: Request,
) -> Result<Json<UploadResponse>, ApiError> {
    require_organization_writes(&state).await?;
    let filename = validate_upload_filename(&query.filename)?;
    validate_upload_destination(&query, &state).await?;
    let byte_size = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| ApiError {
            status: StatusCode::LENGTH_REQUIRED,
            code: "upload_size_required",
            message: "The file size is required before uploading.".into(),
            retryable: false,
        })?;
    if byte_size == 0 {
        return Err(ApiError::invalid_upload("Choose a non-empty 3D file."));
    }
    if byte_size > MAX_UPLOAD_BYTES {
        return Err(ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "upload_too_large",
            message: "3D files can be at most 500 MB.".into(),
            retryable: false,
        });
    }
    let _permit = state
        .upload_slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "upload_busy",
            message: "Another Companion upload is being imported. Try again shortly.".into(),
            retryable: true,
        })?;
    let upload_directory = state
        .library
        .data_dir
        .join("companion-uploads")
        .join(Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&upload_directory)
        .await
        .map_err(|error| {
            log::warn!("Could not prepare Companion upload staging: {error}");
            ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "upload_failed",
                message: "Volum Desktop could not prepare this upload.".into(),
                retryable: true,
            }
        })?;
    let upload_path = upload_directory.join(&filename);
    let received = receive_upload(
        request.into_body(),
        &upload_path,
        byte_size,
        MAX_UPLOAD_BYTES,
    )
    .await;
    if let Err(error) = received {
        let _ = tokio::fs::remove_dir_all(&upload_directory).await;
        return Err(error);
    }
    let imported = project_storage::import_companion_upload(
        query.project_id.as_deref(),
        query.project_name.as_deref(),
        &upload_path,
        &state.library,
    )
    .await
    .map_err(ApiError::from_upload_import);
    let _ = tokio::fs::remove_dir_all(&upload_directory).await;
    let imported = imported?;
    let stored_filename: String = sqlx::query_scalar("SELECT filename FROM assets WHERE id = ?")
        .bind(&imported.asset_id)
        .fetch_one(&state.library.pool)
        .await
        .map_err(|error| ApiError::from_upload_import(error.to_string()))?;
    log::info!(
        "Companion imported asset {} into project {} on root {}",
        imported.asset_id,
        imported.project_id,
        imported.root_id
    );
    Ok(Json(UploadResponse {
        project_id: imported.project_id,
        asset_id: imported.asset_id,
        filename: stored_filename,
        byte_size,
    }))
}

async fn validate_upload_destination(
    query: &UploadQuery,
    state: &ApiState,
) -> Result<(), ApiError> {
    match (query.project_id.as_deref(), query.project_name.as_deref()) {
        (Some(project_id), None) => {
            if Uuid::parse_str(project_id).is_err() {
                return Err(ApiError::invalid_upload("The selected project is invalid."));
            }
            let project = sqlx::query("SELECT storage_mode, trashed_at FROM models WHERE id = ?")
                .bind(project_id)
                .fetch_optional(&state.library.pool)
                .await
                .map_err(|error| ApiError::from_upload_import(error.to_string()))?
                .ok_or_else(|| ApiError {
                    status: StatusCode::NOT_FOUND,
                    code: "not_found",
                    message: "The selected project is unavailable.".into(),
                    retryable: false,
                })?;
            if project.get::<Option<String>, _>("trashed_at").is_some()
                || project.get::<String, _>("storage_mode") != "managed"
            {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    code: "upload_target_unavailable",
                    message: "Choose a managed project that is not in Trash.".into(),
                    retryable: false,
                });
            }
        }
        (None, Some(project_name)) => {
            let trimmed = project_name.trim();
            if trimmed.is_empty()
                || trimmed != project_name
                || trimmed.chars().count() > 160
                || trimmed.contains(['/', '\\'])
                || trimmed.chars().any(char::is_control)
            {
                return Err(ApiError::invalid_upload("Enter a valid project name."));
            }
            let managed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM library_roots WHERE root_kind = 'managed' AND status != 'removed')")
                .fetch_one(&state.library.pool)
                .await
                .map_err(|error| ApiError::from_upload_import(error.to_string()))?;
            if !managed {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    code: "managed_library_required",
                    message: "Configure a managed library on Volum Desktop before uploading files."
                        .into(),
                    retryable: false,
                });
            }
        }
        _ => {
            return Err(ApiError::invalid_upload(
                "Choose either an existing project or a name for a new project.",
            ))
        }
    }
    Ok(())
}

async fn receive_upload(
    body: Body,
    destination: &FilePath,
    expected_bytes: u64,
    maximum_bytes: u64,
) -> Result<(), ApiError> {
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "upload_failed",
            message: "Volum Desktop could not save this upload.".into(),
            retryable: true,
        })?;
    let mut stream = body.into_data_stream();
    let mut received = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ApiError::invalid_upload("The upload was interrupted."))?;
        received = received
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| ApiError::invalid_upload("The uploaded file is too large."))?;
        if received > expected_bytes || received > maximum_bytes {
            return Err(ApiError::invalid_upload(
                "The uploaded data did not match the declared file size.",
            ));
        }
        file.write_all(&chunk).await.map_err(|_| ApiError {
            status: StatusCode::INSUFFICIENT_STORAGE,
            code: "upload_write_failed",
            message: "Volum Desktop could not finish saving this upload.".into(),
            retryable: true,
        })?;
    }
    if received != expected_bytes {
        return Err(ApiError::invalid_upload(
            "The upload ended before the complete file arrived.",
        ));
    }
    file.flush().await.map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "upload_write_failed",
        message: "Volum Desktop could not finish saving this upload.".into(),
        retryable: true,
    })?;
    Ok(())
}

fn validate_upload_filename(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    let path = FilePath::new(trimmed);
    let is_single_component = path
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        && path
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty());
    if trimmed.is_empty()
        || trimmed != value
        || trimmed.chars().count() > MAX_UPLOAD_FILENAME_CHARS
        || trimmed.chars().any(char::is_control)
        || !is_single_component
        || trimmed.contains(['<', '>', ':', '"', '|', '?', '*'])
    {
        return Err(ApiError::invalid_upload("The filename is not valid."));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !SUPPORTED_UPLOAD_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ApiError::invalid_upload(
            "Choose an STL, 3MF, OBJ, STEP, or STP file.",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_photo_filename(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    let path = FilePath::new(trimmed);
    let is_single_component = path
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        && path
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty());
    if trimmed.is_empty()
        || trimmed != value
        || trimmed.chars().count() > MAX_UPLOAD_FILENAME_CHARS
        || trimmed.chars().any(char::is_control)
        || !is_single_component
        || trimmed.contains(['<', '>', ':', '"', '|', '?', '*'])
    {
        return Err(ApiError::invalid_upload("The photo filename is not valid."));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !SUPPORTED_PHOTO_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ApiError::invalid_upload(
            "Choose a JPEG, PNG, or WebP photo.",
        ));
    }
    Ok(trimmed.to_string())
}

async fn list_folders(
    State(state): State<ApiState>,
    Query(query): Query<FolderQuery>,
) -> Result<Json<Vec<CompanionFolder>>, ApiError> {
    let folders = commands::list_folders_impl(query.root_id, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    Ok(Json(
        folders
            .into_iter()
            .map(|folder| CompanionFolder {
                id: folder.id,
                root_id: folder.root_id,
                parent_id: folder.parent_id,
                relative_path: folder.relative_path,
                name: folder.name,
                model_count: folder.model_count,
            })
            .collect(),
    ))
}

async fn duplicate_stats(
    State(state): State<ApiState>,
) -> Result<Json<crate::domain::DuplicateStats>, ApiError> {
    commands::get_duplicate_stats_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn duplicate_groups(
    State(state): State<ApiState>,
    Query(query): Query<DuplicateQuery>,
) -> Result<Json<Page<crate::domain::DuplicateGroup>>, ApiError> {
    commands::list_duplicate_groups_impl(
        query.offset.unwrap_or(0),
        query.limit.unwrap_or(48),
        &state.library,
    )
    .await
    .map(Json)
    .map_err(ApiError::from_library)
}

async fn list_web_sources(State(state): State<ApiState>) -> Result<Json<Vec<WebSource>>, ApiError> {
    crate::web_sources::list_web_sources_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn list_collections(
    State(state): State<ApiState>,
) -> Result<Json<Vec<Collection>>, ApiError> {
    commands::list_collections_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn create_collection(
    State(state): State<ApiState>,
    Json(input): Json<CollectionInput>,
) -> Result<Json<Collection>, ApiError> {
    require_organization_writes(&state).await?;
    commands::create_collection_impl(input, &state.library)
        .await
        .map(Json)
        .map_err(collection_write_error)
}

async fn update_collection(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(input): Json<CollectionInput>,
) -> Result<Json<Collection>, ApiError> {
    require_organization_writes(&state).await?;
    commands::update_collection_impl(&id, input, &state.library)
        .await
        .map(Json)
        .map_err(collection_write_error)
}

async fn delete_collection(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    commands::delete_collection_impl(&id, &state.library)
        .await
        .map_err(collection_write_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_project_to_collection(
    State(state): State<ApiState>,
    Path((collection_id, project_id)): Path<(String, String)>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    require_organization_writes(&state).await?;
    commands::set_models_collection_impl(
        &collection_id,
        std::slice::from_ref(&project_id),
        true,
        &state.library,
    )
    .await
    .map_err(collection_write_error)?;
    load_project_detail(&project_id, &state).await.map(Json)
}

async fn remove_project_from_collection(
    State(state): State<ApiState>,
    Path((collection_id, project_id)): Path<(String, String)>,
) -> Result<Json<CompanionProjectDetail>, ApiError> {
    require_organization_writes(&state).await?;
    commands::set_models_collection_impl(
        &collection_id,
        std::slice::from_ref(&project_id),
        false,
        &state.library,
    )
    .await
    .map_err(collection_write_error)?;
    load_project_detail(&project_id, &state).await.map(Json)
}

fn collection_write_error(error: String) -> ApiError {
    if error.contains("already exists") {
        ApiError {
            status: StatusCode::CONFLICT,
            code: "collection_exists",
            message: error,
            retryable: false,
        }
    } else if error.contains("not found") {
        ApiError::from_library(error)
    } else {
        ApiError::invalid_mutation(error)
    }
}

async fn list_tags(State(state): State<ApiState>) -> Result<Json<Vec<Tag>>, ApiError> {
    commands::list_tags_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn create_tag(
    State(state): State<ApiState>,
    Json(request): Json<CreateTagRequest>,
) -> Result<Json<Tag>, ApiError> {
    require_organization_writes(&state).await?;
    commands::save_tag_impl(
        TagInput {
            id: None,
            name: request.name,
            color: request.color,
        },
        &state.library,
    )
    .await
    .map(Json)
    .map_err(tag_write_error)
}

async fn update_tag(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<CreateTagRequest>,
) -> Result<Json<Tag>, ApiError> {
    require_organization_writes(&state).await?;
    commands::save_tag_impl(
        TagInput {
            id: Some(id),
            name: request.name,
            color: request.color,
        },
        &state.library,
    )
    .await
    .map(Json)
    .map_err(tag_write_error)
}

async fn delete_tag(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    commands::delete_tag_impl(&id, &state.library)
        .await
        .map_err(tag_write_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn tag_write_error(error: String) -> ApiError {
    if error.contains("already exists") {
        ApiError {
            status: StatusCode::CONFLICT,
            code: "tag_exists",
            message: error,
            retryable: false,
        }
    } else if error.contains("not found") {
        ApiError::from_library(error)
    } else {
        ApiError::invalid_mutation(error)
    }
}

async fn list_saved_searches(
    State(state): State<ApiState>,
) -> Result<Json<Vec<SavedSearch>>, ApiError> {
    commands::list_saved_searches_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn save_saved_search(
    State(state): State<ApiState>,
    Json(input): Json<SavedSearchInput>,
) -> Result<Json<SavedSearch>, ApiError> {
    require_organization_writes(&state).await?;
    commands::save_saved_search_impl(input, &state.library)
        .await
        .map(Json)
        .map_err(ApiError::invalid_mutation)
}

async fn delete_saved_search(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    commands::delete_saved_search_impl(&id, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_custom_fields(
    State(state): State<ApiState>,
) -> Result<Json<Vec<crate::domain::CustomFieldDefinition>>, ApiError> {
    project_docs::custom_field_definitions(&state.library.pool)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn save_custom_field(
    State(state): State<ApiState>,
    Json(input): Json<project_docs::CustomFieldInput>,
) -> Result<Json<crate::domain::CustomFieldDefinition>, ApiError> {
    require_organization_writes(&state).await?;
    project_docs::save_custom_field_definition_impl(input, &state.library)
        .await
        .map(Json)
        .map_err(ApiError::invalid_mutation)
}

async fn delete_custom_field(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_organization_writes(&state).await?;
    project_docs::delete_custom_field_definition_impl(&id, &state.library)
        .await
        .map_err(ApiError::from_library)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_materials(State(state): State<ApiState>) -> Result<Json<Vec<Material>>, ApiError> {
    commands::list_materials_impl(&state.library)
        .await
        .map(Json)
        .map_err(ApiError::from_library)
}

async fn save_material(
    State(state): State<ApiState>,
    Json(input): Json<MaterialInput>,
) -> Result<Json<Material>, ApiError> {
    require_organization_writes(&state).await?;
    commands::save_material_impl(input, &state.library)
        .await
        .map(Json)
        .map_err(ApiError::invalid_mutation)
}

fn binary_response(
    bytes: Vec<u8>,
    mime_type: &str,
    request_headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let mut headers = HeaderMap::new();
    let content_type = HeaderValue::from_str(mime_type).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "invalid_media",
        message: "Volum could not return this media item.".into(),
        retryable: false,
    })?;
    headers.insert(header::CONTENT_TYPE, content_type);
    let etag = format!("\"{}\"", blake3::hash(&bytes).to_hex());
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "invalid_media",
            message: "Volum could not identify this media item.".into(),
            retryable: false,
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=0, must-revalidate"),
    );
    if request_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|candidate| {
                let candidate = candidate.trim();
                candidate == "*" || candidate == etag || candidate.strip_prefix("W/") == Some(&etag)
            })
        })
    {
        return Ok((StatusCode::NOT_MODIFIED, headers, Body::empty()).into_response());
    }
    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}

struct TailnetInfo {
    ip: String,
    dns_name: Option<String>,
}

fn discover_tailnet() -> CompanionResult<TailnetInfo> {
    let executable = [
        "tailscale",
        "/opt/homebrew/bin/tailscale",
        "/usr/local/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    ]
    .into_iter()
    .find(|candidate| {
        Command::new(candidate)
            .args(["status", "--json"])
            .output()
            .is_ok_and(|output| output.status.success())
    })
    .ok_or_else(|| "Tailscale is not running or its CLI is unavailable".to_string())?;
    let ip_output = Command::new(executable)
        .args(["ip", "-4"])
        .output()
        .map_err(|error| format!("Could not ask Tailscale for an address: {error}"))?;
    if !ip_output.status.success() {
        return Err("Tailscale is not connected".into());
    }
    let ip = String::from_utf8_lossy(&ip_output.stdout)
        .trim()
        .to_string();
    if ip.is_empty() {
        return Err("Tailscale did not provide an IPv4 address".into());
    }
    let status = Command::new(executable)
        .args(["status", "--json"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok());
    let dns_name = status
        .as_ref()
        .and_then(|value| value.pointer("/Self/DNSName"))
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim_end_matches('.').to_string());
    Ok(TailnetInfo { ip, dns_name })
}

fn default_network_mode() -> String {
    "automatic".into()
}

fn is_private_transport_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

fn parse_private_bind_address(value: &str) -> CompanionResult<IpAddr> {
    let ip = value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .map_err(|_| "The listener address must be an IP address on this computer".to_string())?;
    if !is_private_transport_ip(ip) {
        return Err("Companion Access can listen only on a private network address".into());
    }
    Ok(ip)
}

async fn validate_public_url(value: &str) -> CompanionResult<String> {
    let normalized = normalize_public_url(value)?;
    let url = reqwest::Url::parse(&normalized).map_err(|error| error.to_string())?;
    if url.scheme() == "https" {
        return Ok(normalized);
    }
    let host = url.host_str().unwrap_or_default();
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_transport_ip(ip) {
            return Ok(normalized);
        }
        return Err("Plain HTTP is allowed only on private network addresses".into());
    }
    let port = url.port_or_known_default().unwrap_or(DEFAULT_PORT);
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "The Companion Access address could not be resolved".to_string())?
        .collect::<Vec<_>>();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !is_private_transport_ip(address.ip()))
    {
        return Err(
            "Plain HTTP is allowed only when the server name resolves to a private address".into(),
        );
    }
    Ok(normalized)
}

fn transport_for_url(value: &str) -> &'static str {
    if value.starts_with("https://") {
        "https"
    } else if reqwest::Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .is_some_and(|host| {
            host.ends_with(".ts.net")
                || host.parse::<IpAddr>().is_ok_and(|ip| match ip {
                    IpAddr::V4(ip) => {
                        let octets = ip.octets();
                        octets[0] == 100 && (64..=127).contains(&octets[1])
                    }
                    IpAddr::V6(ip) => {
                        let segments = ip.segments();
                        segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
                    }
                })
        })
    {
        "tailscale-http"
    } else {
        "private-http"
    }
}

fn load_config(path: &FilePath) -> CompanionResult<CompanionConfig> {
    if !path.exists() {
        let config = CompanionConfig {
            enabled: false,
            server_id: Uuid::new_v4().to_string(),
            port: DEFAULT_PORT,
            devices: Vec::new(),
            organization_writes: false,
            photo_uploads: Some(false),
            network_mode: default_network_mode(),
            bind_address: None,
            public_url: None,
        };
        persist_config(path, &config)?;
        return Ok(config);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read Companion Access settings: {error}"))?;
    let mut config: CompanionConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Companion Access settings are damaged: {error}"))?;
    let mut changed = false;
    if config.server_id.is_empty() {
        config.server_id = Uuid::new_v4().to_string();
        changed = true;
    }
    if config.port == 0 {
        config.port = DEFAULT_PORT;
        changed = true;
    }
    if config.photo_uploads.is_none() {
        config.photo_uploads = Some(config.organization_writes);
        changed = true;
    }
    if changed {
        persist_config(path, &config)?;
    }
    Ok(config)
}

fn persist_config(path: &FilePath, config: &CompanionConfig) -> CompanionResult<()> {
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not save Companion Access settings: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&temporary, permissions)
            .map_err(|error| format!("Could not protect Companion Access settings: {error}"))?;
    }
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("Could not replace Companion Access settings: {error}"))?;
    }
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("Could not activate Companion Access settings: {error}"))?;
    Ok(())
}

fn create_pairing_locked(
    inner: &mut ControllerInner,
    role: &str,
) -> CompanionResult<CompanionPairingInfo> {
    let base_url = inner
        .base_url
        .clone()
        .ok_or_else(|| "Companion Access is not running".to_string())?;
    let secret = random_secret();
    let expires_at = Utc::now() + chrono::Duration::seconds(PAIRING_LIFETIME_SECONDS);
    let server_name = server_name(inner);
    let transport = transport_for_url(&base_url);
    let payload = serde_json::json!({
        "version": PROTOCOL_VERSION,
        "serverId": inner.config.server_id,
        "serverName": server_name,
        "baseUrl": base_url,
        "secret": secret,
        "expiresAt": expires_at.to_rfc3339(),
        "transport": transport,
        "purpose": if role == "migration" { "library-migration" } else { "client" }
    });
    inner.pairing = Some(PairingSession {
        secret: secret.clone(),
        expires_at,
        role: role.to_string(),
    });
    Ok(CompanionPairingInfo {
        server_id: inner.config.server_id.clone(),
        server_name,
        base_url,
        secret,
        expires_at: expires_at.to_rfc3339(),
        transport: transport.to_string(),
        qr_payload: payload.to_string(),
    })
}

fn random_secret() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

fn public_device(device: &DeviceRecord) -> CompanionDevice {
    CompanionDevice {
        id: device.id.clone(),
        name: device.name.clone(),
        platform: device.platform.clone(),
        paired_at: device.paired_at.clone(),
        last_seen_at: device.last_seen_at.clone(),
        role: device.role.clone(),
    }
}

fn default_device_role() -> String {
    "client".into()
}

fn server_name(inner: &ControllerInner) -> String {
    inner
        .dns_name
        .as_deref()
        .and_then(|name| name.split('.').next())
        .unwrap_or(if inner.headless {
            "Volum Server"
        } else {
            "Volum Desktop"
        })
        .to_string()
}

fn format_base_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    }
}

fn normalize_public_url(value: &str) -> CompanionResult<String> {
    let value = value.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(value)
        .map_err(|_| "VOLUM_PUBLIC_URL must be a complete http:// or https:// URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("VOLUM_PUBLIC_URL must contain only an http(s) origin".into());
    }
    Ok(url.origin().ascii_serialization())
}

const fn default_port() -> u16 {
    DEFAULT_PORT
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[test]
    fn network_asset_does_not_serialize_relative_path() {
        let asset = CompanionAsset {
            id: "asset".into(),
            filename: "part.stl".into(),
            extension: "stl".into(),
            byte_size: 42,
            modified_at: "2026-01-01T00:00:00Z".into(),
            parse_status: "ready".into(),
            metadata: AssetMetadata::default(),
            missing: false,
            role: "printable".into(),
        };
        let value = serde_json::to_value(asset).unwrap();
        assert!(value.get("relativePath").is_none());
        assert!(value.get("rootPath").is_none());
    }

    #[test]
    fn tokens_are_stored_as_hashes() {
        let token = random_secret();
        let hash = token_hash(&token);
        assert_ne!(token, hash);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn custom_listeners_accept_only_private_addresses() {
        assert!(parse_private_bind_address("192.168.1.20").is_ok());
        assert!(parse_private_bind_address("10.8.0.2").is_ok());
        assert!(parse_private_bind_address("100.64.0.8").is_ok());
        assert!(parse_private_bind_address("fd00::20").is_ok());
        assert!(parse_private_bind_address("203.0.113.8").is_err());
        assert!(parse_private_bind_address("0.0.0.0").is_err());
    }

    #[test]
    fn pairing_transport_preserves_tailscale_and_classifies_new_routes() {
        assert_eq!(
            transport_for_url("http://100.64.0.8:43821"),
            "tailscale-http"
        );
        assert_eq!(
            transport_for_url("http://volum.example.ts.net:43821"),
            "tailscale-http"
        );
        assert_eq!(
            transport_for_url("http://192.168.1.20:43821"),
            "private-http"
        );
        assert_eq!(transport_for_url("https://volum.example.com"), "https");
    }

    #[test]
    fn media_etags_are_stable_and_support_revalidation() {
        let response = binary_response(b"media".to_vec(), "image/png", &HeaderMap::new()).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let etag = response.headers().get(header::ETAG).unwrap().clone();
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "private, max-age=0, must-revalidate"
        );

        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::IF_NONE_MATCH, etag.clone());
        let not_modified =
            binary_response(b"media".to_vec(), "image/png", &request_headers).unwrap();
        assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(not_modified.headers().get(header::ETAG), Some(&etag));

        let changed = binary_response(b"changed".to_vec(), "image/png", &request_headers).unwrap();
        assert_eq!(changed.status(), StatusCode::OK);
        assert_ne!(changed.headers().get(header::ETAG), Some(&etag));
    }

    #[test]
    fn upload_filenames_are_basenames_with_supported_extensions() {
        assert_eq!(
            validate_upload_filename("phone bracket.3MF").unwrap(),
            "phone bracket.3MF"
        );
        assert!(validate_upload_filename("../private/model.stl").is_err());
        assert!(validate_upload_filename("folder/model.stl").is_err());
        assert!(validate_upload_filename("model.zip").is_err());
        assert!(validate_upload_filename("model?.stl").is_err());
    }

    #[test]
    fn photo_upload_filenames_are_basenames_with_supported_extensions() {
        assert_eq!(
            validate_photo_filename("workshop photo.JPEG").unwrap(),
            "workshop photo.JPEG"
        );
        assert!(validate_photo_filename("../private/photo.jpg").is_err());
        assert!(validate_photo_filename("photo.gif").is_err());
        assert!(validate_photo_filename("photo?.png").is_err());
    }

    #[test]
    fn byte_ranges_support_resume_and_reject_ambiguous_requests() {
        assert_eq!(parse_byte_range("bytes=0-3", 10).unwrap(), (0, 3));
        assert_eq!(parse_byte_range("bytes=4-", 10).unwrap(), (4, 9));
        assert_eq!(parse_byte_range("bytes=-3", 10).unwrap(), (7, 9));
        assert_eq!(parse_byte_range("bytes=8-99", 10).unwrap(), (8, 9));
        assert_eq!(parse_byte_range("bytes=-99", 10).unwrap(), (0, 9));
        assert!(parse_byte_range("bytes=10-", 10).is_err());
        assert!(parse_byte_range("bytes=4-2", 10).is_err());
        assert!(parse_byte_range("bytes=0-1,4-5", 10).is_err());
        assert!(parse_byte_range("items=0-1", 10).is_err());
        assert!(parse_byte_range("bytes=0-", 0).is_err());
    }

    #[test]
    fn mirror_paths_are_relative_and_remove_unsafe_components() {
        let path = mirror_asset_path("../Lamp: shade", "project-123456789", "../../part?.stl");
        assert_eq!(path, "Projects/_Lamp_ shade--project1/_.._part_.stl");
        assert!(!path.starts_with('/'));
        assert!(!path.split('/').any(|component| component == ".."));
        assert_eq!(safe_mirror_component("...", "Project"), "Project");
    }

    async fn mutation_test_state() -> (tempfile::TempDir, ApiState) {
        let directory = tempfile::tempdir().unwrap();
        let library = AppState::initialize(directory.path()).await.unwrap();
        let timestamp = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, created_at, updated_at) VALUES ('root', ?, 'Library', 'online', ?, ?)")
            .bind(directory.path().join("library").to_string_lossy().to_string())
            .bind(timestamp)
            .bind(timestamp)
            .execute(&library.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO folders (id, root_id, relative_path, name, status) VALUES ('folder', 'root', '', 'Library', 'available')")
            .execute(&library.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, added_at, updated_at) VALUES ('project', 'folder', 'Phone stand', 'phone-stand', ?, ?)")
            .bind(timestamp)
            .bind(timestamp)
            .execute(&library.pool)
            .await
            .unwrap();
        let inner = ControllerInner {
            config: CompanionConfig {
                enabled: false,
                server_id: "server".into(),
                port: DEFAULT_PORT,
                devices: Vec::new(),
                organization_writes: true,
                photo_uploads: Some(true),
                network_mode: default_network_mode(),
                bind_address: None,
                public_url: None,
            },
            config_path: directory.path().join("companion.json"),
            pairing: None,
            server: None,
            tailnet_ip: None,
            dns_name: None,
            base_url: None,
            last_error: None,
            headless: false,
        };
        let state = ApiState {
            library,
            app: None,
            controller: Arc::new(Mutex::new(inner)),
            upload_slot: Arc::new(Semaphore::new(1)),
            viewer_slot: Arc::new(Semaphore::new(1)),
            migration_lock: Arc::new(Mutex::new(())),
            web_admin: None,
        };
        (directory, state)
    }

    async fn authenticated_test_state() -> (tempfile::TempDir, ApiState, String) {
        let (directory, state) = mutation_test_state().await;
        let token = random_secret();
        state
            .controller
            .lock()
            .await
            .config
            .devices
            .push(DeviceRecord {
                id: "device".into(),
                name: "Workshop phone".into(),
                platform: "test".into(),
                token_hash: token_hash(&token),
                paired_at: "2026-01-01T00:00:00Z".into(),
                last_seen_at: None,
                role: "client".into(),
                migration_id: None,
            });
        (directory, state, token)
    }

    #[tokio::test]
    async fn health_and_readiness_are_distinct_and_propagate_request_ids() {
        let (_directory, state) = mutation_test_state().await;
        let router = companion_router(state, None);

        let health = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .header("x-request-id", "health-check-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(health.headers()["x-request-id"], "health-check-1");

        let ready = router
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::OK);
        assert!(ready.headers().contains_key("x-request-id"));
        let body = axum::body::to_bytes(ready.into_body(), 64).await.unwrap();
        assert_eq!(&body[..], b"ready");
    }

    #[tokio::test]
    async fn committed_openapi_operations_resolve_to_real_routes() {
        let (_directory, mut state) = mutation_test_state().await;
        state.web_admin = Some(crate::web_admin::WebAdminState::for_test(
            "correct horse battery staple",
        ));
        let router = companion_router(state, None);
        let contract_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/companion-api/openapi.json");
        let contract: serde_json::Value =
            serde_json::from_slice(&std::fs::read(contract_path).unwrap()).unwrap();

        for (path, operations) in contract["paths"].as_object().unwrap() {
            let concrete_path = path
                .replace("{collection_id}", "collection")
                .replace("{project_id}", "project")
                .replace("{plate_index}", "0")
                .replace("{photo_id}", "photo")
                .replace("{field_id}", "field")
                .replace("{item_id}", "item")
                .replace("{step_id}", "step")
                .replace("{file_id}", "file")
                .replace("{id}", "resource");
            for method in operations.as_object().unwrap().keys() {
                if method == "parameters" {
                    continue;
                }
                let request = Request::builder()
                    .method(method.to_ascii_uppercase().as_str())
                    .uri(&concrete_path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap();
                let response = router.clone().oneshot(request).await.unwrap();
                assert_ne!(
                    response.status(),
                    StatusCode::NOT_FOUND,
                    "OpenAPI operation {method} {path} has no Axum route"
                );
                assert_ne!(
                    response.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "OpenAPI operation {method} {path} uses an unsupported method"
                );
            }
        }
    }

    #[tokio::test]
    async fn web_administrator_can_create_pairings_and_clients_cannot() {
        let (_directory, mut state) = mutation_test_state().await;
        state.web_admin = Some(crate::web_admin::WebAdminState::for_test(
            "correct horse battery staple",
        ));
        {
            let mut inner = state.controller.lock().await;
            inner.base_url = Some("http://192.168.1.20:43821".into());
            inner.headless = true;
        }
        let router = companion_router(state.clone(), None);
        let mut login_request = Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/v1/web/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "password": "correct horse battery staple",
                    "deviceName": "Test browser"
                })
                .to_string(),
            ))
            .unwrap();
        login_request.extensions_mut().insert(ConnectInfo(
            "192.168.1.12:54321".parse::<SocketAddr>().unwrap(),
        ));
        let login_response = router.clone().oneshot(login_request).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        assert_eq!(login_response.headers()[header::CACHE_CONTROL], "no-store");
        let login_bytes = axum::body::to_bytes(login_response.into_body(), 32 * 1024)
            .await
            .unwrap();
        let login: serde_json::Value = serde_json::from_slice(&login_bytes).unwrap();
        let admin_token = login["token"].as_str().unwrap();

        let pairing_response = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/admin/pairings",
                admin_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(pairing_response.status(), StatusCode::OK);
        let pairing_bytes = axum::body::to_bytes(pairing_response.into_body(), 32 * 1024)
            .await
            .unwrap();
        let pairing: serde_json::Value = serde_json::from_slice(&pairing_bytes).unwrap();
        assert_eq!(pairing["serverName"], "Volum Server");
        assert!(pairing["qrPayload"].as_str().unwrap().contains("secret"));

        let migration_pairing_response = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/admin/migration-pairings",
                admin_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(migration_pairing_response.status(), StatusCode::OK);
        let migration_pairing_bytes =
            axum::body::to_bytes(migration_pairing_response.into_body(), 32 * 1024)
                .await
                .unwrap();
        let migration_pairing: serde_json::Value =
            serde_json::from_slice(&migration_pairing_bytes).unwrap();
        let migration_payload: serde_json::Value =
            serde_json::from_str(migration_pairing["qrPayload"].as_str().unwrap()).unwrap();
        assert_eq!(migration_payload["purpose"], "library-migration");
        let pair_migration = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::POST)
                    .uri("/pair")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "secret": migration_payload["secret"],
                            "deviceName": "Migrating Desktop",
                            "platform": "desktop"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(pair_migration.status(), StatusCode::OK);
        assert!(state
            .controller
            .lock()
            .await
            .config
            .devices
            .iter()
            .any(|device| device.name == "Migrating Desktop" && device.role == "migration"));

        let client_token = random_secret();
        state
            .controller
            .lock()
            .await
            .config
            .devices
            .push(DeviceRecord {
                id: "client".into(),
                name: "Phone".into(),
                platform: "iOS".into(),
                token_hash: token_hash(&client_token),
                paired_at: Utc::now().to_rfc3339(),
                last_seen_at: None,
                role: "client".into(),
                migration_id: None,
            });
        let forbidden = router
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/admin/pairings",
                &client_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    }

    fn authenticated_request(
        method: axum::http::Method,
        uri: &str,
        token: &str,
        body: Body,
    ) -> Request {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .unwrap()
    }

    async fn response_error_code(response: Response) -> String {
        response_json(response).await["error"]["code"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn client_tokens_cannot_use_the_migration_api() {
        let (_directory, state, token) = authenticated_test_state().await;
        state.controller.lock().await.headless = true;
        let response = companion_router(state, None)
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/migrations/preflight",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_error_code(response).await,
            "migration_token_required"
        );
    }

    #[tokio::test]
    async fn migration_tokens_cannot_use_the_normal_api() {
        let (_directory, state) = mutation_test_state().await;
        let token = random_secret();
        {
            let mut inner = state.controller.lock().await;
            inner.headless = true;
            inner.config.devices.push(DeviceRecord {
                id: "migration-device".into(),
                name: "Migrating Desktop".into(),
                platform: "desktop".into(),
                token_hash: token_hash(&token),
                paired_at: Utc::now().to_rfc3339(),
                last_seen_at: None,
                role: "migration".into(),
                migration_id: None,
            });
        }
        let response = companion_router(state, None)
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response_error_code(response).await, "migration_scope_only");
    }

    #[tokio::test]
    async fn creating_a_migration_job_binds_it_to_the_temporary_token() {
        let (directory, state) = mutation_test_state().await;
        sqlx::query("DELETE FROM models")
            .execute(&state.library.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE library_roots SET root_kind = 'managed'")
            .execute(&state.library.pool)
            .await
            .unwrap();
        std::fs::create_dir_all(directory.path().join("library")).unwrap();
        let token = random_secret();
        let other_token = random_secret();
        {
            let mut inner = state.controller.lock().await;
            inner.headless = true;
            for (id, value) in [("migration-device", &token), ("other-device", &other_token)] {
                inner.config.devices.push(DeviceRecord {
                    id: id.into(),
                    name: id.into(),
                    platform: "desktop".into(),
                    token_hash: token_hash(value),
                    paired_at: Utc::now().to_rfc3339(),
                    last_seen_at: None,
                    role: "migration".into(),
                    migration_id: None,
                });
            }
        }
        let schema_version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_one(&state.library.pool)
                .await
                .unwrap();
        let migration_id = Uuid::new_v4().to_string();
        let manifest = serde_json::json!({
            "formatVersion": library_migration::MIGRATION_FORMAT_VERSION,
            "id": migration_id,
            "createdAt": Utc::now().to_rfc3339(),
            "schemaVersion": schema_version,
            "allowMissing": false,
            "stats": {
                "projects": 0,
                "files": 0,
                "photos": 0,
                "collections": 0,
                "tags": 0,
                "linkedProjects": 0,
                "managedProjects": 0,
                "missingFiles": 0,
                "totalBytes": 0
            },
            "files": [{
                "id": "database",
                "kind": "database",
                "relativePath": "database/volum.sqlite3",
                "byteSize": 0,
                "blake3": "0".repeat(64),
                "missing": false
            }]
        });
        let router = companion_router(state.clone(), None);
        let created = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/migrations",
                &token,
                Body::from(manifest.to_string()),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        assert!(state
            .controller
            .lock()
            .await
            .config
            .devices
            .iter()
            .any(|device| device.token_hash == token_hash(&token)
                && device.migration_id.as_deref() == Some(migration_id.as_str())));

        let unrelated = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                &format!("/api/v1/migrations/{migration_id}"),
                &other_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(unrelated.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_error_code(unrelated).await,
            "migration_job_forbidden"
        );

        let cancelled = router
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                &format!("/api/v1/migrations/{migration_id}/cancel"),
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(cancelled.status(), StatusCode::OK);
        let cancelled_body = response_json(cancelled).await;
        assert_eq!(cancelled_body["state"], "cancelled");
    }

    #[tokio::test]
    async fn finishing_a_complete_migration_promotes_its_token_to_client() {
        let (_directory, state) = mutation_test_state().await;
        let token = random_secret();
        let other_token = random_secret();
        let migration_id = Uuid::new_v4().to_string();
        {
            let mut inner = state.controller.lock().await;
            inner.headless = true;
            inner.config.devices.push(DeviceRecord {
                id: "migration-device".into(),
                name: "Migrating Desktop".into(),
                platform: "desktop".into(),
                token_hash: token_hash(&token),
                paired_at: Utc::now().to_rfc3339(),
                last_seen_at: None,
                role: "migration".into(),
                migration_id: Some(migration_id.clone()),
            });
            inner.config.devices.push(DeviceRecord {
                id: "other-migration-device".into(),
                name: "Another Desktop".into(),
                platform: "desktop".into(),
                token_hash: token_hash(&other_token),
                paired_at: Utc::now().to_rfc3339(),
                last_seen_at: None,
                role: "migration".into(),
                migration_id: None,
            });
        }
        let job_directory = state
            .library
            .data_dir
            .join("library-migrations")
            .join(&migration_id);
        std::fs::create_dir_all(&job_directory).unwrap();
        let timestamp = Utc::now().to_rfc3339();
        std::fs::write(
            job_directory.join("job.json"),
            serde_json::to_vec(&serde_json::json!({
                "manifest": {
                    "formatVersion": 1,
                    "id": migration_id,
                    "createdAt": timestamp,
                    "schemaVersion": 1,
                    "allowMissing": false,
                    "stats": {
                        "projects": 0,
                        "files": 0,
                        "photos": 0,
                        "collections": 0,
                        "tags": 0,
                        "linkedProjects": 0,
                        "managedProjects": 0,
                        "missingFiles": 0,
                        "totalBytes": 0
                    },
                    "files": []
                },
                "state": "complete",
                "targetLibraryPath": state.library.data_dir.join("library"),
                "createdAt": timestamp,
                "updatedAt": timestamp,
                "completedAt": timestamp,
                "error": null
            }))
            .unwrap(),
        )
        .unwrap();

        let router = companion_router(state.clone(), None);
        let unrelated_finish = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                &format!("/api/v1/migrations/{migration_id}/finish"),
                &other_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(unrelated_finish.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_error_code(unrelated_finish).await,
            "migration_job_forbidden"
        );

        let finish = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                &format!("/api/v1/migrations/{migration_id}/finish"),
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(finish.status(), StatusCode::OK);
        let promoted_role = state
            .controller
            .lock()
            .await
            .config
            .devices
            .iter()
            .find(|device| device.id == "migration-device")
            .unwrap()
            .role
            .clone();
        assert_eq!(promoted_role, "client");

        let completion_proof = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                &format!("/api/v1/migrations/{migration_id}"),
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(completion_proof.status(), StatusCode::OK);
        let repeated_finish = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                &format!("/api/v1/migrations/{migration_id}/finish"),
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(repeated_finish.status(), StatusCode::OK);

        let normal_api = router
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(normal_api.status(), StatusCode::OK);
    }

    async fn add_test_asset(directory: &tempfile::TempDir, state: &ApiState) -> Vec<u8> {
        let bytes = b"0123456789abcdef".to_vec();
        let library = directory.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::write(library.join("part.stl"), &bytes).unwrap();
        sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at, parse_status, metadata_json) VALUES ('asset', 'root', 'folder', 'part.stl', 'part.stl', 'stl', ?, '2026-01-01T00:00:00Z', 'ready', '{}')")
            .bind(bytes.len() as i64)
            .execute(&state.library.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO model_assets (model_id, asset_id, role, sort_order) VALUES ('project', 'asset', 'printable', 0)")
            .execute(&state.library.pool)
            .await
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn sync_manifest_never_exposes_server_paths() {
        let (directory, state, token) = authenticated_test_state().await;
        add_test_asset(&directory, &state).await;
        let response = companion_router(state, None)
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/sync/manifest",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(!text.contains(directory.path().to_string_lossy().as_ref()));
        let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            manifest["assets"][0]["mirrorPath"],
            "Projects/Phone stand--project/part.stl"
        );
        assert!(manifest["assets"][0].get("relativePath").is_none());
    }

    #[tokio::test]
    async fn selective_sync_manifest_applies_project_choices_and_exclusions() {
        let (directory, state, token) = authenticated_test_state().await;
        add_test_asset(&directory, &state).await;
        state.controller.lock().await.config.organization_writes = false;
        let router = companion_router(state, None);
        let selected = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/sync/manifest",
                &token,
                Body::from(r#"{"projectIds":["project"]}"#),
            ))
            .await
            .unwrap();
        assert_eq!(selected.status(), StatusCode::OK);
        let body = axum::body::to_bytes(selected.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(manifest["assets"].as_array().unwrap().len(), 1);

        let excluded = router
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/sync/manifest",
                &token,
                Body::from(r#"{"projectIds":["project"],"excludedProjectIds":["project"]}"#),
            ))
            .await
            .unwrap();
        assert_eq!(excluded.status(), StatusCode::OK);
        let body = axum::body::to_bytes(excluded.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(manifest["assets"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn selective_sync_scopes_follow_nested_folders_and_smart_collections() {
        let (_directory, state) = mutation_test_state().await;
        let timestamp = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO folders (id, root_id, parent_id, relative_path, name, status) VALUES ('parent', 'root', 'folder', 'Parent', 'Parent', 'available'), ('child', 'root', 'parent', 'Parent/Child', 'Child', 'available')")
            .execute(&state.library.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, favorite, added_at, updated_at) VALUES ('nested', 'child', 'Nested project', 'nested', 1, ?, ?)")
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.library.pool)
            .await
            .unwrap();
        let folder_projects = project_ids_for_sync_scope(
            ModelQuery {
                folder_id: Some("parent".into()),
                ..ModelQuery::default()
            },
            &state.library,
        )
        .await
        .unwrap();
        assert_eq!(folder_projects, HashSet::from(["nested".to_string()]));

        let rule = serde_json::json!({
            "version": 2,
            "matchMode": "all",
            "rules": [{ "field": "favorite", "operator": "is", "value": true }]
        });
        sqlx::query("INSERT INTO collections (id, name, symbol, color, sort_order, created_at, updated_at, kind, rule_json) VALUES ('smart', 'Favorites', 'star', '#ff5a36', 0, ?, ?, 'smart', ?)")
            .bind(timestamp)
            .bind(timestamp)
            .bind(rule.to_string())
            .execute(&state.library.pool)
            .await
            .unwrap();
        let smart_projects = project_ids_for_sync_scope(
            ModelQuery {
                collection_id: Some("smart".into()),
                ..ModelQuery::default()
            },
            &state.library,
        )
        .await
        .unwrap();
        assert_eq!(smart_projects, HashSet::from(["nested".to_string()]));
    }

    #[tokio::test]
    async fn original_asset_downloads_support_head_range_etag_and_hashing() {
        let (directory, state, token) = authenticated_test_state().await;
        let bytes = add_test_asset(&directory, &state).await;
        let router = companion_router(state.clone(), None);
        let full = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/assets/asset/content",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(
            full.headers()[header::CONTENT_LENGTH],
            bytes.len().to_string()
        );
        let etag = full.headers()[header::ETAG].clone();
        let full_body = axum::body::to_bytes(full.into_body(), 1024).await.unwrap();
        assert_eq!(full_body.as_ref(), bytes);

        let range = Request::builder()
            .method(axum::http::Method::GET)
            .uri("/api/v1/assets/asset/content")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::RANGE, "bytes=4-7")
            .body(Body::empty())
            .unwrap();
        let partial = router.clone().oneshot(range).await.unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.headers()[header::CONTENT_RANGE], "bytes 4-7/16");
        assert_eq!(
            axum::body::to_bytes(partial.into_body(), 1024)
                .await
                .unwrap()
                .as_ref(),
            b"4567"
        );

        let head = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::HEAD,
                "/api/v1/assets/asset/content",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[header::CONTENT_LENGTH], "16");
        assert!(axum::body::to_bytes(head.into_body(), 1024)
            .await
            .unwrap()
            .is_empty());

        let cached = Request::builder()
            .method(axum::http::Method::GET)
            .uri("/api/v1/assets/asset/content")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_NONE_MATCH, etag)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            router.oneshot(cached).await.unwrap().status(),
            StatusCode::NOT_MODIFIED
        );
        let stored_hash: Option<String> =
            sqlx::query_scalar("SELECT content_hash FROM assets WHERE id = 'asset'")
                .fetch_one(&state.library.pool)
                .await
                .unwrap();
        let expected_hash = blake3::hash(&bytes).to_hex().to_string();
        assert_eq!(stored_hash.as_deref(), Some(expected_hash.as_str()));
    }

    #[tokio::test]
    async fn managed_asset_replacements_require_the_downloaded_revision() {
        let (directory, state, token) = authenticated_test_state().await;
        let original = add_test_asset(&directory, &state).await;
        sqlx::query("UPDATE library_roots SET root_kind = 'managed' WHERE id = 'root'")
            .execute(&state.library.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE models SET storage_mode = 'managed' WHERE id = 'project'")
            .execute(&state.library.pool)
            .await
            .unwrap();
        let expected = blake3::hash(&original).to_hex().to_string();
        let replacement = b"new-model-bytes".to_vec();
        let request = Request::builder()
            .method(axum::http::Method::PUT)
            .uri("/api/v1/assets/asset/content")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, format!("\"{expected}\""))
            .header(header::CONTENT_LENGTH, replacement.len().to_string())
            .body(Body::from(replacement.clone()))
            .unwrap();
        let router = companion_router(state.clone(), None);
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            std::fs::read(directory.path().join("library/part.stl")).unwrap(),
            replacement
        );
        let stored_hash: Option<String> =
            sqlx::query_scalar("SELECT content_hash FROM assets WHERE id = 'asset'")
                .fetch_one(&state.library.pool)
                .await
                .unwrap();
        let replacement_hash = blake3::hash(&replacement).to_hex().to_string();
        assert_eq!(stored_hash.as_deref(), Some(replacement_hash.as_str()));

        let stale = Request::builder()
            .method(axum::http::Method::PUT)
            .uri("/api/v1/assets/asset/content")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, format!("\"{expected}\""))
            .header(header::CONTENT_LENGTH, "4")
            .body(Body::from("stale"))
            .unwrap();
        assert_eq!(
            router.oneshot(stale).await.unwrap().status(),
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn router_applies_write_scope_downgrades_without_blocking_reads() {
        let (_directory, state, token) = authenticated_test_state().await;
        let router = companion_router(state.clone(), None);
        let initial = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(initial.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(initial.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let status: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status["capabilities"]["organizationWrites"], true);
        assert_eq!(status["capabilities"]["photoUploads"], true);
        assert_eq!(status["capabilities"]["fileUploads"], true);

        state.controller.lock().await.config.organization_writes = false;
        let read_after_downgrade = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(read_after_downgrade.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(read_after_downgrade.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let status: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status["capabilities"]["organizationWrites"], false);
        assert_eq!(status["capabilities"]["photoUploads"], true);
        assert_eq!(status["capabilities"]["fileUploads"], false);

        let write = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::PATCH,
                "/api/v1/projects/project",
                &token,
                Body::from(
                    serde_json::json!({
                        "favorite": true,
                        "ifUnmodifiedSince": "2026-01-01T00:00:00Z"
                    })
                    .to_string(),
                ),
            ))
            .await
            .unwrap();
        assert_eq!(write.status(), StatusCode::FORBIDDEN);

        let upload = router
            .oneshot(authenticated_request(
                axum::http::Method::POST,
                "/api/v1/uploads?filename=blocked.stl&projectName=Blocked",
                &token,
                Body::from("solid blocked"),
            ))
            .await
            .unwrap();
        assert_eq!(upload.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn router_rejects_a_token_immediately_after_revocation() {
        let (_directory, state, token) = authenticated_test_state().await;
        let router = companion_router(state.clone(), None);
        let before = router
            .clone()
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(before.status(), StatusCode::OK);

        state.controller.lock().await.config.devices.clear();
        let revoked = router
            .oneshot(authenticated_request(
                axum::http::Method::GET,
                "/api/v1/status",
                &token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mobile_project_edits_reject_stale_revisions() {
        let (_directory, state) = mutation_test_state().await;
        let response = update_project(
            State(state.clone()),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: None,
                favorite: Some(true),
                notes: Some("Use four wall loops".into()),
                tag_ids: None,
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(response.summary.favorite);
        assert_eq!(response.notes, "Use four wall loops");
        assert_ne!(response.revision, "2026-01-01T00:00:00Z");

        let error = update_project(
            State(state),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: None,
                favorite: Some(false),
                notes: None,
                tag_ids: None,
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "edit_conflict");
    }

    #[tokio::test]
    async fn mobile_renames_and_tags_projects() {
        let (_directory, state) = mutation_test_state().await;
        let renamed = update_project(
            State(state.clone()),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: Some("Desk phone stand".into()),
                favorite: None,
                notes: None,
                tag_ids: None,
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(renamed.summary.display_name, "Desk phone stand");

        let tag = create_tag(
            State(state.clone()),
            Json(CreateTagRequest {
                name: "Prototype".into(),
                color: "#4d82ba".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let tagged = update_project(
            State(state),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: None,
                favorite: None,
                notes: None,
                tag_ids: Some(vec![tag.id.clone()]),
                if_unmodified_since: renamed.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(tagged.tags.len(), 1);
        assert_eq!(tagged.tags[0].id, tag.id);
    }

    #[tokio::test]
    async fn mobile_renames_recolors_and_deletes_tags() {
        let (_directory, state) = mutation_test_state().await;
        let tag = create_tag(
            State(state.clone()),
            Json(CreateTagRequest {
                name: "Prototype".into(),
                color: "#4d82ba".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let tagged = update_project(
            State(state.clone()),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: None,
                favorite: None,
                notes: None,
                tag_ids: Some(vec![tag.id.clone()]),
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap()
        .0;

        let updated = update_tag(
            State(state.clone()),
            Path(tag.id.clone()),
            Json(CreateTagRequest {
                name: "Ready to print".into(),
                color: "#4f9368".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(updated.name, "Ready to print");
        assert_eq!(updated.color, "#4f9368");
        assert_eq!(updated.model_count, 1);
        let renamed_project = load_project_detail("project", &state).await.unwrap();
        assert_eq!(renamed_project.tags[0].name, "Ready to print");
        assert_ne!(renamed_project.revision, tagged.revision);

        assert_eq!(
            delete_tag(State(state.clone()), Path(tag.id))
                .await
                .unwrap(),
            StatusCode::NO_CONTENT
        );
        let untagged = load_project_detail("project", &state).await.unwrap();
        assert!(untagged.tags.is_empty());
    }

    #[tokio::test]
    async fn organization_writes_are_enforced_server_side() {
        let (_directory, state) = mutation_test_state().await;
        state.controller.lock().await.config.organization_writes = false;
        let error = update_project(
            State(state),
            Path("project".into()),
            Json(ProjectUpdateRequest {
                display_name: None,
                favorite: Some(true),
                notes: None,
                tag_ids: None,
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "organization_writes_disabled");
    }

    #[tokio::test]
    async fn photo_upload_permission_is_independent_of_organization_writes() {
        let (_directory, state) = mutation_test_state().await;
        state.controller.lock().await.config.organization_writes = false;
        assert!(require_photo_uploads(&state).await.is_ok());

        {
            let mut inner = state.controller.lock().await;
            inner.config.organization_writes = true;
            inner.config.photo_uploads = Some(false);
        }
        let error = require_photo_uploads(&state).await.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "photo_uploads_disabled");
        assert!(require_organization_writes(&state).await.is_ok());
    }

    #[test]
    fn legacy_companion_config_preserves_existing_photo_access() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("companion.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "enabled": false,
                "serverId": "legacy-server",
                "port": DEFAULT_PORT,
                "devices": [],
                "organizationWrites": true,
                "networkMode": "automatic"
            })
            .to_string(),
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.photo_uploads, Some(true));
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted["photoUploads"], true);
    }

    #[tokio::test]
    async fn invalid_pairing_attempts_do_not_destroy_the_active_code() {
        let (_directory, state, _token) = authenticated_test_state().await;
        state.controller.lock().await.pairing = Some(PairingSession {
            secret: "valid-pairing-secret".into(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            role: "client".into(),
        });

        for _ in 0..12 {
            let error = pair(
                State(state.clone()),
                Json(PairRequest {
                    secret: "wrong".into(),
                    device_name: "Attempt".into(),
                    platform: "Test".into(),
                }),
            )
            .await
            .unwrap_err();
            assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        }

        let response = pair(
            State(state),
            Json(PairRequest {
                secret: "valid-pairing-secret".into(),
                device_name: "Paired device".into(),
                platform: "Test".into(),
            }),
        )
        .await
        .unwrap();
        assert!(!response.0.token.is_empty());
    }

    #[tokio::test]
    async fn mobile_creates_and_assigns_manual_collections() {
        let (_directory, state) = mutation_test_state().await;
        let collection = create_collection(
            State(state.clone()),
            Json(CollectionInput {
                name: "Workshop".into(),
                symbol: "boxes".into(),
                color: "#ff5a36".into(),
                smart: false,
                rule: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let project = add_project_to_collection(
            State(state),
            Path((collection.id.clone(), "project".into())),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(project.collection_ids, vec![collection.id]);
    }

    #[tokio::test]
    async fn mobile_updates_and_deletes_manual_collections() {
        let (_directory, state) = mutation_test_state().await;
        let collection = create_collection(
            State(state.clone()),
            Json(CollectionInput {
                name: "Workshop".into(),
                symbol: "boxes".into(),
                color: "#ff5a36".into(),
                smart: false,
                rule: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let _ = add_project_to_collection(
            State(state.clone()),
            Path((collection.id.clone(), "project".into())),
        )
        .await
        .unwrap();

        let updated = update_collection(
            State(state.clone()),
            Path(collection.id.clone()),
            Json(CollectionInput {
                name: "Printer queue".into(),
                symbol: "boxes".into(),
                color: "#4f9368".into(),
                smart: false,
                rule: None,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(updated.name, "Printer queue");
        assert_eq!(updated.model_count, 1);

        assert_eq!(
            delete_collection(State(state.clone()), Path(collection.id))
                .await
                .unwrap(),
            StatusCode::NO_CONTENT
        );
        assert!(load_project_detail("project", &state)
            .await
            .unwrap()
            .collection_ids
            .is_empty());
    }

    #[tokio::test]
    async fn mobile_cost_estimates_use_desktop_material_prices() {
        let (_directory, state) = mutation_test_state().await;
        let timestamp = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO materials (id, name, material_type, spool_price_minor, currency, spool_weight_g, created_at, updated_at) VALUES ('pla', 'Workshop PLA', 'PLA', 2000, 'EUR', 1000, ?, ?)")
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.library.pool)
            .await
            .unwrap();

        let project = save_cost_estimate(
            State(state),
            Path("project".into()),
            Json(CostEstimateRequest {
                asset_id: None,
                material_id: Some("pla".into()),
                plastic_g: 100.0,
                quantity: 2,
                source: "manual".into(),
                if_unmodified_since: timestamp.into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let estimate = project.estimate.unwrap();
        assert_eq!(estimate.material_id.as_deref(), Some("pla"));
        assert_eq!(estimate.cost_per_piece_minor, Some(200.0));
        assert_eq!(estimate.batch_cost_minor, Some(400.0));
    }

    #[tokio::test]
    async fn mobile_uploads_updates_and_deletes_project_photos() {
        let (_directory, state) = mutation_test_state().await;
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([255, 90, 54]));
        let mut cursor = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        let bytes = cursor.into_inner();
        let request = Request::builder()
            .header(header::CONTENT_LENGTH, bytes.len().to_string())
            .body(Body::from(bytes))
            .unwrap();

        let uploaded = upload_project_photo(
            State(state.clone()),
            Path("project".into()),
            Query(PhotoUploadQuery {
                filename: "workshop.png".into(),
                caption: "First print".into(),
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
            request,
        )
        .await
        .unwrap()
        .0;
        assert_eq!(uploaded.documentation.photos.len(), 1);
        let photo = uploaded.documentation.photos[0].clone();
        assert_eq!(photo.caption, "First print");
        assert!(photo.is_cover);

        let updated = update_project_photo(
            State(state.clone()),
            Path(("project".into(), photo.id.clone())),
            Json(PhotoUpdateRequest {
                caption: "Finished print".into(),
                sort_order: 2.0,
                is_cover: true,
                if_unmodified_since: uploaded.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(updated.documentation.photos[0].caption, "Finished print");

        let empty = delete_project_photo(
            State(state),
            Path(("project".into(), photo.id)),
            Json(RevisionRequest {
                if_unmodified_since: updated.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(empty.documentation.photos.is_empty());
    }

    #[tokio::test]
    async fn mobile_custom_values_validate_and_reject_stale_revisions() {
        let (_directory, state) = mutation_test_state().await;
        let timestamp = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO custom_field_definitions (id, name, field_type, options_json, sort_order, created_at, updated_at) VALUES ('field', 'Layer height', 'number', '[]', 1, ?, ?)")
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.library.pool)
            .await
            .unwrap();

        let documentation = set_custom_value(
            State(state.clone()),
            Path(("project".into(), "field".into())),
            Json(CustomValueRequest {
                value: Some(serde_json::json!(0.2)),
                if_unmodified_since: timestamp.into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(documentation.documentation.custom_values.len(), 1);
        assert_eq!(
            documentation.documentation.custom_values[0].value,
            serde_json::json!(0.2)
        );

        let stale = set_custom_value(
            State(state.clone()),
            Path(("project".into(), "field".into())),
            Json(CustomValueRequest {
                value: Some(serde_json::json!(0.3)),
                if_unmodified_since: timestamp.into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(stale.status, StatusCode::CONFLICT);

        let invalid = set_custom_value(
            State(state.clone()),
            Path(("project".into(), "field".into())),
            Json(CustomValueRequest {
                value: Some(serde_json::json!("not a number")),
                if_unmodified_since: documentation.revision.clone(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(invalid.status, StatusCode::BAD_REQUEST);

        let retry = set_custom_value(
            State(state),
            Path(("project".into(), "field".into())),
            Json(CustomValueRequest {
                value: Some(serde_json::json!(0.3)),
                if_unmodified_since: documentation.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(
            retry.documentation.custom_values[0].value,
            serde_json::json!(0.3)
        );
    }

    #[tokio::test]
    async fn mobile_manages_bom_and_assembly_with_ownership_checks() {
        let (_directory, state) = mutation_test_state().await;
        let timestamp = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, added_at, updated_at) VALUES ('other', 'folder', 'Other project', 'other', ?, ?)")
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.library.pool)
            .await
            .unwrap();

        let with_bom = save_bom_item(
            State(state.clone()),
            Path("project".into()),
            Json(BomItemRequest {
                id: None,
                kind: "purchased".into(),
                linked_asset_id: None,
                name: "M3 screw".into(),
                quantity: 4.0,
                unit: "pcs".into(),
                url: Some("https://example.com/screws".into()),
                notes: "12 mm".into(),
                sort_order: None,
                if_unmodified_since: timestamp.into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let item_id = with_bom.documentation.bom_items[0].id.clone();

        let with_step = save_assembly_step(
            State(state.clone()),
            Path("project".into()),
            Json(AssemblyStepRequest {
                id: None,
                title: "Fasten the base".into(),
                instructions: "Tighten evenly.".into(),
                sort_order: None,
                bom_item_ids: vec![item_id.clone()],
                photo_ids: Vec::new(),
                if_unmodified_since: with_bom.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(with_step.documentation.assembly_steps.len(), 1);
        assert_eq!(
            with_step.documentation.assembly_steps[0].bom_item_ids,
            vec![item_id.clone()]
        );
        let step_id = with_step.documentation.assembly_steps[0].id.clone();

        let wrong_owner = delete_bom_item(
            State(state.clone()),
            Path(("other".into(), item_id.clone())),
            Json(RevisionRequest {
                if_unmodified_since: timestamp.into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(wrong_owner.status, StatusCode::NOT_FOUND);

        let without_step = delete_assembly_step(
            State(state.clone()),
            Path(("project".into(), step_id)),
            Json(RevisionRequest {
                if_unmodified_since: with_step.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(without_step.documentation.assembly_steps.is_empty());

        let empty = delete_bom_item(
            State(state),
            Path(("project".into(), item_id)),
            Json(RevisionRequest {
                if_unmodified_since: without_step.revision,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(empty.documentation.bom_items.is_empty());
    }

    #[tokio::test]
    async fn disabled_organization_writes_block_documentation_changes() {
        let (_directory, state) = mutation_test_state().await;
        state.controller.lock().await.config.organization_writes = false;
        let error = save_bom_item(
            State(state),
            Path("project".into()),
            Json(BomItemRequest {
                id: None,
                kind: "purchased".into(),
                linked_asset_id: None,
                name: "Blocked part".into(),
                quantity: 1.0,
                unit: "pcs".into(),
                url: None,
                notes: String::new(),
                sort_order: None,
                if_unmodified_since: "2026-01-01T00:00:00Z".into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "organization_writes_disabled");
    }

    #[tokio::test]
    #[ignore = "requires a running Tailscale connection"]
    async fn tailnet_pairing_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let library = AppState::initialize(directory.path()).await.unwrap();
        let controller = CompanionController::new(directory.path(), library, None)
            .expect("controller should initialize");
        let status = controller.set_enabled(true).await.unwrap();
        assert!(status.running);
        let pairing = controller.create_pairing().await.unwrap();
        let client = reqwest::Client::new();
        let pair: PairResponse = client
            .post(format!("{}/pair", pairing.base_url))
            .json(&serde_json::json!({
                "secret": pairing.secret,
                "deviceName": "Integration test",
                "platform": "Test"
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let replay = client
            .post(format!("{}/pair", pairing.base_url))
            .json(&serde_json::json!({
                "secret": pairing.secret,
                "deviceName": "Replay test",
                "platform": "Test"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(replay.status(), reqwest::StatusCode::GONE);
        let api_status = client
            .get(format!("{}/api/v1/status", pairing.base_url))
            .bearer_auth(&pair.token)
            .send()
            .await
            .unwrap();
        assert_eq!(api_status.status(), reqwest::StatusCode::OK);
        controller.revoke_device(&pair.device_id).await.unwrap();
        let revoked = client
            .get(format!("{}/api/v1/status", pairing.base_url))
            .bearer_auth(pair.token)
            .send()
            .await
            .unwrap();
        assert_eq!(revoked.status(), reqwest::StatusCode::UNAUTHORIZED);
        controller.stop().await;
    }
}
