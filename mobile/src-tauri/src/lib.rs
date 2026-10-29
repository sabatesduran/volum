#[cfg(not(target_os = "android"))]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures_util::StreamExt;
#[cfg(not(target_os = "android"))]
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::IpAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{ipc::Channel, Manager};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use tokio::sync::{watch, Mutex};
use tokio_util::io::ReaderStream;

const MAX_JSON_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const MAX_MEDIA_RESPONSE_BYTES: usize = 30 * 1024 * 1024;
const MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;
const MAX_PHOTO_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;
const MEDIA_ENVELOPE_MAGIC: &[u8; 4] = b"VMC1";
#[cfg(target_os = "ios")]
const VAULT_KEYRING_SERVICE: &str = "dev.didac.volum.companion.vault";
#[cfg(target_os = "ios")]
const VAULT_KEYRING_USER: &str = "stronghold-password";

#[derive(Clone)]
struct MobileState {
    client: reqwest::Client,
    upload_client: reqwest::Client,
    uploads: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanionRequest {
    base_url: String,
    path: String,
    token: Option<String>,
    method: Option<String>,
    body: Option<serde_json::Value>,
    if_none_match: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionResponse {
    status: u16,
    content_type: Option<String>,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanionUploadRequest {
    base_url: String,
    token: String,
    file_path: FilePath,
    filename: String,
    project_id: Option<String>,
    project_name: Option<String>,
    upload_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanionPhotoUploadRequest {
    base_url: String,
    token: String,
    file_path: FilePath,
    filename: String,
    project_id: String,
    caption: String,
    if_unmodified_since: String,
    upload_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadProgress {
    uploaded_bytes: u64,
    total_bytes: u64,
}

struct UploadConstraints<'a> {
    maximum_bytes: u64,
    empty_message: &'a str,
    too_large_message: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResult {
    project_id: String,
    asset_id: String,
    filename: String,
    byte_size: u64,
}

#[tauri::command]
async fn companion_json_request(
    request: CompanionRequest,
    state: tauri::State<'_, MobileState>,
) -> Result<CompanionResponse, String> {
    let url = validated_url(&request.base_url, &request.path)?;
    let method = request
        .method
        .as_deref()
        .unwrap_or(if request.body.is_some() {
            "POST"
        } else {
            "GET"
        });
    let mut builder = match method {
        "GET" => state.client.get(url),
        "POST" => state.client.post(url),
        "PUT" => state.client.put(url),
        "PATCH" => state.client.patch(url),
        "DELETE" => state.client.delete(url),
        _ => return Err("The mobile request method is invalid".into()),
    };
    if method == "GET" && request.body.is_some() {
        return Err("Read-only requests cannot include changes".into());
    }
    if let Some(token) = request.token.as_deref() {
        builder = builder.bearer_auth(token);
    }
    if let Some(body) = request.body.as_ref() {
        builder = builder.json(body);
    }
    let response = builder
        .send()
        .await
        .map_err(|error| friendly_network_error(&error))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if response
        .content_length()
        .is_some_and(|length| length > MAX_JSON_RESPONSE_BYTES as u64)
    {
        return Err("Volum Desktop returned too much data".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Volum Desktop returned an unreadable response".to_string())?;
    if bytes.len() > MAX_JSON_RESPONSE_BYTES {
        return Err("Volum Desktop returned too much data".into());
    }
    let body = String::from_utf8(bytes.to_vec())
        .map_err(|_| "Volum Desktop returned an unreadable response".to_string())?;
    Ok(CompanionResponse {
        status,
        content_type,
        body,
    })
}

#[tauri::command]
async fn companion_media_request(
    request: CompanionRequest,
    state: tauri::State<'_, MobileState>,
) -> Result<tauri::ipc::Response, String> {
    if request.body.is_some() {
        return Err("Media requests must be read-only".into());
    }
    let url = validated_url(&request.base_url, &request.path)?;
    let mut builder = state.client.get(url);
    if let Some(token) = request.token.as_deref() {
        builder = builder.bearer_auth(token);
    }
    if let Some(etag) = request.if_none_match.as_deref() {
        if etag.len() > 160 || etag.chars().any(char::is_control) {
            return Err("The cached media identity is invalid".into());
        }
        builder = builder.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = builder
        .send()
        .await
        .map_err(|error| friendly_network_error(&error))?;
    let status = response.status();
    if status != reqwest::StatusCode::NOT_MODIFIED && !status.is_success() {
        return Err(match response.status().as_u16() {
            401 => "This device is no longer paired with Volum Desktop".into(),
            404 => "This media item is not available".into(),
            _ => "Volum Desktop could not return this media item".into(),
        });
    }
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MEDIA_RESPONSE_BYTES as u64)
    {
        return Err("This media item is too large for the companion".into());
    }
    let bytes = if status == reqwest::StatusCode::NOT_MODIFIED {
        Vec::new()
    } else {
        response
            .bytes()
            .await
            .map_err(|_| "Volum Desktop returned unreadable media".to_string())?
            .to_vec()
    };
    if bytes.len() > MAX_MEDIA_RESPONSE_BYTES {
        return Err("This media item is too large for the companion".into());
    }
    Ok(tauri::ipc::Response::new(encode_media_envelope(
        status.as_u16(),
        etag.as_deref(),
        content_type.as_deref(),
        &bytes,
    )?))
}

fn encode_media_envelope(
    status: u16,
    etag: Option<&str>,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let etag = etag.unwrap_or_default().as_bytes();
    let content_type = content_type.unwrap_or_default().as_bytes();
    let etag_length = u16::try_from(etag.len()).map_err(|_| "The media identity is too long")?;
    let content_type_length =
        u16::try_from(content_type.len()).map_err(|_| "The media type is too long")?;
    let mut envelope = Vec::with_capacity(10 + etag.len() + content_type.len() + body.len());
    envelope.extend_from_slice(MEDIA_ENVELOPE_MAGIC);
    envelope.extend_from_slice(&status.to_le_bytes());
    envelope.extend_from_slice(&etag_length.to_le_bytes());
    envelope.extend_from_slice(&content_type_length.to_le_bytes());
    envelope.extend_from_slice(etag);
    envelope.extend_from_slice(content_type);
    envelope.extend_from_slice(body);
    Ok(envelope)
}

#[tauri::command]
async fn companion_upload_file(
    request: CompanionUploadRequest,
    progress: Channel<UploadProgress>,
    app: tauri::AppHandle,
    state: tauri::State<'_, MobileState>,
) -> Result<UploadResult, String> {
    let filename = validate_upload_filename(&request.filename)?;
    if request.token.is_empty() {
        return Err("Pair this device with Volum Desktop before uploading".into());
    }
    if request.project_id.is_some() == request.project_name.is_some() {
        return Err("Choose an existing project or enter a name for a new project".into());
    }
    let mut url = validated_url(&request.base_url, "/api/v1/uploads")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("filename", &filename);
        if let Some(project_id) = request.project_id.as_deref() {
            query.append_pair("projectId", project_id);
        }
        if let Some(project_name) = request.project_name.as_deref() {
            query.append_pair("projectName", project_name);
        }
    }
    let mut options = OpenOptions::new();
    options.read(true);
    let file_path = request.file_path.clone();
    let file = open_selected_file(&app, &file_path, options)
        .map_err(|_| "The selected file is no longer available".to_string())?;
    let cancellation = register_upload(&request.upload_id, &state).await?;
    let result = upload_open_file(
        file,
        url,
        &request.token,
        progress,
        &state.upload_client,
        cancellation,
    )
    .await;
    state.uploads.lock().await.remove(&request.upload_id);
    #[cfg(target_os = "ios")]
    if let FilePath::Url(url) = file_path {
        let _ = app.fs().stop_accessing_security_scoped_resource(url);
    }
    result
}

#[tauri::command]
async fn companion_upload_photo(
    request: CompanionPhotoUploadRequest,
    progress: Channel<UploadProgress>,
    app: tauri::AppHandle,
    state: tauri::State<'_, MobileState>,
) -> Result<serde_json::Value, String> {
    let filename = validate_photo_filename(&request.filename)?;
    if request.token.is_empty() || request.project_id.is_empty() {
        return Err("Pair this device and choose a project before uploading".into());
    }
    if request.caption.chars().count() > 500 || request.if_unmodified_since.is_empty() {
        return Err("Refresh the project and enter a shorter photo caption".into());
    }
    let mut url = validated_url(&request.base_url, "/")?;
    url.path_segments_mut()
        .map_err(|_| "The project photo upload path is invalid".to_string())?
        .clear()
        .push("api")
        .push("v1")
        .push("projects")
        .push(&request.project_id)
        .push("photos");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("filename", &filename);
        query.append_pair("caption", &request.caption);
        query.append_pair("ifUnmodifiedSince", &request.if_unmodified_since);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    let file_path = request.file_path.clone();
    let file = open_selected_file(&app, &file_path, options)
        .map_err(|_| "The selected photo is no longer available".to_string())?;
    let cancellation = register_upload(&request.upload_id, &state).await?;
    let result = upload_open_file_json(
        file,
        url,
        &request.token,
        progress,
        &state.upload_client,
        UploadConstraints {
            maximum_bytes: MAX_PHOTO_UPLOAD_BYTES,
            empty_message: "Choose a non-empty photo",
            too_large_message: "Project photos can be at most 25 MB",
        },
        cancellation,
    )
    .await;
    state.uploads.lock().await.remove(&request.upload_id);
    #[cfg(target_os = "ios")]
    if let FilePath::Url(url) = file_path {
        let _ = app.fs().stop_accessing_security_scoped_resource(url);
    }
    result
}

#[tauri::command]
async fn companion_cancel_upload(
    upload_id: String,
    state: tauri::State<'_, MobileState>,
) -> Result<bool, String> {
    validate_upload_id(&upload_id)?;
    let uploads = state.uploads.lock().await;
    let Some(cancellation) = uploads.get(&upload_id) else {
        return Ok(false);
    };
    cancellation
        .send(true)
        .map_err(|_| "The upload has already finished".to_string())?;
    Ok(true)
}

async fn register_upload(
    upload_id: &str,
    state: &MobileState,
) -> Result<watch::Receiver<bool>, String> {
    validate_upload_id(upload_id)?;
    let mut uploads = state.uploads.lock().await;
    if uploads.contains_key(upload_id) {
        return Err("That upload is already running".into());
    }
    let (sender, receiver) = watch::channel(false);
    uploads.insert(upload_id.to_string(), sender);
    Ok(receiver)
}

async fn upload_open_file(
    file: std::fs::File,
    url: reqwest::Url,
    token: &str,
    progress: Channel<UploadProgress>,
    client: &reqwest::Client,
    cancellation: watch::Receiver<bool>,
) -> Result<UploadResult, String> {
    let value = upload_open_file_json(
        file,
        url,
        token,
        progress,
        client,
        UploadConstraints {
            maximum_bytes: MAX_UPLOAD_BYTES,
            empty_message: "Choose a non-empty 3D file",
            too_large_message: "3D files can be at most 500 MB",
        },
        cancellation,
    )
    .await?;
    serde_json::from_value(value)
        .map_err(|_| "Volum Desktop returned an unreadable upload response".to_string())
}

async fn upload_open_file_json(
    file: std::fs::File,
    url: reqwest::Url,
    token: &str,
    progress: Channel<UploadProgress>,
    client: &reqwest::Client,
    constraints: UploadConstraints<'_>,
    mut cancellation: watch::Receiver<bool>,
) -> Result<serde_json::Value, String> {
    let byte_size = file
        .metadata()
        .map_err(|_| "The selected file could not be inspected".to_string())?
        .len();
    if byte_size == 0 {
        return Err(constraints.empty_message.into());
    }
    if byte_size > constraints.maximum_bytes {
        return Err(constraints.too_large_message.into());
    }
    let sent = Arc::new(AtomicU64::new(0));
    let progress_channel = progress.clone();
    let sent_bytes = sent.clone();
    let stream_cancellation = cancellation.clone();
    let stream = ReaderStream::new(tokio::fs::File::from_std(file)).map(move |chunk| {
        if *stream_cancellation.borrow() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "upload canceled",
            ));
        }
        if let Ok(bytes) = &chunk {
            let uploaded_bytes =
                sent_bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed) + bytes.len() as u64;
            let _ = progress_channel.send(UploadProgress {
                uploaded_bytes,
                total_bytes: byte_size,
            });
        }
        chunk
    });
    let _ = progress.send(UploadProgress {
        uploaded_bytes: 0,
        total_bytes: byte_size,
    });
    let request = client
        .post(url)
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .header(reqwest::header::CONTENT_LENGTH, byte_size)
        .body(reqwest::Body::wrap_stream(stream))
        .send();
    tokio::pin!(request);
    let response = tokio::select! {
        response = &mut request => response.map_err(|error| friendly_network_error(&error))?,
        () = wait_for_cancellation(&mut cancellation) => return Err("Upload canceled".into()),
    };
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_JSON_RESPONSE_BYTES as u64)
    {
        return Err("Volum Desktop returned too much data".into());
    }
    let response_bytes = response.bytes();
    tokio::pin!(response_bytes);
    let bytes = tokio::select! {
        bytes = &mut response_bytes => bytes.map_err(|_| "Volum Desktop returned an unreadable upload response".to_string())?,
        () = wait_for_cancellation(&mut cancellation) => return Err("Upload canceled".into()),
    };
    if bytes.len() > MAX_JSON_RESPONSE_BYTES {
        return Err("Volum Desktop returned too much data".into());
    }
    if !status.is_success() {
        let message = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("Volum Desktop rejected this upload ({status})"));
        return Err(message);
    }
    let result = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|_| "Volum Desktop returned an unreadable upload response".to_string())?;
    let _ = progress.send(UploadProgress {
        uploaded_bytes: byte_size,
        total_bytes: byte_size,
    });
    Ok(result)
}

async fn wait_for_cancellation(receiver: &mut watch::Receiver<bool>) {
    if *receiver.borrow_and_update() {
        return;
    }
    while receiver.changed().await.is_ok() {
        if *receiver.borrow_and_update() {
            return;
        }
    }
}

fn validate_upload_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("The upload identity is invalid".into());
    }
    Ok(())
}

fn validate_upload_filename(value: &str) -> Result<String, String> {
    let filename = PathBuf::from(value);
    let extension = filename
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if value.trim() != value
        || value.is_empty()
        || filename.file_name().and_then(|value| value.to_str()) != Some(value)
        || !matches!(extension.as_str(), "stl" | "3mf" | "obj" | "step" | "stp")
    {
        return Err("Choose an STL, 3MF, OBJ, STEP, or STP file".into());
    }
    Ok(value.to_string())
}

fn validate_photo_filename(value: &str) -> Result<String, String> {
    let filename = PathBuf::from(value);
    let extension = filename
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if value.trim() != value
        || value.is_empty()
        || filename.file_name().and_then(|value| value.to_str()) != Some(value)
        || !matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp")
    {
        return Err("Choose a JPEG, PNG, or WebP photo".into());
    }
    Ok(value.to_string())
}

fn open_selected_file(
    app: &tauri::AppHandle,
    file_path: &FilePath,
    options: OpenOptions,
) -> std::io::Result<std::fs::File> {
    match file_path {
        FilePath::Path(path) => {
            if !app.fs_scope().is_allowed(path) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "the file was not selected through the system picker",
                ));
            }
        }
        FilePath::Url(url) if url.scheme() == "file" => {
            let path = url.to_file_path().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid file URL")
            })?;
            if !app.fs_scope().is_allowed(&path) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "the file was not selected through the system picker",
                ));
            }
        }
        FilePath::Url(url) if url.scheme() == "content" => {
            // Android's content resolver independently enforces the picker grant.
        }
        FilePath::Url(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported selected-file URL",
            ));
        }
    }
    app.fs().open(file_path.clone(), options)
}

#[tauri::command]
fn vault_password(app: tauri::AppHandle) -> Result<String, String> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join("vault.key");
    let legacy = path
        .exists()
        .then(|| std::fs::read_to_string(&path))
        .transpose()
        .map_err(|error| error.to_string())?;

    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let password = platform_vault_password(legacy.as_deref())?;
        if legacy.is_some() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("Could not remove the migrated vault key: {error}"))?;
        }
        return Ok(password);
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if let Some(password) = legacy {
        return Ok(password);
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let password = random_vault_password();
        write_private(&path, password.as_bytes())?;
        Ok(password)
    }
}

#[cfg(not(target_os = "android"))]
fn random_vault_password() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(target_os = "ios")]
fn platform_vault_password(legacy: Option<&str>) -> Result<String, String> {
    let entry = keyring::Entry::new(VAULT_KEYRING_SERVICE, VAULT_KEYRING_USER)
        .map_err(|error| format!("Could not access the iOS Keychain: {error}"))?;
    match entry.get_password() {
        Ok(password) => Ok(password),
        Err(keyring::Error::NoEntry) => {
            let password = legacy
                .map(str::to_owned)
                .unwrap_or_else(random_vault_password);
            entry
                .set_password(&password)
                .map_err(|error| format!("Could not protect the vault key in Keychain: {error}"))?;
            Ok(password)
        }
        Err(error) => Err(format!(
            "Could not read the vault key from Keychain: {error}"
        )),
    }
}

#[cfg(target_os = "android")]
fn platform_vault_password(legacy: Option<&str>) -> Result<String, String> {
    use jni::objects::{JObject, JString, JValue};

    let context = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }
        .map_err(|error| format!("Could not access Android secure storage: {error}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| format!("Could not access Android secure storage: {error}"))?;
    let context_object = unsafe { JObject::from_raw(context.context().cast()) };
    let class = env
        .get_object_class(&context_object)
        .map_err(|error| format!("Could not access Android secure storage: {error}"))?;
    let legacy = env
        .new_string(legacy.unwrap_or_default())
        .map_err(|error| format!("Could not migrate the Android vault key: {error}"))?;
    let legacy_object = JObject::from(legacy);
    let value = env
        .call_static_method(
            class,
            "getOrCreateVaultPassword",
            "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&context_object),
                JValue::Object(&legacy_object),
            ],
        )
        .and_then(|value| value.l())
        .map_err(|error| format!("Could not read the Android vault key: {error}"))?;
    let password = env
        .get_string(&JString::from(value))
        .map_err(|error| format!("Could not read the Android vault key: {error}"))?
        .into();
    std::mem::forget(context_object);
    Ok(password)
}

fn validated_url(base_url: &str, path: &str) -> Result<reqwest::Url, String> {
    let base = reqwest::Url::parse(base_url).map_err(|_| "The Tailnet address is invalid")?;
    if !matches!(base.scheme(), "http" | "https")
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || base.path() != "/"
    {
        return Err("The Tailnet address is invalid".into());
    }
    let host = base
        .host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let is_tailnet = match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let octets = ip.octets();
            octets[0] == 100 && (64..=127).contains(&octets[1])
        }
        Ok(IpAddr::V6(ip)) => {
            let segments = ip.segments();
            segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
        }
        Err(_) => host.ends_with(".ts.net"),
    };
    if !is_tailnet {
        return Err("Volum Companion only connects to Tailscale addresses".into());
    }
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['\\', '\r', '\n']) {
        return Err("The companion request path is invalid".into());
    }
    base.join(path)
        .map_err(|_| "The companion request path is invalid".into())
}

fn friendly_network_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "Volum Desktop did not respond. Check that it is open and Companion Access is on.".into()
    } else if error.is_connect() {
        "Could not reach Volum Desktop through Tailscale.".into()
    } else {
        "The connection to Volum Desktop failed.".into()
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("could not initialize companion networking");
    let upload_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(15 * 60))
        .build()
        .expect("could not initialize companion uploads");
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(MobileState {
            client,
            upload_client,
            uploads: Arc::new(Mutex::new(HashMap::new())),
        })
        .setup(|app| {
            let salt_path = app
                .path()
                .app_local_data_dir()
                .map_err(|error| error.to_string())?
                .join("stronghold.salt");
            app.handle()
                .plugin(tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build())?;
            Ok(())
        });
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    builder
        .invoke_handler(tauri::generate_handler![
            companion_json_request,
            companion_media_request,
            companion_upload_file,
            companion_upload_photo,
            companion_cancel_upload,
            vault_password
        ])
        .run(tauri::generate_context!())
        .expect("error while running Volum Companion");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_cgnat_tailnet_addresses() {
        assert!(validated_url("http://100.64.0.1:43821", "/api/v1/status").is_ok());
    }

    #[test]
    fn rejects_lan_and_public_addresses() {
        assert!(validated_url("http://192.168.1.2:43821", "/api/v1/status").is_err());
        assert!(validated_url("https://example.com", "/api/v1/status").is_err());
    }

    #[test]
    fn allows_tailnet_ipv6_and_magic_dns() {
        assert!(validated_url("http://[fd7a:115c:a1e0::1]:43821", "/api/v1/status").is_ok());
        assert!(validated_url(
            "http://volum-desktop.example.ts.net:43821",
            "/api/v1/status"
        )
        .is_ok());
    }

    #[test]
    fn rejects_credentials_and_authority_replacement() {
        assert!(validated_url("http://user:password@100.64.0.1:43821", "/api/v1/status").is_err());
        assert!(validated_url("http://100.64.0.1:43821", "//example.com/status").is_err());
        assert!(validated_url("http://100.64.0.1:43821", "/\\example.com/status").is_err());
    }

    #[test]
    fn accepts_only_supported_upload_filenames() {
        assert_eq!(
            validate_upload_filename("printer part.3MF").unwrap(),
            "printer part.3MF"
        );
        assert!(validate_upload_filename("../part.stl").is_err());
        assert!(validate_upload_filename("part.zip").is_err());
    }

    #[test]
    fn accepts_only_bounded_upload_identifiers() {
        assert!(validate_upload_id("6e136264-ff20-4cec-a138-8ae7f5cd84e2").is_ok());
        assert!(validate_upload_id("").is_err());
        assert!(validate_upload_id("../../other-upload").is_err());
        assert!(validate_upload_id(&"a".repeat(101)).is_err());
    }

    #[test]
    fn media_envelope_preserves_status_headers_and_binary_body() {
        let envelope =
            encode_media_envelope(200, Some("\"revision\""), Some("image/png"), &[0, 1, 255])
                .unwrap();
        assert_eq!(&envelope[..4], MEDIA_ENVELOPE_MAGIC);
        assert_eq!(u16::from_le_bytes([envelope[4], envelope[5]]), 200);
        assert_eq!(u16::from_le_bytes([envelope[6], envelope[7]]), 10);
        assert_eq!(u16::from_le_bytes([envelope[8], envelope[9]]), 9);
        assert_eq!(&envelope[10..20], b"\"revision\"");
        assert_eq!(&envelope[20..29], b"image/png");
        assert_eq!(&envelope[29..], &[0, 1, 255]);
    }

    #[tokio::test]
    async fn cancellation_signal_interrupts_upload_waiters() {
        let (sender, mut receiver) = watch::channel(false);
        sender.send(true).unwrap();
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            wait_for_cancellation(&mut receiver),
        )
        .await
        .expect("cancellation should be observed immediately");
    }
}
