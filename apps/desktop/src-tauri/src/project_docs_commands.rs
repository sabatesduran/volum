use std::path::Path;
use tauri::{ipc::Response, State};
use volum_server::{
    domain::{
        AssemblyStep, CustomFieldDefinition, ProjectBomItem, ProjectDocumentation, ProjectPhoto,
    },
    project_docs::{
        self, AssemblyStepInput, CustomFieldInput, ProjectBomItemInput, ProjectCustomValueInput,
        ProjectPhotoUpdate,
    },
    state::AppState,
};

#[tauri::command]
pub async fn get_project_documentation(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectDocumentation, String> {
    project_docs::documentation_for_project(&model_id, &state.pool).await
}

#[tauri::command]
pub async fn add_project_photo(
    model_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectPhoto, String> {
    let _guard = state.file_operation_lock.lock().await;
    project_docs::store_project_photo(&model_id, Path::new(&path), None, "", None, None, &state)
        .await
}

#[tauri::command]
pub async fn update_project_photo(
    input: ProjectPhotoUpdate,
    state: State<'_, AppState>,
) -> Result<ProjectPhoto, String> {
    project_docs::update_project_photo_impl(input, state.inner()).await
}

#[tauri::command]
pub async fn delete_project_photo(
    photo_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.file_operation_lock.lock().await;
    project_docs::delete_project_photo_impl(&photo_id, state.inner()).await
}

#[tauri::command]
pub async fn get_project_photo(
    photo_id: String,
    thumbnail: bool,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let (bytes, _) = project_docs::project_photo_bytes(&photo_id, thumbnail, state.inner()).await?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn list_custom_field_definitions(
    state: State<'_, AppState>,
) -> Result<Vec<CustomFieldDefinition>, String> {
    project_docs::custom_field_definitions(&state.pool).await
}

#[tauri::command]
pub async fn save_custom_field_definition(
    input: CustomFieldInput,
    state: State<'_, AppState>,
) -> Result<CustomFieldDefinition, String> {
    project_docs::save_custom_field_definition_impl(input, &state).await
}

#[tauri::command]
pub async fn delete_custom_field_definition(
    field_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_docs::delete_custom_field_definition_impl(&field_id, &state).await
}

#[tauri::command]
pub async fn set_project_custom_value(
    input: ProjectCustomValueInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_docs::set_project_custom_value_impl(input, state.inner()).await
}

#[tauri::command]
pub async fn save_project_bom_item(
    input: ProjectBomItemInput,
    state: State<'_, AppState>,
) -> Result<ProjectBomItem, String> {
    project_docs::save_project_bom_item_impl(input, state.inner()).await
}

#[tauri::command]
pub async fn delete_project_bom_item(
    item_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_docs::delete_project_bom_item_impl(&item_id, state.inner()).await
}

#[tauri::command]
pub async fn save_assembly_step(
    input: AssemblyStepInput,
    state: State<'_, AppState>,
) -> Result<AssemblyStep, String> {
    project_docs::save_assembly_step_impl(input, state.inner()).await
}

#[tauri::command]
pub async fn delete_assembly_step(
    step_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_docs::delete_assembly_step_impl(&step_id, state.inner()).await
}
