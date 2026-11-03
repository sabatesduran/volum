use tauri::{AppHandle, State};
use volum_server::{
    domain::{WebSource, WebSourcePreview},
    state::AppState,
    web_sources as core,
};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn preview_web_source(url: String) -> Result<WebSourcePreview> {
    core::preview_web_source(url).await
}

#[tauri::command]
pub async fn list_web_sources(state: State<'_, AppState>) -> Result<Vec<WebSource>> {
    core::list_web_sources(state.inner()).await
}

#[tauri::command]
pub async fn save_web_source(
    preview: WebSourcePreview,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<WebSource> {
    core::save_web_source(preview, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn delete_web_source(
    source_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<()> {
    core::delete_web_source(source_id, state.inner(), crate::event_emitter(app)).await
}

#[tauri::command]
pub async fn attach_web_source_file(
    source_id: String,
    local_path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<WebSource> {
    core::attach_web_source_file(
        source_id,
        local_path,
        state.inner(),
        crate::event_emitter(app),
    )
    .await
}
