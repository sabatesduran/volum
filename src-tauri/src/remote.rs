use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tauri::ipc::Response as IpcResponse;
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tokio_util::io::ReaderStream;

const KEYRING_SERVICE: &str = "dev.didac.volum.remote";
const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_BINARY_BYTES: usize = 64 * 1024 * 1024;

type RemoteResult<T> = Result<T, String>;

#[derive(Clone)]
pub struct RemoteController {
    inner: Arc<Mutex<RemoteInner>>,
    client: reqwest::Client,
    sync_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    test_token: Arc<Mutex<Option<String>>>,
}

struct RemoteInner {
    config: BackendConfig,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendConfig {
    #[serde(default = "default_mode")]
    mode: String,
    remote: Option<RemoteEndpoint>,
    mirror: Option<MirrorConfig>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            remote: None,
            mirror: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteEndpoint {
    server_id: String,
    server_name: String,
    base_url: String,
    device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MirrorConfig {
    path: String,
    #[serde(default)]
    enabled: bool,
    last_sync_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncManifest {
    server_id: String,
    cursor: i64,
    assets: Vec<SyncAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncAsset {
    id: String,
    project_id: String,
    project_name: String,
    filename: String,
    byte_size: u64,
    modified_at: String,
    content_hash: Option<String>,
    mirror_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalMirrorManifest {
    format_version: u32,
    server_id: String,
    cursor: i64,
    assets: Vec<LocalMirrorAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalMirrorAsset {
    id: String,
    project_id: String,
    project_name: String,
    filename: String,
    byte_size: u64,
    modified_at: String,
    content_hash: String,
    mirror_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorSyncReport {
    downloaded: usize,
    uploaded: usize,
    removed: usize,
    conflicts: usize,
    unchanged: usize,
    total_assets: usize,
    cursor: i64,
    completed_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MirroredAsset {
    id: String,
    project_name: String,
    filename: String,
    byte_size: u64,
    modified_at: String,
    mirror_path: String,
}

#[derive(Debug)]
struct RemoteFileIdentity {
    byte_size: u64,
    content_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingPayload {
    version: u32,
    server_id: String,
    server_name: String,
    base_url: String,
    secret: String,
    expires_at: String,
    transport: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    server_id: String,
    device_id: String,
    token: String,
    protocol_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResponse {
    server_id: String,
    protocol_min: u32,
    protocol_max: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendConnectionStatus {
    mode: String,
    configured: bool,
    online: bool,
    server_id: Option<String>,
    server_name: Option<String>,
    base_url: Option<String>,
    mirror_enabled: bool,
    mirror_path: Option<String>,
    last_sync_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRequest {
    path: String,
    method: Option<String>,
    body: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUploadRequest {
    file_path: String,
    filename: String,
    project_id: Option<String>,
    project_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePhotoUploadRequest {
    file_path: String,
    filename: String,
    project_id: String,
    if_unmodified_since: String,
}

impl RemoteController {
    pub fn new(data_dir: &Path) -> RemoteResult<Self> {
        let path = data_dir.join("remote.json");
        let config = load_config(&path)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| format!("Could not initialize remote networking: {error}"))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(RemoteInner { config, path })),
            client,
            sync_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            test_token: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn run_background_sync(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let should_sync = {
                let inner = self.inner.lock().await;
                inner.config.mode == "remote"
                    && inner
                        .config
                        .mirror
                        .as_ref()
                        .is_some_and(|mirror| mirror.enabled)
            };
            if should_sync {
                if let Err(error) = self.sync_mirror().await {
                    log::warn!("Remote mirror synchronization failed: {error}");
                }
            }
        }
    }

    async fn endpoint_and_token(&self) -> RemoteResult<(RemoteEndpoint, String)> {
        let endpoint = self
            .inner
            .lock()
            .await
            .config
            .remote
            .clone()
            .ok_or_else(|| "No remote Volum server is configured".to_string())?;
        #[cfg(test)]
        if let Some(token) = self.test_token.lock().await.clone() {
            return Ok((endpoint, token));
        }
        let server_id = endpoint.server_id.clone();
        let token = tokio::task::spawn_blocking(move || load_token(&server_id))
            .await
            .map_err(|error| error.to_string())??;
        Ok((endpoint, token))
    }

    async fn status(&self) -> BackendConnectionStatus {
        let config = self.inner.lock().await.config.clone();
        let Some(_) = config.remote.as_ref() else {
            return status_from_config(&config, false, None);
        };
        if config.mode != "remote" {
            return status_from_config(&config, false, None);
        }
        let online = match self.endpoint_and_token().await {
            Ok((endpoint, token)) => self
                .send_json(&endpoint, &token, "/api/v1/status", "GET", None)
                .await
                .map(|_| true),
            Err(error) => Err(error),
        };
        match online {
            Ok(value) => status_from_config(&config, value, None),
            Err(error) => status_from_config(&config, false, Some(error)),
        }
    }

    async fn connect(&self, payload_json: &str) -> RemoteResult<BackendConnectionStatus> {
        let payload: PairingPayload = serde_json::from_str(payload_json.trim())
            .map_err(|_| "Paste a complete Volum pairing payload".to_string())?;
        if payload.version != 1
            || payload.transport != "tailscale-http"
            || payload.server_id.trim().is_empty()
            || payload.secret.trim().is_empty()
        {
            return Err("This is not a compatible Volum pairing payload".into());
        }
        let expires_at = DateTime::parse_from_rfc3339(&payload.expires_at)
            .map_err(|_| "The pairing expiry is invalid".to_string())?
            .with_timezone(&Utc);
        if expires_at <= Utc::now() {
            return Err("This pairing payload has expired".into());
        }
        let base_url = validate_base_url(&payload.base_url)?;
        let identity_url = validated_url(&base_url, "/identity")?;
        let identity = self
            .client
            .get(identity_url)
            .send()
            .await
            .map_err(friendly_network_error)?
            .error_for_status()
            .map_err(|_| "The Volum server identity could not be verified".to_string())?
            .json::<IdentityResponse>()
            .await
            .map_err(|_| "The Volum server returned an unreadable identity".to_string())?;
        if identity.server_id != payload.server_id
            || identity.protocol_min > 1
            || identity.protocol_max < 1
        {
            return Err("The Volum server identity or protocol changed during pairing".into());
        }
        let pair_url = validated_url(&base_url, "/pair")?;
        let response = self
            .client
            .post(pair_url)
            .json(&serde_json::json!({
                "secret": payload.secret,
                "deviceName": desktop_device_name(),
                "platform": std::env::consts::OS,
            }))
            .send()
            .await
            .map_err(friendly_network_error)?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "The Volum server returned an unreadable pairing response".to_string())?;
        if !status.is_success() {
            return Err(api_error_message(
                &bytes,
                "The Volum server rejected this pairing",
            ));
        }
        let paired: PairResponse = serde_json::from_slice(&bytes)
            .map_err(|_| "The Volum server returned an unreadable pairing response".to_string())?;
        if paired.server_id != payload.server_id || paired.protocol_version != 1 {
            return Err("The Volum server identity changed during pairing".into());
        }
        let token_server_id = paired.server_id.clone();
        let token = paired.token.clone();
        tokio::task::spawn_blocking(move || save_token(&token_server_id, &token))
            .await
            .map_err(|error| error.to_string())??;
        let mut inner = self.inner.lock().await;
        let previous_server = inner
            .config
            .remote
            .as_ref()
            .map(|remote| remote.server_id.clone());
        inner.config.mode = "remote".into();
        inner.config.remote = Some(RemoteEndpoint {
            server_id: paired.server_id,
            server_name: payload.server_name.trim().to_string(),
            base_url,
            device_id: paired.device_id,
        });
        if previous_server.as_deref() != Some(payload.server_id.as_str()) {
            inner.config.mirror = None;
        }
        persist_config(&inner.path, &inner.config)?;
        let config = inner.config.clone();
        drop(inner);
        Ok(status_from_config(&config, true, None))
    }

    async fn use_local(&self) -> RemoteResult<BackendConnectionStatus> {
        let mut inner = self.inner.lock().await;
        inner.config.mode = "local".into();
        persist_config(&inner.path, &inner.config)?;
        Ok(status_from_config(&inner.config, false, None))
    }

    async fn use_remote(&self) -> RemoteResult<BackendConnectionStatus> {
        {
            let mut inner = self.inner.lock().await;
            if inner.config.remote.is_none() {
                return Err("Pair this Desktop with a Volum server first".into());
            }
            inner.config.mode = "remote".into();
            persist_config(&inner.path, &inner.config)?;
        }
        Ok(self.status().await)
    }

    async fn forget_remote(&self) -> RemoteResult<BackendConnectionStatus> {
        let server_id = self
            .inner
            .lock()
            .await
            .config
            .remote
            .as_ref()
            .map(|remote| remote.server_id.clone());
        if let Some(server_id) = server_id {
            let _ = tokio::task::spawn_blocking(move || delete_token(&server_id)).await;
        }
        let mut inner = self.inner.lock().await;
        inner.config.mode = "local".into();
        inner.config.remote = None;
        inner.config.mirror = None;
        persist_config(&inner.path, &inner.config)?;
        Ok(status_from_config(&inner.config, false, None))
    }

    async fn send_json(
        &self,
        endpoint: &RemoteEndpoint,
        token: &str,
        path: &str,
        method: &str,
        body: Option<&serde_json::Value>,
    ) -> RemoteResult<serde_json::Value> {
        let url = validated_url(&endpoint.base_url, path)?;
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "PATCH" => self.client.patch(url),
            "DELETE" => self.client.delete(url),
            _ => return Err("The remote request method is invalid".into()),
        }
        .bearer_auth(token);
        if method == "GET" && body.is_some() {
            return Err("Remote read requests cannot include a body".into());
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(friendly_network_error)?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_JSON_BYTES as u64)
        {
            return Err("The Volum server returned too much data".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "The Volum server returned an unreadable response".to_string())?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err("The Volum server returned too much data".into());
        }
        if !status.is_success() {
            return Err(api_error_message(
                &bytes,
                &format!("The Volum server returned {status}"),
            ));
        }
        if bytes.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| "The Volum server returned an unreadable response".into())
    }

    async fn configure_mirror(
        &self,
        path: Option<String>,
        enabled: bool,
    ) -> RemoteResult<BackendConnectionStatus> {
        let mut inner = self.inner.lock().await;
        if inner.config.remote.is_none() {
            return Err("Pair this Desktop with a Volum server before enabling a mirror".into());
        }
        if enabled {
            let selected = PathBuf::from(
                path.as_deref()
                    .or_else(|| {
                        inner
                            .config
                            .mirror
                            .as_ref()
                            .map(|value| value.path.as_str())
                    })
                    .ok_or_else(|| "Choose a folder for the managed mirror".to_string())?,
            );
            std::fs::create_dir_all(&selected)
                .map_err(|error| format!("Could not create the mirror folder: {error}"))?;
            let canonical = selected
                .canonicalize()
                .map_err(|_| "Choose a mirror folder that is currently available".to_string())?;
            if !canonical.is_dir() {
                return Err("The managed mirror location must be a folder".into());
            }
            let config_dir = inner.path.parent().unwrap_or_else(|| Path::new("."));
            if config_dir
                .canonicalize()
                .is_ok_and(|config| paths_overlap(&canonical, &config))
            {
                return Err("Choose a mirror outside Volum's app-data folder".into());
            }
            inner.config.mirror = Some(MirrorConfig {
                path: canonical.to_string_lossy().to_string(),
                enabled: true,
                last_sync_at: inner
                    .config
                    .mirror
                    .as_ref()
                    .and_then(|value| value.last_sync_at.clone()),
                last_error: None,
            });
        } else if let Some(mirror) = inner.config.mirror.as_mut() {
            mirror.enabled = false;
            mirror.last_error = None;
        }
        persist_config(&inner.path, &inner.config)?;
        Ok(status_from_config(
            &inner.config,
            inner.config.mode == "remote",
            None,
        ))
    }

    async fn sync_mirror(&self) -> RemoteResult<MirrorSyncReport> {
        let _sync_guard = self.sync_lock.lock().await;
        let result = self.sync_mirror_inner().await;
        let mut inner = self.inner.lock().await;
        if let Some(mirror) = inner.config.mirror.as_mut() {
            match &result {
                Ok(report) => {
                    mirror.last_sync_at = Some(report.completed_at.clone());
                    mirror.last_error = None;
                }
                Err(error) => mirror.last_error = Some(error.clone()),
            }
            let _ = persist_config(&inner.path, &inner.config);
        }
        result
    }

    async fn sync_mirror_inner(&self) -> RemoteResult<MirrorSyncReport> {
        let mirror = self
            .inner
            .lock()
            .await
            .config
            .mirror
            .clone()
            .filter(|value| value.enabled)
            .ok_or_else(|| "Enable the managed mirror and choose its folder first".to_string())?;
        let root = PathBuf::from(&mirror.path);
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("The mirror folder is unavailable: {error}"))?;
        let root = root
            .canonicalize()
            .map_err(|_| "The mirror folder is unavailable".to_string())?;
        let (endpoint, token) = self.endpoint_and_token().await?;
        let manifest_value = self
            .send_json(&endpoint, &token, "/api/v1/sync/manifest", "GET", None)
            .await?;
        let remote: SyncManifest = serde_json::from_value(manifest_value)
            .map_err(|_| "The Volum server returned an invalid sync manifest".to_string())?;
        if remote.server_id != endpoint.server_id {
            return Err("The Volum server identity changed during synchronization".into());
        }
        let partials_dir = ensure_mirror_directory(&root, Path::new(".volum/partials"))?;
        let local_path = checked_relative_path(&root, Path::new(".volum/manifest.json"))?;
        let previous = read_local_manifest(&local_path, &endpoint.server_id)?;
        let previous_by_id = previous
            .as_ref()
            .map(|value| {
                value
                    .assets
                    .iter()
                    .cloned()
                    .map(|asset| (asset.id.clone(), asset))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let remote_ids = remote
            .assets
            .iter()
            .map(|asset| asset.id.clone())
            .collect::<HashSet<_>>();
        let mut report = MirrorSyncReport {
            downloaded: 0,
            uploaded: 0,
            removed: 0,
            conflicts: 0,
            unchanged: 0,
            total_assets: remote.assets.len(),
            cursor: remote.cursor,
            completed_at: String::new(),
        };
        let mut completed_assets = Vec::with_capacity(remote.assets.len());
        for asset in &remote.assets {
            let target = checked_mirror_path(&root, &asset.mirror_path)?;
            ensure_mirror_parent(&root, &target)?;
            let identity = self.remote_file_identity(&endpoint, &token, asset).await?;
            let previous_asset = previous_by_id.get(&asset.id);
            if let Some(previous_asset) =
                previous_asset.filter(|old| old.mirror_path != asset.mirror_path)
            {
                let previous_target = checked_mirror_path(&root, &previous_asset.mirror_path)?;
                if previous_target.is_file() {
                    let old_hash = hash_path(&previous_target).await?;
                    if old_hash == previous_asset.content_hash
                        && identity.content_hash == previous_asset.content_hash
                        && !target.exists()
                    {
                        ensure_mirror_parent(&root, &target)?;
                        tokio::fs::rename(&previous_target, &target)
                            .await
                            .map_err(|error| {
                                format!("Could not move a renamed mirror file: {error}")
                            })?;
                    } else {
                        preserve_conflict(&root, &previous_target, &previous_asset.mirror_path)
                            .await?;
                        report.conflicts += 1;
                    }
                }
            }
            let previous_hash = previous_asset
                .filter(|old| old.mirror_path == asset.mirror_path)
                .map(|old| old.content_hash.as_str());
            let mut final_identity = identity;
            if target.is_file() {
                let local_hash = hash_path(&target).await?;
                if local_hash == final_identity.content_hash {
                    report.unchanged += 1;
                } else if previous_hash == Some(local_hash.as_str()) {
                    final_identity = self
                        .download_asset(
                            &endpoint,
                            &token,
                            asset,
                            &final_identity,
                            &target,
                            &partials_dir,
                        )
                        .await?;
                    report.downloaded += 1;
                } else if previous_hash == Some(final_identity.content_hash.as_str()) {
                    match self
                        .upload_asset(
                            &endpoint,
                            &token,
                            asset,
                            &target,
                            &final_identity.content_hash,
                        )
                        .await
                    {
                        Ok(uploaded) => {
                            final_identity = uploaded;
                            report.uploaded += 1;
                        }
                        Err(error)
                            if error.contains("changed")
                                || error.contains("managed library")
                                || error.contains("managed") =>
                        {
                            preserve_conflict(&root, &target, &asset.mirror_path).await?;
                            report.conflicts += 1;
                            final_identity = self
                                .download_asset(
                                    &endpoint,
                                    &token,
                                    asset,
                                    &final_identity,
                                    &target,
                                    &partials_dir,
                                )
                                .await?;
                            report.downloaded += 1;
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    preserve_conflict(&root, &target, &asset.mirror_path).await?;
                    report.conflicts += 1;
                    final_identity = self
                        .download_asset(
                            &endpoint,
                            &token,
                            asset,
                            &final_identity,
                            &target,
                            &partials_dir,
                        )
                        .await?;
                    report.downloaded += 1;
                }
            } else {
                final_identity = self
                    .download_asset(
                        &endpoint,
                        &token,
                        asset,
                        &final_identity,
                        &target,
                        &partials_dir,
                    )
                    .await?;
                report.downloaded += 1;
            }
            completed_assets.push(LocalMirrorAsset {
                id: asset.id.clone(),
                project_id: asset.project_id.clone(),
                project_name: asset.project_name.clone(),
                filename: asset.filename.clone(),
                byte_size: final_identity.byte_size,
                modified_at: asset.modified_at.clone(),
                content_hash: final_identity.content_hash,
                mirror_path: asset.mirror_path.clone(),
            });
        }
        for stale in previous_by_id
            .values()
            .filter(|asset| !remote_ids.contains(&asset.id))
        {
            let stale_path = checked_mirror_path(&root, &stale.mirror_path)?;
            if !stale_path.is_file() {
                continue;
            }
            if hash_path(&stale_path).await? == stale.content_hash {
                tokio::fs::remove_file(&stale_path).await.map_err(|error| {
                    format!("Could not remove a deleted server file from the mirror: {error}")
                })?;
                report.removed += 1;
            } else {
                preserve_conflict(&root, &stale_path, &stale.mirror_path).await?;
                report.conflicts += 1;
            }
        }
        report.completed_at = Utc::now().to_rfc3339();
        write_local_manifest(
            &local_path,
            &LocalMirrorManifest {
                format_version: 1,
                server_id: endpoint.server_id,
                cursor: remote.cursor,
                assets: completed_assets,
            },
        )?;
        Ok(report)
    }

    async fn remote_file_identity(
        &self,
        endpoint: &RemoteEndpoint,
        token: &str,
        asset: &SyncAsset,
    ) -> RemoteResult<RemoteFileIdentity> {
        let path = format!(
            "/api/v1/assets/{}/content",
            percent_encoding::utf8_percent_encode(&asset.id, percent_encoding::NON_ALPHANUMERIC)
        );
        let response = self
            .client
            .head(validated_url(&endpoint.base_url, &path)?)
            .bearer_auth(token)
            .send()
            .await
            .map_err(friendly_network_error)?;
        if !response.status().is_success() {
            return Err(format!("The server file {} is unavailable", asset.filename));
        }
        let byte_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| "The server omitted a mirrored file size".to_string())?;
        let content_hash = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim_matches('"').to_string())
            .filter(|value| {
                value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
            })
            .ok_or_else(|| "The server omitted a valid mirrored file hash".to_string())?;
        if byte_size != asset.byte_size {
            return Err("The server file changed while its sync manifest was being read".into());
        }
        if asset
            .content_hash
            .as_deref()
            .is_some_and(|hash| hash != content_hash)
        {
            return Err(
                "The server file hash changed while its sync manifest was being read".into(),
            );
        }
        Ok(RemoteFileIdentity {
            byte_size,
            content_hash,
        })
    }

    async fn download_asset(
        &self,
        endpoint: &RemoteEndpoint,
        token: &str,
        asset: &SyncAsset,
        identity: &RemoteFileIdentity,
        target: &Path,
        partials_dir: &Path,
    ) -> RemoteResult<RemoteFileIdentity> {
        let partial = partials_dir.join(format!("{}.part", safe_identifier(&asset.id)?));
        match std::fs::symlink_metadata(&partial) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("A mirror partial download has an unsafe file type".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Could not inspect a mirror partial download: {error}"
                ));
            }
        }
        let existing = tokio::fs::metadata(&partial)
            .await
            .map(|value| value.len())
            .unwrap_or(0);
        if existing > identity.byte_size {
            let _ = tokio::fs::remove_file(&partial).await;
        }
        let offset = tokio::fs::metadata(&partial)
            .await
            .map(|value| value.len())
            .unwrap_or(0);
        let path = format!(
            "/api/v1/assets/{}/content",
            percent_encoding::utf8_percent_encode(&asset.id, percent_encoding::NON_ALPHANUMERIC)
        );
        let mut request = self
            .client
            .get(validated_url(&endpoint.base_url, &path)?)
            .bearer_auth(token);
        if offset > 0 && offset < identity.byte_size {
            request = request
                .header(reqwest::header::RANGE, format!("bytes={offset}-"))
                .header(
                    reqwest::header::IF_RANGE,
                    format!("\"{}\"", identity.content_hash),
                );
        }
        let response = request.send().await.map_err(friendly_network_error)?;
        if !response.status().is_success() {
            let status = response.status();
            let bytes = response.bytes().await.unwrap_or_default();
            return Err(api_error_message(
                &bytes,
                &format!("Could not download {} ({status})", asset.filename),
            ));
        }
        let append = response.status() == reqwest::StatusCode::PARTIAL_CONTENT && offset > 0;
        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).write(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options
            .open(&partial)
            .await
            .map_err(|error| format!("Could not prepare a partial mirror download: {error}"))?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk.map_err(|_| "The mirror download was interrupted".to_string())?)
                .await
                .map_err(|error| format!("Could not write a mirror download: {error}"))?;
        }
        file.flush().await.map_err(|error| error.to_string())?;
        drop(file);
        let downloaded_size = tokio::fs::metadata(&partial)
            .await
            .map_err(|error| error.to_string())?
            .len();
        let downloaded_hash = hash_path(&partial).await?;
        if downloaded_size != identity.byte_size || downloaded_hash != identity.content_hash {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(format!(
                "The downloaded copy of {} failed verification",
                asset.filename
            ));
        }
        activate_download(&partial, target).await?;
        Ok(RemoteFileIdentity {
            byte_size: downloaded_size,
            content_hash: downloaded_hash,
        })
    }

    async fn upload_asset(
        &self,
        endpoint: &RemoteEndpoint,
        token: &str,
        asset: &SyncAsset,
        path: &Path,
        expected_hash: &str,
    ) -> RemoteResult<RemoteFileIdentity> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|_| "A locally changed mirror file is unavailable".to_string())?;
        let byte_size = file
            .metadata()
            .await
            .map_err(|error| error.to_string())?
            .len();
        if byte_size == 0 || byte_size > 500 * 1024 * 1024 {
            return Err("A changed mirror file must be non-empty and at most 500 MB".into());
        }
        let request_path = format!(
            "/api/v1/assets/{}/content",
            percent_encoding::utf8_percent_encode(&asset.id, percent_encoding::NON_ALPHANUMERIC)
        );
        let response = self
            .client
            .put(validated_url(&endpoint.base_url, &request_path)?)
            .bearer_auth(token)
            .header(reqwest::header::IF_MATCH, format!("\"{expected_hash}\""))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header(reqwest::header::CONTENT_LENGTH, byte_size)
            .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
            .send()
            .await
            .map_err(friendly_network_error)?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "The server returned an unreadable upload response".to_string())?;
        if !status.is_success() {
            return Err(api_error_message(
                &bytes,
                "The server rejected the changed mirror file",
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| "The server returned an unreadable upload response".to_string())?;
        let content_hash = value["contentHash"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or_else(|| "The server omitted the uploaded file hash".to_string())?
            .to_string();
        let byte_size = value["byteSize"].as_u64().unwrap_or(byte_size);
        Ok(RemoteFileIdentity {
            byte_size,
            content_hash,
        })
    }

    async fn mirrored_asset_path(&self, asset_id: &str) -> RemoteResult<PathBuf> {
        let mirror = self
            .inner
            .lock()
            .await
            .config
            .mirror
            .clone()
            .filter(|value| value.enabled)
            .ok_or_else(|| "Enable and synchronize the managed mirror first".to_string())?;
        let root = PathBuf::from(mirror.path)
            .canonicalize()
            .map_err(|_| "The mirror folder is unavailable".to_string())?;
        let manifest_path = checked_relative_path(&root, Path::new(".volum/manifest.json"))?;
        let manifest = read_local_manifest(&manifest_path, "")?
            .ok_or_else(|| "Synchronize the managed mirror before opening files".to_string())?;
        let asset = manifest
            .assets
            .into_iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| "This file has not been mirrored yet".to_string())?;
        let path = checked_mirror_path(&root, &asset.mirror_path)?;
        if !path.is_file() {
            return Err("This mirrored file is unavailable".into());
        }
        Ok(path)
    }

    async fn mirrored_assets(&self) -> RemoteResult<Vec<MirroredAsset>> {
        let mirror = self
            .inner
            .lock()
            .await
            .config
            .mirror
            .clone()
            .filter(|value| value.enabled)
            .ok_or_else(|| "No managed mirror is enabled".to_string())?;
        let root = PathBuf::from(mirror.path)
            .canonicalize()
            .map_err(|_| "The mirror folder is unavailable".to_string())?;
        let manifest_path = checked_relative_path(&root, Path::new(".volum/manifest.json"))?;
        let Some(manifest) = read_local_manifest(&manifest_path, "")? else {
            return Ok(Vec::new());
        };
        Ok(manifest
            .assets
            .into_iter()
            .filter(|asset| {
                checked_mirror_path(&root, &asset.mirror_path).is_ok_and(|path| path.is_file())
            })
            .map(|asset| MirroredAsset {
                id: asset.id,
                project_name: asset.project_name,
                filename: asset.filename,
                byte_size: asset.byte_size,
                modified_at: asset.modified_at,
                mirror_path: asset.mirror_path,
            })
            .collect())
    }
}

#[tauri::command]
pub async fn get_backend_connection(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    Ok(controller.status().await)
}

#[tauri::command]
pub async fn connect_remote_server(
    pairing_payload: String,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    controller.connect(&pairing_payload).await
}

#[tauri::command]
pub async fn use_local_backend(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    controller.use_local().await
}

#[tauri::command]
pub async fn use_remote_backend(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    controller.use_remote().await
}

#[tauri::command]
pub async fn forget_remote_server(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    controller.forget_remote().await
}

#[tauri::command]
pub async fn remote_json_request(
    request: RemoteRequest,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<serde_json::Value> {
    if !request.path.starts_with("/api/v1/") && request.path != "/api/v1/status" {
        return Err("Remote requests must use the versioned Volum API".into());
    }
    let (endpoint, token) = controller.endpoint_and_token().await?;
    controller
        .send_json(
            &endpoint,
            &token,
            &request.path,
            request.method.as_deref().unwrap_or("GET"),
            request.body.as_ref(),
        )
        .await
}

#[tauri::command]
pub async fn remote_binary_request(
    path: String,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<IpcResponse> {
    if !path.starts_with("/api/v1/") {
        return Err("Remote media requests must use the versioned Volum API".into());
    }
    let (endpoint, token) = controller.endpoint_and_token().await?;
    let response = controller
        .client
        .get(validated_url(&endpoint.base_url, &path)?)
        .bearer_auth(token)
        .send()
        .await
        .map_err(friendly_network_error)?;
    let status = response.status();
    if !status.is_success() {
        let bytes = response.bytes().await.unwrap_or_default();
        return Err(api_error_message(
            &bytes,
            "The remote media item is unavailable",
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BINARY_BYTES as u64)
    {
        return Err("The remote media item is too large".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "The remote media item could not be read".to_string())?;
    if bytes.len() > MAX_BINARY_BYTES {
        return Err("The remote media item is too large".into());
    }
    Ok(IpcResponse::new(bytes.to_vec()))
}

#[tauri::command]
pub async fn remote_upload_file(
    request: RemoteUploadRequest,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<serde_json::Value> {
    if request.project_id.is_some() == request.project_name.is_some() {
        return Err("Choose an existing project or a name for a new project".into());
    }
    let path = PathBuf::from(&request.file_path);
    let actual_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if request.filename.is_empty()
        || request.filename != actual_name
        || request.filename.contains(['/', '\\'])
        || !matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str(),
            "stl" | "3mf" | "obj" | "step" | "stp"
        )
    {
        return Err("Choose a supported 3D project file".into());
    }
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| "The selected project file is unavailable".to_string())?;
    let byte_size = file
        .metadata()
        .await
        .map_err(|error| error.to_string())?
        .len();
    if byte_size == 0 || byte_size > 500 * 1024 * 1024 {
        return Err("Project files must be non-empty and at most 500 MB".into());
    }
    let (endpoint, token) = controller.endpoint_and_token().await?;
    let mut url = validated_url(&endpoint.base_url, "/api/v1/uploads")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("filename", &request.filename);
        if let Some(project_id) = request.project_id.as_deref() {
            query.append_pair("projectId", project_id);
        }
        if let Some(project_name) = request.project_name.as_deref() {
            query.append_pair("projectName", project_name);
        }
    }
    let response = controller
        .client
        .post(url)
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .header(reqwest::header::CONTENT_LENGTH, byte_size)
        .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
        .send()
        .await
        .map_err(friendly_network_error)?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "The server returned an unreadable upload response".to_string())?;
    if !status.is_success() {
        return Err(api_error_message(
            &bytes,
            "The server rejected the project file",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "The server returned an unreadable upload response".into())
}

#[tauri::command]
pub async fn remote_upload_photo(
    request: RemotePhotoUploadRequest,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<serde_json::Value> {
    let path = PathBuf::from(&request.file_path);
    let actual_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if request.filename.is_empty()
        || request.filename != actual_name
        || request.project_id.is_empty()
        || request.if_unmodified_since.is_empty()
        || !matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str(),
            "jpg" | "jpeg" | "png" | "webp"
        )
    {
        return Err("Choose a supported project photo".into());
    }
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| "The selected project photo is unavailable".to_string())?;
    let byte_size = file
        .metadata()
        .await
        .map_err(|error| error.to_string())?
        .len();
    if byte_size == 0 || byte_size > 25 * 1024 * 1024 {
        return Err("Project photos must be non-empty and at most 25 MB".into());
    }
    let (endpoint, token) = controller.endpoint_and_token().await?;
    let request_path = format!(
        "/api/v1/projects/{}/photos",
        percent_encoding::utf8_percent_encode(
            &request.project_id,
            percent_encoding::NON_ALPHANUMERIC
        )
    );
    let mut url = validated_url(&endpoint.base_url, &request_path)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("filename", &request.filename);
        query.append_pair("caption", "");
        query.append_pair("ifUnmodifiedSince", &request.if_unmodified_since);
    }
    let response = controller
        .client
        .post(url)
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .header(reqwest::header::CONTENT_LENGTH, byte_size)
        .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
        .send()
        .await
        .map_err(friendly_network_error)?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "The server returned an unreadable photo response".to_string())?;
    if !status.is_success() {
        return Err(api_error_message(
            &bytes,
            "The server rejected the project photo",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "The server returned an unreadable photo response".into())
}

#[tauri::command]
pub async fn configure_remote_mirror(
    path: Option<String>,
    enabled: bool,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<BackendConnectionStatus> {
    controller.configure_mirror(path, enabled).await
}

#[tauri::command]
pub async fn sync_remote_mirror(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<MirrorSyncReport> {
    controller.sync_mirror().await
}

#[tauri::command]
pub async fn open_remote_asset(
    asset_id: String,
    app_path: Option<String>,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<()> {
    let path = controller.mirrored_asset_path(&asset_id).await?;
    if let Some(application) = app_path {
        crate::commands::open_with(&path, &application)
    } else {
        open::that(path).map_err(|error| error.to_string())
    }
}

#[tauri::command]
pub async fn reveal_remote_asset(
    asset_id: String,
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<()> {
    crate::commands::reveal(&controller.mirrored_asset_path(&asset_id).await?)
}

#[tauri::command]
pub async fn list_mirrored_assets(
    controller: tauri::State<'_, RemoteController>,
) -> RemoteResult<Vec<MirroredAsset>> {
    controller.mirrored_assets().await
}

fn default_mode() -> String {
    "local".into()
}

fn load_config(path: &Path) -> RemoteResult<BackendConfig> {
    if !path.exists() {
        return Ok(BackendConfig::default());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read remote connection settings: {error}"))?;
    let config: BackendConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Remote connection settings are damaged: {error}"))?;
    if !matches!(config.mode.as_str(), "local" | "remote") {
        return Err("Remote connection settings contain an invalid mode".into());
    }
    Ok(config)
}

fn persist_config(path: &Path, config: &BackendConfig) -> RemoteResult<()> {
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not save remote connection settings: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not protect remote connection settings: {error}"))?;
    }
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    std::fs::rename(temporary, path)
        .map_err(|error| format!("Could not activate remote connection settings: {error}"))
}

fn save_token(server_id: &str, token: &str) -> RemoteResult<()> {
    keyring::Entry::new(KEYRING_SERVICE, server_id)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?
        .set_password(token)
        .map_err(|error| format!("Could not protect the remote access token: {error}"))
}

fn load_token(server_id: &str) -> RemoteResult<String> {
    keyring::Entry::new(KEYRING_SERVICE, server_id)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?
        .get_password()
        .map_err(|_| "The remote access token is missing. Pair this Desktop again.".to_string())
}

fn delete_token(server_id: &str) -> RemoteResult<()> {
    keyring::Entry::new(KEYRING_SERVICE, server_id)
        .map_err(|error| error.to_string())?
        .delete_credential()
        .or_else(|error| match error {
            keyring::Error::NoEntry => Ok(()),
            error => Err(error),
        })
        .map_err(|error| error.to_string())
}

fn status_from_config(
    config: &BackendConfig,
    online: bool,
    connection_error: Option<String>,
) -> BackendConnectionStatus {
    let remote = config.remote.as_ref();
    let mirror = config.mirror.as_ref();
    BackendConnectionStatus {
        mode: config.mode.clone(),
        configured: remote.is_some(),
        online,
        server_id: remote.map(|value| value.server_id.clone()),
        server_name: remote.map(|value| value.server_name.clone()),
        base_url: remote.map(|value| value.base_url.clone()),
        mirror_enabled: mirror.is_some_and(|value| value.enabled),
        mirror_path: mirror.map(|value| value.path.clone()),
        last_sync_at: mirror.and_then(|value| value.last_sync_at.clone()),
        last_error: connection_error.or_else(|| mirror.and_then(|value| value.last_error.clone())),
    }
}

fn validate_base_url(value: &str) -> RemoteResult<String> {
    let url = reqwest::Url::parse(value.trim())
        .map_err(|_| "The Volum server address is invalid".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.host_str().is_none()
    {
        return Err("The Volum server address must contain only an http(s) origin".into());
    }
    let host = url.host_str().unwrap_or_default();
    if url.scheme() == "http" && !is_private_transport_host(host) {
        return Err(
            "Unencrypted Volum connections are allowed only over Tailscale or localhost".into(),
        );
    }
    Ok(url.origin().ascii_serialization())
}

fn validated_url(base_url: &str, path: &str) -> RemoteResult<reqwest::Url> {
    let base_url = validate_base_url(base_url)?;
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['\\', '\r', '\n']) {
        return Err("The remote request path is invalid".into());
    }
    reqwest::Url::parse(&base_url)
        .and_then(|base| base.join(path))
        .map_err(|_| "The remote request path is invalid".into())
}

fn is_private_transport_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let octets = ip.octets();
            ip.is_loopback() || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        }
        Ok(IpAddr::V6(ip)) => {
            let segments = ip.segments();
            ip.is_loopback()
                || (segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0)
        }
        Err(_) => host.eq_ignore_ascii_case("localhost") || host.ends_with(".ts.net"),
    }
}

fn friendly_network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "The Volum server did not respond in time".into()
    } else if error.is_connect() {
        "The Volum server could not be reached".into()
    } else {
        "The connection to the Volum server failed".into()
    }
}

fn api_error_message(bytes: &[u8], fallback: &str) -> String {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")?
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn desktop_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!("Volum Desktop on {name}"))
        .unwrap_or_else(|| "Volum Desktop".into())
}

fn safe_relative_path(value: &str) -> RemoteResult<PathBuf> {
    let path = Path::new(value);
    if value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("The server returned an unsafe mirror path".into());
    }
    Ok(path.to_path_buf())
}

fn checked_mirror_path(root: &Path, value: &str) -> RemoteResult<PathBuf> {
    checked_relative_path(root, &safe_relative_path(value)?)
}

fn checked_relative_path(root: &Path, relative: &Path) -> RemoteResult<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("The mirror path is invalid".into());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "The mirror folder is unavailable".to_string())?;
    let mut current = root.clone();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err("The mirror path is invalid".into());
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err("The mirror contains a symbolic link in a managed path".into());
                }
                if index + 1 < components.len() && !metadata.is_dir() {
                    return Err("A managed mirror path is not a directory".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect a managed mirror path: {error}")),
        }
    }
    Ok(current)
}

fn ensure_mirror_directory(root: &Path, relative: &Path) -> RemoteResult<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("The mirror directory is invalid".into());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "The mirror folder is unavailable".to_string())?;
    let mut current = root;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err("The mirror directory is invalid".into());
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("The mirror contains a symbolic link in a managed path".into());
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err("A managed mirror path is not a directory".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)
                    .map_err(|error| format!("Could not create a mirror directory: {error}"))?;
                let metadata = std::fs::symlink_metadata(&current)
                    .map_err(|error| format!("Could not inspect a mirror directory: {error}"))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("The mirror directory changed while it was being created".into());
                }
            }
            Err(error) => return Err(format!("Could not inspect a mirror directory: {error}")),
        }
    }
    Ok(current)
}

fn ensure_mirror_parent(root: &Path, path: &Path) -> RemoteResult<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| "The mirror path has no parent directory".to_string())?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| "The mirror path escaped its selected folder".to_string())?;
    ensure_mirror_directory(root, relative)
}

fn safe_identifier(value: &str) -> RemoteResult<String> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("The server returned an unsafe asset identity".into());
    }
    Ok(value.to_string())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn read_local_manifest(path: &Path, server_id: &str) -> RemoteResult<Option<LocalMirrorManifest>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read the local mirror manifest: {error}"))?;
    let manifest: LocalMirrorManifest = serde_json::from_slice(&bytes)
        .map_err(|_| "The local mirror manifest is damaged".to_string())?;
    if manifest.format_version != 1 {
        return Err("The local mirror manifest uses an unsupported format".into());
    }
    if !server_id.is_empty() && manifest.server_id != server_id {
        return Err("This mirror folder belongs to a different Volum server".into());
    }
    Ok(Some(manifest))
}

fn write_local_manifest(path: &Path, manifest: &LocalMirrorManifest) -> RemoteResult<()> {
    use std::io::Write;

    let temporary = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("Could not create the local mirror manifest: {error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not write the local mirror manifest: {error}"))?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    std::fs::rename(temporary, path)
        .map_err(|error| format!("Could not activate the local mirror manifest: {error}"))
}

async fn hash_path(path: &Path) -> RemoteResult<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
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
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn preserve_conflict(root: &Path, source: &Path, relative: &str) -> RemoteResult<PathBuf> {
    let relative = safe_relative_path(relative)?;
    let filename = relative
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let conflict_name = format!("{filename}.local-{stamp}");
    let mut conflict_relative = relative.clone();
    conflict_relative.set_file_name(conflict_name);
    let destination_relative = Path::new("Conflicts").join(conflict_relative);
    let destination = checked_relative_path(root, &destination_relative)?;
    ensure_mirror_parent(root, &destination)?;
    let destination = unique_conflict_path(destination).await;
    let destination_relative = destination
        .strip_prefix(root)
        .map_err(|_| "The conflict path escaped the mirror".to_string())?;
    let destination = checked_relative_path(root, destination_relative)?;
    tokio::fs::rename(source, &destination)
        .await
        .map_err(|error| format!("Could not preserve a locally changed mirror file: {error}"))?;
    Ok(destination)
}

async fn unique_conflict_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("asset.local");
    for index in 2..10_000 {
        let candidate = parent.join(format!("{filename}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

async fn activate_download(source: &Path, destination: &Path) -> RemoteResult<()> {
    #[cfg(not(windows))]
    {
        tokio::fs::rename(source, destination)
            .await
            .map_err(|error| format!("Could not activate a mirrored file: {error}"))
    }
    #[cfg(windows)]
    {
        if destination.exists() {
            tokio::fs::remove_file(destination)
                .await
                .map_err(|error| format!("Could not replace a mirrored file: {error}"))?;
        }
        tokio::fs::rename(source, destination)
            .await
            .map_err(|error| format!("Could not activate a mirrored file: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        companion::{CompanionController, HeadlessServerOptions},
        project_storage::{self, CreateManagedProjectInput},
        state::AppState,
    };
    use sqlx::Row;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use walkdir::WalkDir;

    #[test]
    fn remote_urls_require_https_or_tailnet_transport() {
        assert!(validate_base_url("http://100.64.0.2:43821").is_ok());
        assert!(validate_base_url("http://workshop.example.ts.net:43821").is_ok());
        assert!(validate_base_url("http://127.0.0.1:43821").is_ok());
        assert!(validate_base_url("https://volum.example.com").is_ok());
        assert!(validate_base_url("http://192.168.1.4:43821").is_err());
        assert!(validate_base_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn remote_request_paths_cannot_replace_the_server_authority() {
        let base = "http://100.64.0.2:43821";
        assert!(validated_url(base, "/api/v1/status").is_ok());
        assert!(validated_url(base, "//example.com/status").is_err());
        assert!(validated_url(base, "/\\example.com/status").is_err());
    }

    #[test]
    fn mirror_paths_cannot_escape_the_selected_directory() {
        assert_eq!(
            safe_relative_path("Projects/Lamp/part.stl").unwrap(),
            PathBuf::from("Projects/Lamp/part.stl")
        );
        assert!(safe_relative_path("../part.stl").is_err());
        assert!(safe_relative_path("/private/part.stl").is_err());
        assert!(safe_relative_path("Projects/./part.stl").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn mirror_paths_reject_nested_symbolic_links() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("mirror");
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(root.join("Projects")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("Projects/Redirected")).unwrap();

        assert!(checked_mirror_path(&root, "Projects/Redirected/part.stl").is_err());
        assert!(ensure_mirror_directory(&root, Path::new("Projects/Redirected/Nested")).is_err());
        assert!(!outside.join("Nested").exists());
    }

    #[tokio::test]
    async fn real_server_mirror_uploads_edits_and_preserves_conflicts() {
        let directory = tempfile::tempdir().unwrap();
        let server_data = directory.path().join("server-data");
        let library = directory.path().join("library");
        let client_data = directory.path().join("client-data");
        let mirror = directory.path().join("mirror");
        std::fs::create_dir_all(&client_data).unwrap();
        std::fs::create_dir_all(&mirror).unwrap();

        let state = AppState::initialize(&server_data).await.unwrap();
        crate::seed_defaults(&state).await.unwrap();
        project_storage::configure_managed_library_impl(
            library.to_string_lossy().to_string(),
            &state,
            None,
        )
        .await
        .unwrap();

        let original = test_stl("0");
        let source = directory.path().join("source.stl");
        std::fs::write(&source, &original).unwrap();
        let input = CreateManagedProjectInput {
            name: "Mirror test".into(),
            file_paths: vec![source.to_string_lossy().to_string()],
            asset_ids: Vec::new(),
            transfer_mode: "copy".into(),
        };
        let (project_id, _) = project_storage::create_managed_project_internal(&input, &state)
            .await
            .unwrap();
        let row = sqlx::query("SELECT a.id, r.path root_path, a.relative_path FROM assets a JOIN model_assets ma ON ma.asset_id = a.id JOIN library_roots r ON r.id = a.root_id WHERE ma.model_id = ? LIMIT 1")
            .bind(&project_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let asset_id: String = row.get("id");
        let canonical_path = PathBuf::from(row.get::<String, _>("root_path"))
            .join(row.get::<String, _>("relative_path"));

        let reservation =
            std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let base_url = format!("http://127.0.0.1:{port}");
        let server = CompanionController::new(&server_data, state.clone(), None).unwrap();
        server
            .start_headless(HeadlessServerOptions {
                bind: format!("127.0.0.1:{port}").parse().unwrap(),
                public_url: base_url.clone(),
                organization_writes: true,
            })
            .await
            .unwrap();

        let pairing = server.create_pairing().await.unwrap();
        let payload: PairingPayload = serde_json::from_str(&pairing.qr_payload).unwrap();
        let http = reqwest::Client::new();
        let paired = http
            .post(format!("{base_url}/pair"))
            .json(&serde_json::json!({
                "secret": payload.secret,
                "deviceName": "Mirror integration test",
                "platform": "test"
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<PairResponse>()
            .await
            .unwrap();

        let remote = RemoteController::new(&client_data).unwrap();
        {
            let mut inner = remote.inner.lock().await;
            inner.config = BackendConfig {
                mode: "remote".into(),
                remote: Some(RemoteEndpoint {
                    server_id: paired.server_id.clone(),
                    server_name: "Test server".into(),
                    base_url: base_url.clone(),
                    device_id: paired.device_id,
                }),
                mirror: Some(MirrorConfig {
                    path: mirror.to_string_lossy().to_string(),
                    enabled: true,
                    last_sync_at: None,
                    last_error: None,
                }),
            };
            persist_config(&inner.path, &inner.config).unwrap();
        }
        *remote.test_token.lock().await = Some(paired.token.clone());

        let initial = remote.sync_mirror().await.unwrap();
        assert_eq!(initial.downloaded, 1);
        assert_eq!(initial.conflicts, 0);
        let local_manifest = read_local_manifest(&mirror.join(".volum/manifest.json"), "")
            .unwrap()
            .unwrap();
        let mirrored = local_manifest
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .unwrap();
        let mirrored_path = mirror.join(safe_relative_path(&mirrored.mirror_path).unwrap());
        assert_eq!(std::fs::read(&mirrored_path).unwrap(), original);

        let local_edit = test_stl("1");
        std::fs::write(&mirrored_path, &local_edit).unwrap();
        let upload = remote.sync_mirror().await.unwrap();
        assert_eq!(upload.uploaded, 1);
        assert_eq!(upload.conflicts, 0);
        assert_eq!(std::fs::read(&canonical_path).unwrap(), local_edit);

        let after_upload = read_local_manifest(&mirror.join(".volum/manifest.json"), "")
            .unwrap()
            .unwrap();
        let uploaded_hash = after_upload
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .unwrap()
            .content_hash
            .clone();
        let server_edit = test_stl("2");
        let replacement = http
            .put(format!("{base_url}/api/v1/assets/{asset_id}/content"))
            .bearer_auth(&paired.token)
            .header(reqwest::header::IF_MATCH, format!("\"{uploaded_hash}\""))
            .body(server_edit.clone())
            .send()
            .await
            .unwrap();
        assert!(replacement.status().is_success());

        let conflicting_local_edit = test_stl("3");
        std::fs::write(&mirrored_path, &conflicting_local_edit).unwrap();
        let conflict = remote.sync_mirror().await.unwrap();
        assert_eq!(conflict.conflicts, 1);
        assert_eq!(conflict.downloaded, 1);
        assert_eq!(std::fs::read(&mirrored_path).unwrap(), server_edit);
        let preserved = WalkDir::new(mirror.join("Conflicts"))
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| entry.file_type().is_file())
            .expect("the local edit should be preserved in Conflicts");
        assert_eq!(
            std::fs::read(preserved.path()).unwrap(),
            conflicting_local_edit
        );

        server.stop().await;
        state.pool.close().await;
    }

    fn test_stl(offset: &str) -> Vec<u8> {
        format!(
            "solid mirror\n  facet normal 0 0 1\n    outer loop\n      vertex {offset} 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid mirror\n"
        )
        .into_bytes()
    }
}
