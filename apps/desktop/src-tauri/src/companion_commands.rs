use tauri::State;
use volum_server::companion::{CompanionController, CompanionPairingInfo, CompanionStatus};

#[tauri::command]
pub async fn get_companion_status(
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    Ok(controller.status().await)
}

#[tauri::command]
pub async fn set_companion_enabled(
    enabled: bool,
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    controller.set_enabled(enabled).await
}

#[tauri::command]
pub async fn set_companion_organization_writes(
    enabled: bool,
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    controller.set_organization_writes(enabled).await
}

#[tauri::command]
pub async fn set_companion_photo_uploads(
    enabled: bool,
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    controller.set_photo_uploads(enabled).await
}

#[tauri::command]
pub async fn set_companion_network(
    mode: String,
    bind_address: Option<String>,
    public_url: Option<String>,
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    controller
        .set_network(&mode, bind_address, public_url)
        .await
}

#[tauri::command]
pub async fn create_companion_pairing(
    controller: State<'_, CompanionController>,
) -> Result<CompanionPairingInfo, String> {
    controller.create_pairing().await
}

#[tauri::command]
pub async fn revoke_companion_device(
    device_id: String,
    controller: State<'_, CompanionController>,
) -> Result<CompanionStatus, String> {
    controller.revoke_device(&device_id).await
}
