use tauri::{AppHandle, State};
use volum_server::{
    companion::CompanionController,
    library_migration::{self as core, *},
    remote::{BackendConnectionStatus, RemoteController},
    state::AppState,
};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn inspect_library_migration(state: State<'_, AppState>) -> Result<MigrationPreflight> {
    core::inspect_library_migration(state.inner()).await
}

#[tauri::command]
pub async fn get_pending_library_migration(
    state: State<'_, AppState>,
) -> Result<Option<PendingMigration>> {
    core::get_pending_library_migration(state.inner()).await
}

#[tauri::command]
pub async fn start_library_migration(
    pairing_payload: String,
    allow_missing: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> Result<BackendConnectionStatus> {
    core::start_library_migration(
        pairing_payload,
        allow_missing,
        state.inner(),
        remote.inner(),
        companion.inner(),
        crate::event_emitter(app),
    )
    .await
}

#[tauri::command]
pub async fn resume_library_migration(
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> Result<BackendConnectionStatus> {
    core::resume_library_migration(
        state.inner(),
        remote.inner(),
        companion.inner(),
        crate::event_emitter(app),
    )
    .await
}

#[tauri::command]
pub async fn abandon_library_migration(
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    companion: State<'_, CompanionController>,
    app: AppHandle,
) -> Result<BackendConnectionStatus> {
    core::abandon_library_migration(
        state.inner(),
        remote.inner(),
        companion.inner(),
        crate::event_emitter(app),
    )
    .await
}

#[tauri::command]
pub async fn rollback_library_migration(
    nas_stopped: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteController>,
    app: AppHandle,
) -> Result<BackendConnectionStatus> {
    core::rollback_library_migration(
        nas_stopped,
        state.inner(),
        remote.inner(),
        crate::event_emitter(app),
    )
    .await
}
