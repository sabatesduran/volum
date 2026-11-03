use std::collections::HashMap;
use tauri::{ipc::Response, AppHandle, State};
use volum_server::{commands as core, commands::*, domain::*, state::AppState};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn list_roots(state: State<'_, AppState>) -> Result<Vec<LibraryRoot>> {
    core::list_roots(state.inner()).await
}

#[tauri::command]
pub async fn add_library_root(path: String, state: State<'_, AppState>) -> Result<LibraryRoot> {
    core::add_library_root(path, state.inner()).await
}

#[tauri::command]
pub async fn remove_library_root(
    root_id: String,
    keep_metadata: bool,
    state: State<'_, AppState>,
) -> Result<()> {
    core::remove_library_root(root_id, keep_metadata, state.inner()).await
}

#[tauri::command]
pub async fn reconnect_library_root(
    root_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<LibraryRoot> {
    core::reconnect_library_root(root_id, path, state.inner()).await
}

#[tauri::command]
pub async fn start_scan(root_id: String, state: State<'_, AppState>, app: AppHandle) -> Result<()> {
    core::start_scan(root_id, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn pause_scan(root_id: String, state: State<'_, AppState>) -> Result<()> {
    core::pause_scan(root_id, state.inner()).await
}

#[tauri::command]
pub async fn get_scan_status(root_id: String, state: State<'_, AppState>) -> Result<ScanStatus> {
    core::get_scan_status(root_id, state.inner()).await
}

#[tauri::command]
pub async fn list_folders(
    root_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Folder>> {
    core::list_folders(root_id, state.inner()).await
}

#[tauri::command]
pub async fn list_models(
    query: ModelQuery,
    state: State<'_, AppState>,
) -> Result<Page<ModelSummary>> {
    core::list_models(query, state.inner()).await
}

#[tauri::command]
pub async fn get_model(model_id: String, state: State<'_, AppState>) -> Result<ModelDetail> {
    core::get_model(model_id, state.inner()).await
}

#[tauri::command]
pub async fn toggle_favorite(model_id: String, state: State<'_, AppState>) -> Result<bool> {
    core::toggle_favorite(model_id, state.inner()).await
}

#[tauri::command]
pub async fn set_favorite(
    model_ids: Vec<String>,
    favorite: bool,
    state: State<'_, AppState>,
) -> Result<()> {
    core::set_favorite(model_ids, favorite, state.inner()).await
}

#[tauri::command]
pub async fn save_notes(model_id: String, notes: String, state: State<'_, AppState>) -> Result<()> {
    core::save_notes(model_id, notes, state.inner()).await
}

#[tauri::command]
pub async fn list_saved_searches(state: State<'_, AppState>) -> Result<Vec<SavedSearch>> {
    core::list_saved_searches(state.inner()).await
}

#[tauri::command]
pub async fn save_saved_search(
    input: SavedSearchInput,
    state: State<'_, AppState>,
) -> Result<SavedSearch> {
    core::save_saved_search(input, state.inner()).await
}

#[tauri::command]
pub async fn delete_saved_search(search_id: String, state: State<'_, AppState>) -> Result<()> {
    core::delete_saved_search(search_id, state.inner()).await
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>> {
    core::list_collections(state.inner()).await
}

#[tauri::command]
pub async fn create_collection(
    input: CollectionInput,
    state: State<'_, AppState>,
) -> Result<Collection> {
    core::create_collection(input, state.inner()).await
}

#[tauri::command]
pub async fn update_collection(
    collection_id: String,
    input: CollectionInput,
    state: State<'_, AppState>,
) -> Result<Collection> {
    core::update_collection(collection_id, input, state.inner()).await
}

#[tauri::command]
pub async fn delete_collection(collection_id: String, state: State<'_, AppState>) -> Result<()> {
    core::delete_collection(collection_id, state.inner()).await
}

#[tauri::command]
pub async fn add_models_to_collection(
    collection_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    core::add_models_to_collection(collection_id, model_ids, state.inner()).await
}

#[tauri::command]
pub async fn remove_models_from_collection(
    collection_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    core::remove_models_from_collection(collection_id, model_ids, state.inner()).await
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> Result<Vec<Tag>> {
    core::list_tags(state.inner()).await
}

#[tauri::command]
pub async fn save_tag(input: TagInput, state: State<'_, AppState>) -> Result<Tag> {
    core::save_tag(input, state.inner()).await
}

#[tauri::command]
pub async fn delete_tag(tag_id: String, state: State<'_, AppState>) -> Result<()> {
    core::delete_tag(tag_id, state.inner()).await
}

#[tauri::command]
pub async fn set_model_tags(
    model_id: String,
    tag_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    core::set_model_tags(model_id, tag_ids, state.inner()).await
}

#[tauri::command]
pub async fn add_models_to_tag(
    tag_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    core::add_models_to_tag(tag_id, model_ids, state.inner()).await
}

#[tauri::command]
pub async fn list_related_models(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RelatedModel>> {
    core::list_related_models(model_id, state.inner()).await
}

#[tauri::command]
pub async fn get_duplicate_stats(state: State<'_, AppState>) -> Result<DuplicateStats> {
    core::get_duplicate_stats(state.inner()).await
}

#[tauri::command]
pub async fn list_duplicate_groups(
    offset: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> Result<Page<DuplicateGroup>> {
    core::list_duplicate_groups(offset, limit, state.inner()).await
}

#[tauri::command]
pub async fn merge_projects(input: MergeProjectsInput, state: State<'_, AppState>) -> Result<()> {
    core::merge_projects(input, state.inner()).await
}

#[tauri::command]
pub async fn split_project(input: SplitProjectInput, state: State<'_, AppState>) -> Result<String> {
    core::split_project(input, state.inner()).await
}

#[tauri::command]
pub async fn set_project_primary_asset(
    project_id: String,
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<()> {
    core::set_project_primary_asset(project_id, asset_id, state.inner()).await
}

#[tauri::command]
pub async fn dismiss_duplicate_match(match_key: String, state: State<'_, AppState>) -> Result<()> {
    core::dismiss_duplicate_match(match_key, state.inner()).await
}

#[tauri::command]
pub async fn cleanup_duplicate_group(
    input: DuplicateCleanupInput,
    state: State<'_, AppState>,
) -> Result<()> {
    core::cleanup_duplicate_group(input, state.inner()).await
}

#[tauri::command]
pub async fn list_materials(state: State<'_, AppState>) -> Result<Vec<Material>> {
    core::list_materials(state.inner()).await
}

#[tauri::command]
pub async fn save_material(input: MaterialInput, state: State<'_, AppState>) -> Result<Material> {
    core::save_material(input, state.inner()).await
}

#[tauri::command]
pub fn calculate_cost(input: CostCalculationInput) -> Result<CostCalculation> {
    core::calculate_cost(input)
}

#[tauri::command]
pub async fn save_cost_estimate(
    input: CostEstimateInput,
    state: State<'_, AppState>,
) -> Result<()> {
    core::save_cost_estimate(input, state.inner()).await
}

#[tauri::command]
pub async fn get_preview_payload(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<PreviewPayload> {
    core::get_preview_payload(asset_id, state.inner()).await
}

#[tauri::command]
pub async fn get_3mf_plate_thumbnail(
    asset_id: String,
    plate_index: u32,
    state: State<'_, AppState>,
) -> Result<Response> {
    core::get_3mf_plate_thumbnail(asset_id, plate_index, state.inner())
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn get_viewer_mesh(
    asset_id: String,
    plate_index: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Response> {
    core::get_viewer_mesh(asset_id, plate_index, state.inner())
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn get_cached_thumbnail(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Response> {
    core::get_cached_thumbnail(asset_id, state.inner())
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn clear_preview_cache(state: State<'_, AppState>) -> Result<()> {
    core::clear_preview_cache(state.inner()).await
}

#[tauri::command]
pub async fn request_thumbnail(
    asset_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<bool> {
    core::request_thumbnail(asset_id, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn cancel_thumbnail_request(asset_id: String, state: State<'_, AppState>) -> Result<()> {
    core::cancel_thumbnail_request(asset_id, state.inner()).await
}

#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<HashMap<String, String>> {
    core::get_preferences(state.inner()).await
}

#[tauri::command]
pub async fn list_slicer_apps(
    custom_apps: Option<Vec<ConfiguredSlicer>>,
    state: State<'_, AppState>,
) -> Result<Vec<SlicerApplication>> {
    core::list_slicer_apps(custom_apps, state.inner()).await
}

#[tauri::command]
pub async fn save_preference(key: String, value: String, state: State<'_, AppState>) -> Result<()> {
    core::save_preference(key, value, state.inner()).await
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<()> {
    core::open_external_url(url)
}

#[tauri::command]
pub async fn open_asset(
    asset_id: String,
    app_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    core::open_asset(asset_id, app_path, state.inner()).await
}

#[tauri::command]
pub async fn reveal_asset(asset_id: String, state: State<'_, AppState>) -> Result<()> {
    core::reveal_asset(asset_id, state.inner()).await
}

#[tauri::command]
pub async fn export_metadata(path: String, state: State<'_, AppState>) -> Result<()> {
    core::export_metadata(path, state.inner()).await
}

#[tauri::command]
pub async fn export_diagnostics(
    path: String,
    redact_paths: bool,
    state: State<'_, AppState>,
) -> Result<()> {
    core::export_diagnostics(path, redact_paths, state.inner()).await
}
