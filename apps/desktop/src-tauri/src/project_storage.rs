use tauri::{AppHandle, State};
use volum_server::{
    domain::LibraryRoot, project_storage as core, project_storage::*, state::AppState,
};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn get_managed_library_status(
    state: State<'_, AppState>,
) -> Result<ManagedLibraryStatus> {
    core::get_managed_library_status(state.inner()).await
}

#[tauri::command]
pub async fn configure_managed_library(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<LibraryRoot> {
    core::configure_managed_library(path, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn list_library_assets(state: State<'_, AppState>) -> Result<Vec<LibraryAssetCandidate>> {
    core::list_library_assets(state.inner()).await
}

#[tauri::command]
pub async fn import_managed_inbox(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InboxImportResult> {
    core::import_managed_inbox(state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn create_managed_project(
    input: CreateManagedProjectInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String> {
    core::create_managed_project(input, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn add_files_to_project(
    input: AddProjectFilesInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<String>> {
    core::add_files_to_project(input, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn rename_project(
    project_id: String,
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::rename_project(project_id, name, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn rename_project_file(
    asset_id: String,
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::rename_project_file(asset_id, name, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn convert_project_to_managed(
    input: ConvertProjectInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::convert_project_to_managed(input, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn preview_project_consolidation(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ConsolidationPreview> {
    core::preview_project_consolidation(project_id, state.inner()).await
}

#[tauri::command]
pub async fn trash_project(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::trash_project(project_id, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn restore_project_from_trash(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::restore_project_from_trash(project_id, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn delete_project_permanently(
    project_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::delete_project_permanently(project_id, state.inner(), crate::event_emitter(app)).await
}
