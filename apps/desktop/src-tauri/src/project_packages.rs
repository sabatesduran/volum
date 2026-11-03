use tauri::{AppHandle, State};
use volum_server::{project_packages as core, project_packages::*, state::AppState};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn inspect_project_package(
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectPackagePreview> {
    core::inspect_project_package(path, state.inner()).await
}

#[tauri::command]
pub async fn export_project_package(
    model_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<()> {
    core::export_project_package(model_id, path, state.inner()).await
}

#[tauri::command]
pub async fn import_project_package(
    input: ProjectPackageImportInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String> {
    core::import_project_package(input, state.inner(), crate::event_emitter(app)).await
}
