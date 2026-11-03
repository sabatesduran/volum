use tauri::{ipc::Response, State};
use volum_server::remote::{self as core, *};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn get_backend_connection(
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::get_backend_connection(controller.inner()).await
}

#[tauri::command]
pub async fn connect_remote_server(
    pairing_payload: String,
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::connect_remote_server(pairing_payload, controller.inner()).await
}

#[tauri::command]
pub async fn use_local_backend(
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::use_local_backend(controller.inner()).await
}

#[tauri::command]
pub async fn use_remote_backend(
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::use_remote_backend(controller.inner()).await
}

#[tauri::command]
pub async fn forget_remote_server(
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::forget_remote_server(controller.inner()).await
}

#[tauri::command]
pub async fn remote_json_request(
    request: RemoteRequest,
    controller: State<'_, RemoteController>,
) -> Result<serde_json::Value> {
    core::remote_json_request(request, controller.inner()).await
}

#[tauri::command]
pub async fn remote_binary_request(
    path: String,
    controller: State<'_, RemoteController>,
) -> Result<Response> {
    core::remote_binary_request(path, controller.inner())
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn remote_upload_file(
    request: RemoteUploadRequest,
    controller: State<'_, RemoteController>,
) -> Result<serde_json::Value> {
    core::remote_upload_file(request, controller.inner()).await
}

#[tauri::command]
pub async fn remote_upload_photo(
    request: RemotePhotoUploadRequest,
    controller: State<'_, RemoteController>,
) -> Result<serde_json::Value> {
    core::remote_upload_photo(request, controller.inner()).await
}

#[tauri::command]
pub async fn configure_remote_mirror(
    path: Option<String>,
    enabled: bool,
    mode: Option<String>,
    controller: State<'_, RemoteController>,
) -> Result<BackendConnectionStatus> {
    core::configure_remote_mirror(path, enabled, mode, controller.inner()).await
}

#[tauri::command]
pub async fn get_remote_mirror_settings(
    controller: State<'_, RemoteController>,
) -> Result<RemoteMirrorSettings> {
    core::get_remote_mirror_settings(controller.inner()).await
}

#[tauri::command]
pub async fn set_remote_mirror_mode(
    mode: String,
    controller: State<'_, RemoteController>,
) -> Result<RemoteMirrorSettings> {
    core::set_remote_mirror_mode(mode, controller.inner()).await
}

#[tauri::command]
pub async fn set_remote_mirror_selection(
    input: MirrorSelectionInput,
    controller: State<'_, RemoteController>,
) -> Result<RemoteMirrorSettings> {
    core::set_remote_mirror_selection(input, controller.inner()).await
}

#[tauri::command]
pub async fn sync_remote_mirror(
    controller: State<'_, RemoteController>,
) -> Result<MirrorSyncReport> {
    core::sync_remote_mirror(controller.inner()).await
}

#[tauri::command]
pub async fn open_remote_asset(
    asset_id: String,
    app_path: Option<String>,
    controller: State<'_, RemoteController>,
) -> Result<()> {
    core::open_remote_asset(asset_id, app_path, controller.inner()).await
}

#[tauri::command]
pub async fn reveal_remote_asset(
    asset_id: String,
    controller: State<'_, RemoteController>,
) -> Result<()> {
    core::reveal_remote_asset(asset_id, controller.inner()).await
}

#[tauri::command]
pub async fn list_mirrored_assets(
    controller: State<'_, RemoteController>,
) -> Result<Vec<MirroredAsset>> {
    core::list_mirrored_assets(controller.inner()).await
}

#[tauri::command]
pub async fn list_mirrored_projects(
    controller: State<'_, RemoteController>,
) -> Result<Vec<MirroredProject>> {
    core::list_mirrored_projects(controller.inner()).await
}
