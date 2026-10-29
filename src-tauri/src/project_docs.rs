use crate::{
    domain::{
        AssemblyStep, CustomFieldDefinition, ProjectBomItem, ProjectCustomValue,
        ProjectDocumentation, ProjectPhoto,
    },
    indexing, parsers,
    state::AppState,
};
use chrono::{NaiveDate, Utc};
use image::ImageFormat;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufWriter, Cursor},
    path::{Component, Path, PathBuf},
};
use tauri::State;
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;

const MAX_PHOTO_BYTES: u64 = 25 * 1024 * 1024;
const MAX_PHOTO_PIXELS: u64 = 100_000_000;
const MAX_PHOTO_EDGE: u32 = 20_000;
const THUMBNAIL_EDGE: u32 = 1_200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPhotoUpdate {
    pub id: String,
    pub caption: String,
    pub sort_order: f64,
    pub is_cover: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldInput {
    pub id: Option<String>,
    pub name: String,
    pub field_type: String,
    pub unit: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCustomValueInput {
    pub model_id: String,
    pub field_id: String,
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBomItemInput {
    pub id: Option<String>,
    pub model_id: String,
    pub kind: String,
    pub linked_asset_id: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit: String,
    pub url: Option<String>,
    pub notes: String,
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssemblyStepInput {
    pub id: Option<String>,
    pub model_id: String,
    pub title: String,
    pub instructions: String,
    pub sort_order: Option<f64>,
    #[serde(default)]
    pub bom_item_ids: Vec<String>,
    #[serde(default)]
    pub photo_ids: Vec<String>,
}

#[derive(Debug)]
struct ProcessedPhoto {
    extension: &'static str,
    mime_type: &'static str,
    byte_size: u64,
    width: u32,
    height: u32,
}

#[tauri::command]
pub async fn get_project_documentation(
    model_id: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectDocumentation> {
    documentation_for_project(&model_id, &state.pool).await
}

pub async fn documentation_for_project(
    model_id: &str,
    pool: &SqlitePool,
) -> CommandResult<ProjectDocumentation> {
    ensure_project(model_id, pool).await?;
    let photo_rows = sqlx::query("SELECT id, model_id, original_filename, mime_type, byte_size, width, height, caption, sort_order, is_cover, created_at, updated_at FROM project_photos WHERE model_id = ? ORDER BY sort_order, created_at")
        .bind(model_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let value_rows = sqlx::query(
        "SELECT field_id, value_json, updated_at FROM project_custom_values WHERE model_id = ?",
    )
    .bind(model_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let bom_rows = sqlx::query("SELECT id, model_id, kind, linked_asset_id, name, quantity, unit, url, notes, sort_order, created_at, updated_at FROM project_bom_items WHERE model_id = ? ORDER BY sort_order, created_at")
        .bind(model_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let step_rows = sqlx::query("SELECT id, model_id, title, instructions, sort_order, created_at, updated_at FROM assembly_steps WHERE model_id = ? ORDER BY sort_order, created_at")
        .bind(model_id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut assembly_steps = Vec::with_capacity(step_rows.len());
    for row in step_rows {
        let id: String = row.get("id");
        let bom_item_ids = sqlx::query_scalar(
            "SELECT bom_item_id FROM assembly_step_bom_items WHERE step_id = ? ORDER BY sort_order",
        )
        .bind(&id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
        let photo_ids = sqlx::query_scalar(
            "SELECT photo_id FROM assembly_step_photos WHERE step_id = ? ORDER BY sort_order",
        )
        .bind(&id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
        assembly_steps.push(AssemblyStep {
            id,
            model_id: row.get("model_id"),
            title: row.get("title"),
            instructions: row.get("instructions"),
            sort_order: row.get("sort_order"),
            bom_item_ids,
            photo_ids,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        });
    }
    Ok(ProjectDocumentation {
        photos: photo_rows.into_iter().map(photo_from_row).collect(),
        custom_values: value_rows
            .into_iter()
            .map(|row| ProjectCustomValue {
                field_id: row.get("field_id"),
                value: serde_json::from_str(&row.get::<String, _>("value_json"))
                    .unwrap_or(Value::Null),
                updated_at: row.get("updated_at"),
            })
            .collect(),
        bom_items: bom_rows.into_iter().map(bom_from_row).collect(),
        assembly_steps,
    })
}

#[tauri::command]
pub async fn add_project_photo(
    model_id: String,
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectPhoto> {
    let _guard = state.file_operation_lock.lock().await;
    store_project_photo(&model_id, Path::new(&path), None, "", None, None, &state).await
}

pub(crate) async fn store_project_photo(
    model_id: &str,
    source: &Path,
    original_filename_override: Option<&str>,
    caption: &str,
    sort_order_override: Option<f64>,
    is_cover_override: Option<bool>,
    state: &AppState,
) -> CommandResult<ProjectPhoto> {
    ensure_project(model_id, &state.pool).await?;
    let metadata =
        std::fs::metadata(source).map_err(|_| "Choose an available image file".to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PHOTO_BYTES {
        return Err("Project photos must be non-empty files no larger than 25 MB".into());
    }
    let photo_id = Uuid::new_v4().to_string();
    let project = sqlx::query("SELECT m.storage_mode, f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ?")
        .bind(model_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    let managed = project.get::<String, _>("storage_mode") == "managed";
    let media_dir = if managed {
        PathBuf::from(project.get::<String, _>("root_path"))
            .join(project.get::<String, _>("relative_path"))
            .join("Photos")
    } else {
        state
            .data_dir
            .join("media")
            .join("project-photos")
            .join(model_id)
    };
    let thumbnail_dir = state
        .data_dir
        .join("media")
        .join("project-photos")
        .join(model_id);
    let source_for_processing = source.to_path_buf();
    let media_dir_for_processing = media_dir.clone();
    let thumbnail_dir_for_processing = thumbnail_dir.clone();
    let photo_id_for_processing = photo_id.clone();
    let processed = tokio::task::spawn_blocking(move || {
        process_photo(
            &source_for_processing,
            &media_dir_for_processing,
            &thumbnail_dir_for_processing,
            &photo_id_for_processing,
        )
    })
    .await
    .map_err(|error| error.to_string())??;
    let relative_base = PathBuf::from("project-photos").join(model_id);
    let media_path = if managed {
        format!("managed:Photos/{}.{}", photo_id, processed.extension)
    } else {
        relative_base
            .join(format!("{}.{}", photo_id, processed.extension))
            .to_string_lossy()
            .replace('\\', "/")
    };
    let thumbnail_path = relative_base
        .join(format!("{photo_id}.thumb.png"))
        .to_string_lossy()
        .replace('\\', "/");
    let original_filename = original_filename_override
        .or_else(|| source.file_name().and_then(|value| value.to_str()))
        .unwrap_or("photo")
        .chars()
        .take(255)
        .collect::<String>();
    let timestamp = now();
    let order: f64 = if let Some(order) = sort_order_override.filter(|value| value.is_finite()) {
        order
    } else {
        sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM project_photos WHERE model_id = ?",
        )
        .bind(model_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?
    };
    let has_cover: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_photos WHERE model_id = ? AND is_cover = 1",
    )
    .bind(model_id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    let is_cover = is_cover_override.unwrap_or(has_cover == 0);
    if is_cover {
        sqlx::query("UPDATE project_photos SET is_cover = 0, updated_at = ? WHERE model_id = ?")
            .bind(&timestamp)
            .bind(model_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    let result = sqlx::query("INSERT INTO project_photos (id, model_id, media_path, thumbnail_path, original_filename, mime_type, byte_size, width, height, caption, sort_order, is_cover, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&photo_id)
        .bind(model_id)
        .bind(&media_path)
        .bind(&thumbnail_path)
        .bind(&original_filename)
        .bind(processed.mime_type)
        .bind(processed.byte_size as i64)
        .bind(i64::from(processed.width))
        .bind(i64::from(processed.height))
        .bind(caption.trim().chars().take(500).collect::<String>())
        .bind(order)
        .bind(i64::from(is_cover))
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await;
    if let Err(error) = result {
        let _ =
            std::fs::remove_file(media_dir.join(format!("{}.{}", photo_id, processed.extension)));
        let _ = std::fs::remove_file(state.data_dir.join("media").join(&thumbnail_path));
        return Err(db_error(error));
    }
    touch_project(model_id, &state.pool).await?;
    indexing::refresh_search(model_id, &state.pool).await?;
    photo_by_id(&photo_id, &state.pool).await
}

#[tauri::command]
pub async fn update_project_photo(
    input: ProjectPhotoUpdate,
    state: State<'_, AppState>,
) -> CommandResult<ProjectPhoto> {
    update_project_photo_impl(input, state.inner()).await
}

pub(crate) async fn update_project_photo_impl(
    input: ProjectPhotoUpdate,
    state: &AppState,
) -> CommandResult<ProjectPhoto> {
    let caption = input.caption.trim();
    if caption.chars().count() > 500 || !input.sort_order.is_finite() {
        return Err("Photo captions may contain up to 500 characters".into());
    }
    let model_id: String = sqlx::query_scalar("SELECT model_id FROM project_photos WHERE id = ?")
        .bind(&input.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project photo not found".to_string())?;
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    if input.is_cover {
        sqlx::query("UPDATE project_photos SET is_cover = 0, updated_at = ? WHERE model_id = ?")
            .bind(now())
            .bind(&model_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE project_photos SET caption = ?, sort_order = ?, is_cover = ?, updated_at = ? WHERE id = ?")
        .bind(caption)
        .bind(input.sort_order)
        .bind(i64::from(input.is_cover))
        .bind(now())
        .bind(&input.id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    touch_project(&model_id, &state.pool).await?;
    indexing::refresh_search(&model_id, &state.pool).await?;
    photo_by_id(&input.id, &state.pool).await
}

#[tauri::command]
pub async fn delete_project_photo(
    photo_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let _guard = state.file_operation_lock.lock().await;
    delete_project_photo_impl(&photo_id, state.inner()).await
}

pub(crate) async fn delete_project_photo_impl(
    photo_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    let row = sqlx::query(
        "SELECT model_id, media_path, thumbnail_path, is_cover FROM project_photos WHERE id = ?",
    )
    .bind(photo_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| "Project photo not found".to_string())?;
    let model_id: String = row.get("model_id");
    let media_path =
        resolve_project_photo_path(&model_id, &row.get::<String, _>("media_path"), state).await?;
    let thumbnail_path = safe_media_path(&state.data_dir, &row.get::<String, _>("thumbnail_path"))?;
    let was_cover = row.get::<i64, _>("is_cover") != 0;
    sqlx::query("DELETE FROM project_photos WHERE id = ?")
        .bind(photo_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = std::fs::remove_file(media_path);
    let _ = std::fs::remove_file(thumbnail_path);
    if was_cover {
        sqlx::query("UPDATE project_photos SET is_cover = 1, updated_at = ? WHERE id = (SELECT id FROM project_photos WHERE model_id = ? ORDER BY sort_order, created_at LIMIT 1)")
            .bind(now())
            .bind(&model_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    touch_project(&model_id, &state.pool).await?;
    indexing::refresh_search(&model_id, &state.pool).await
}

#[tauri::command]
pub async fn get_project_photo(
    photo_id: String,
    thumbnail: bool,
    state: State<'_, AppState>,
) -> CommandResult<tauri::ipc::Response> {
    let (bytes, _) = project_photo_bytes(&photo_id, thumbnail, state.inner()).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub(crate) async fn project_photo_bytes(
    photo_id: &str,
    thumbnail: bool,
    state: &AppState,
) -> CommandResult<(Vec<u8>, String)> {
    let row = sqlx::query(
        "SELECT model_id, media_path, thumbnail_path, mime_type FROM project_photos WHERE id = ?",
    )
    .bind(photo_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| "Project photo not found".to_string())?;
    let relative = if thumbnail {
        row.get::<String, _>("thumbnail_path")
    } else {
        row.get::<String, _>("media_path")
    };
    let path = if thumbnail {
        safe_media_path(&state.data_dir, &relative)?
    } else {
        resolve_project_photo_path(&row.get::<String, _>("model_id"), &relative, state).await?
    };
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| "Project photo is unavailable".to_string())?;
    let mime_type = if thumbnail {
        "image/png".to_string()
    } else {
        row.get("mime_type")
    };
    Ok((bytes, mime_type))
}

#[tauri::command]
pub async fn list_custom_field_definitions(
    state: State<'_, AppState>,
) -> CommandResult<Vec<CustomFieldDefinition>> {
    custom_field_definitions(&state.pool).await
}

pub async fn custom_field_definitions(
    pool: &SqlitePool,
) -> CommandResult<Vec<CustomFieldDefinition>> {
    let rows = sqlx::query("SELECT id, name, field_type, unit, options_json, sort_order, created_at, updated_at FROM custom_field_definitions ORDER BY sort_order, lower(name)")
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    Ok(rows.into_iter().map(field_from_row).collect())
}

#[tauri::command]
pub async fn save_custom_field_definition(
    input: CustomFieldInput,
    state: State<'_, AppState>,
) -> CommandResult<CustomFieldDefinition> {
    save_custom_field_definition_impl(input, &state).await
}

pub(crate) async fn save_custom_field_definition_impl(
    input: CustomFieldInput,
    state: &AppState,
) -> CommandResult<CustomFieldDefinition> {
    validate_field_input(&input)?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = input.name.trim();
    let unit = input
        .unit
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let options = normalize_options(&input.options)?;
    let options_json = serde_json::to_string(&options).map_err(|error| error.to_string())?;
    let timestamp = now();
    let existing = sqlx::query(
        "SELECT created_at, sort_order, field_type FROM custom_field_definitions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?;
    if existing
        .as_ref()
        .is_some_and(|row| row.get::<String, _>("field_type") != input.field_type)
    {
        let value_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM project_custom_values WHERE field_id = ?")
                .bind(&id)
                .fetch_one(&state.pool)
                .await
                .map_err(db_error)?;
        if value_count != 0 {
            return Err("A custom field with saved values cannot change type".into());
        }
    }
    if input.field_type == "select" {
        let stored_values: Vec<String> =
            sqlx::query_scalar("SELECT value_json FROM project_custom_values WHERE field_id = ?")
                .bind(&id)
                .fetch_all(&state.pool)
                .await
                .map_err(db_error)?;
        if stored_values.iter().any(|stored| {
            serde_json::from_str::<String>(stored)
                .ok()
                .is_some_and(|value| !options.contains(&value))
        }) {
            return Err("Select options cannot remove values that projects still use".into());
        }
    }
    let created_at = existing
        .as_ref()
        .map(|row| row.get::<String, _>("created_at"))
        .unwrap_or_else(|| timestamp.clone());
    let order = if let Some(row) = existing {
        row.get::<f64, _>("sort_order")
    } else {
        sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM custom_field_definitions",
        )
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?
    };
    sqlx::query("INSERT INTO custom_field_definitions (id, name, field_type, unit, options_json, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, field_type = excluded.field_type, unit = excluded.unit, options_json = excluded.options_json, updated_at = excluded.updated_at")
        .bind(&id)
        .bind(name)
        .bind(&input.field_type)
        .bind(unit)
        .bind(options_json)
        .bind(order)
        .bind(created_at)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(|error| if error.to_string().contains("UNIQUE") { "A custom field with that name already exists".into() } else { db_error(error) })?;
    let affected_models: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM project_custom_values WHERE field_id = ?")
            .bind(&id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    for model_id in affected_models {
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    let row = sqlx::query("SELECT id, name, field_type, unit, options_json, sort_order, created_at, updated_at FROM custom_field_definitions WHERE id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(field_from_row(row))
}

#[tauri::command]
pub async fn delete_custom_field_definition(
    field_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    delete_custom_field_definition_impl(&field_id, &state).await
}

pub(crate) async fn delete_custom_field_definition_impl(
    field_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM project_custom_values WHERE field_id = ?")
            .bind(field_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    sqlx::query("DELETE FROM custom_field_definitions WHERE id = ?")
        .bind(field_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    for model_id in model_ids {
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_project_custom_value(
    input: ProjectCustomValueInput,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    set_project_custom_value_impl(input, state.inner()).await
}

pub(crate) async fn set_project_custom_value_impl(
    input: ProjectCustomValueInput,
    state: &AppState,
) -> CommandResult<()> {
    ensure_project(&input.model_id, &state.pool).await?;
    let row =
        sqlx::query("SELECT field_type, options_json FROM custom_field_definitions WHERE id = ?")
            .bind(&input.field_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "Custom field not found".to_string())?;
    if let Some(value) = input.value {
        validate_custom_value(
            &row.get::<String, _>("field_type"),
            &row.get::<String, _>("options_json"),
            &value,
        )?;
        sqlx::query("INSERT INTO project_custom_values (model_id, field_id, value_json, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(model_id, field_id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
            .bind(&input.model_id)
            .bind(&input.field_id)
            .bind(serde_json::to_string(&value).map_err(|error| error.to_string())?)
            .bind(now())
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    } else {
        sqlx::query("DELETE FROM project_custom_values WHERE model_id = ? AND field_id = ?")
            .bind(&input.model_id)
            .bind(&input.field_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    touch_project(&input.model_id, &state.pool).await?;
    indexing::refresh_search(&input.model_id, &state.pool).await
}

#[tauri::command]
pub async fn save_project_bom_item(
    input: ProjectBomItemInput,
    state: State<'_, AppState>,
) -> CommandResult<ProjectBomItem> {
    save_project_bom_item_impl(input, state.inner()).await
}

pub(crate) async fn save_project_bom_item_impl(
    input: ProjectBomItemInput,
    state: &AppState,
) -> CommandResult<ProjectBomItem> {
    validate_bom_input(&input, &state.pool).await?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let existing =
        sqlx::query("SELECT model_id, created_at, sort_order FROM project_bom_items WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    if existing
        .as_ref()
        .is_some_and(|row| row.get::<String, _>("model_id") != input.model_id)
    {
        return Err("BOM item belongs to another project".into());
    }
    let created_at = existing
        .as_ref()
        .map(|row| row.get::<String, _>("created_at"))
        .unwrap_or_else(|| timestamp.clone());
    let sort_order = if let Some(value) = input.sort_order {
        value
    } else if let Some(row) = existing {
        row.get("sort_order")
    } else {
        sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM project_bom_items WHERE model_id = ?",
        )
        .bind(&input.model_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?
    };
    if !sort_order.is_finite() {
        return Err("BOM order is invalid".into());
    }
    sqlx::query("INSERT INTO project_bom_items (id, model_id, kind, linked_asset_id, name, quantity, unit, url, notes, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, linked_asset_id = excluded.linked_asset_id, name = excluded.name, quantity = excluded.quantity, unit = excluded.unit, url = excluded.url, notes = excluded.notes, sort_order = excluded.sort_order, updated_at = excluded.updated_at")
        .bind(&id)
        .bind(&input.model_id)
        .bind(&input.kind)
        .bind(&input.linked_asset_id)
        .bind(input.name.trim())
        .bind(input.quantity)
        .bind(input.unit.trim())
        .bind(normalize_url(input.url.as_deref())?)
        .bind(input.notes.trim())
        .bind(sort_order)
        .bind(created_at)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    touch_project(&input.model_id, &state.pool).await?;
    indexing::refresh_search(&input.model_id, &state.pool).await?;
    bom_by_id(&id, &state.pool).await
}

#[tauri::command]
pub async fn delete_project_bom_item(
    item_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    delete_project_bom_item_impl(&item_id, state.inner()).await
}

pub(crate) async fn delete_project_bom_item_impl(
    item_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    let model_id: String =
        sqlx::query_scalar("SELECT model_id FROM project_bom_items WHERE id = ?")
            .bind(item_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "BOM item not found".to_string())?;
    sqlx::query("DELETE FROM project_bom_items WHERE id = ?")
        .bind(item_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    touch_project(&model_id, &state.pool).await?;
    indexing::refresh_search(&model_id, &state.pool).await
}

#[tauri::command]
pub async fn save_assembly_step(
    input: AssemblyStepInput,
    state: State<'_, AppState>,
) -> CommandResult<AssemblyStep> {
    save_assembly_step_impl(input, state.inner()).await
}

pub(crate) async fn save_assembly_step_impl(
    input: AssemblyStepInput,
    state: &AppState,
) -> CommandResult<AssemblyStep> {
    validate_step_input(&input, &state.pool).await?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let existing =
        sqlx::query("SELECT model_id, created_at, sort_order FROM assembly_steps WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    if existing
        .as_ref()
        .is_some_and(|row| row.get::<String, _>("model_id") != input.model_id)
    {
        return Err("Assembly step belongs to another project".into());
    }
    let created_at = existing
        .as_ref()
        .map(|row| row.get::<String, _>("created_at"))
        .unwrap_or_else(|| timestamp.clone());
    let sort_order = if let Some(value) = input.sort_order {
        value
    } else if let Some(row) = existing {
        row.get("sort_order")
    } else {
        sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM assembly_steps WHERE model_id = ?",
        )
        .bind(&input.model_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?
    };
    if !sort_order.is_finite() {
        return Err("Assembly step order is invalid".into());
    }
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    sqlx::query("INSERT INTO assembly_steps (id, model_id, title, instructions, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET title = excluded.title, instructions = excluded.instructions, sort_order = excluded.sort_order, updated_at = excluded.updated_at")
        .bind(&id)
        .bind(&input.model_id)
        .bind(input.title.trim())
        .bind(input.instructions.trim())
        .bind(sort_order)
        .bind(created_at)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM assembly_step_bom_items WHERE step_id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for (index, item_id) in input.bom_item_ids.iter().enumerate() {
        sqlx::query("INSERT INTO assembly_step_bom_items (step_id, bom_item_id, sort_order) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(item_id)
            .bind(index as f64)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("DELETE FROM assembly_step_photos WHERE step_id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for (index, photo_id) in input.photo_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO assembly_step_photos (step_id, photo_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(photo_id)
        .bind(index as f64)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    }
    transaction.commit().await.map_err(db_error)?;
    touch_project(&input.model_id, &state.pool).await?;
    indexing::refresh_search(&input.model_id, &state.pool).await?;
    step_by_id(&id, &state.pool).await
}

#[tauri::command]
pub async fn delete_assembly_step(
    step_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    delete_assembly_step_impl(&step_id, state.inner()).await
}

pub(crate) async fn delete_assembly_step_impl(
    step_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    let model_id: String = sqlx::query_scalar("SELECT model_id FROM assembly_steps WHERE id = ?")
        .bind(step_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Assembly step not found".to_string())?;
    sqlx::query("DELETE FROM assembly_steps WHERE id = ?")
        .bind(step_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    touch_project(&model_id, &state.pool).await?;
    indexing::refresh_search(&model_id, &state.pool).await
}

pub async fn searchable_text(model_id: &str, pool: &SqlitePool) -> CommandResult<String> {
    let custom: String = sqlx::query_scalar("SELECT COALESCE(group_concat(d.name || ' ' || v.value_json, ' '), '') FROM project_custom_values v JOIN custom_field_definitions d ON d.id = v.field_id WHERE v.model_id = ?")
        .bind(model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let bom: String = sqlx::query_scalar("SELECT COALESCE(group_concat(name || ' ' || COALESCE(url, '') || ' ' || notes, ' '), '') FROM project_bom_items WHERE model_id = ?")
        .bind(model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let steps: String = sqlx::query_scalar("SELECT COALESCE(group_concat(title || ' ' || instructions, ' '), '') FROM assembly_steps WHERE model_id = ?")
        .bind(model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let photos: String = sqlx::query_scalar(
        "SELECT COALESCE(group_concat(caption, ' '), '') FROM project_photos WHERE model_id = ?",
    )
    .bind(model_id)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    Ok(format!("{custom} {bom} {steps} {photos}"))
}

pub async fn transfer_documentation(
    source_id: &str,
    keeper_id: &str,
    transaction: &mut Transaction<'_, Sqlite>,
) -> CommandResult<()> {
    let keeper_has_cover: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_photos WHERE model_id = ? AND is_cover = 1",
    )
    .bind(keeper_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(db_error)?;
    if keeper_has_cover != 0 {
        sqlx::query("UPDATE project_photos SET is_cover = 0 WHERE model_id = ?")
            .bind(source_id)
            .execute(&mut **transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE project_photos SET model_id = ?, updated_at = ? WHERE model_id = ?")
        .bind(keeper_id)
        .bind(now())
        .bind(source_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;
    transfer_custom_values(source_id, keeper_id, transaction).await?;
    sqlx::query("UPDATE project_bom_items SET model_id = ?, updated_at = ? WHERE model_id = ?")
        .bind(keeper_id)
        .bind(now())
        .bind(source_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE assembly_steps SET model_id = ?, updated_at = ? WHERE model_id = ?")
        .bind(keeper_id)
        .bind(now())
        .bind(source_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn transfer_custom_values(
    source_id: &str,
    keeper_id: &str,
    transaction: &mut Transaction<'_, Sqlite>,
) -> CommandResult<()> {
    let rows = sqlx::query("SELECT v.field_id, v.value_json, v.updated_at, d.name, d.field_type, d.unit, d.options_json FROM project_custom_values v JOIN custom_field_definitions d ON d.id = v.field_id WHERE v.model_id = ?")
        .bind(source_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(db_error)?;
    let source_name: String = sqlx::query_scalar("SELECT display_name FROM models WHERE id = ?")
        .bind(source_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(db_error)?;
    for row in rows {
        let field_id: String = row.get("field_id");
        let source_value: String = row.get("value_json");
        let keeper_value: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM project_custom_values WHERE model_id = ? AND field_id = ?",
        )
        .bind(keeper_id)
        .bind(&field_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(db_error)?;
        if keeper_value.as_deref() == Some(source_value.as_str()) {
            continue;
        }
        let target_field = if keeper_value.is_none() {
            field_id
        } else {
            let base = format!("{} ({})", row.get::<String, _>("name"), source_name);
            let mut candidate = base.chars().take(80).collect::<String>();
            let mut suffix = 2;
            while sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM custom_field_definitions WHERE name = ? COLLATE NOCASE",
            )
            .bind(&candidate)
            .fetch_one(&mut **transaction)
            .await
            .map_err(db_error)?
                != 0
            {
                candidate = format!("{} {suffix}", base.chars().take(74).collect::<String>());
                suffix += 1;
            }
            let new_id = Uuid::new_v4().to_string();
            let order: f64 = sqlx::query_scalar(
                "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM custom_field_definitions",
            )
            .fetch_one(&mut **transaction)
            .await
            .map_err(db_error)?;
            let timestamp = now();
            sqlx::query("INSERT INTO custom_field_definitions (id, name, field_type, unit, options_json, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&new_id)
                .bind(candidate)
                .bind(row.get::<String, _>("field_type"))
                .bind(row.get::<Option<String>, _>("unit"))
                .bind(row.get::<String, _>("options_json"))
                .bind(order)
                .bind(&timestamp)
                .bind(&timestamp)
                .execute(&mut **transaction)
                .await
                .map_err(db_error)?;
            new_id
        };
        sqlx::query("INSERT OR REPLACE INTO project_custom_values (model_id, field_id, value_json, updated_at) VALUES (?, ?, ?, ?)")
            .bind(keeper_id)
            .bind(target_field)
            .bind(source_value)
            .bind(row.get::<String, _>("updated_at"))
            .execute(&mut **transaction)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

fn process_photo(
    source: &Path,
    media_dir: &Path,
    thumbnail_dir: &Path,
    photo_id: &str,
) -> CommandResult<ProcessedPhoto> {
    let bytes = std::fs::read(source).map_err(|error| format!("Could not read photo: {error}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_PHOTO_BYTES {
        return Err("Project photos must be non-empty files no larger than 25 MB".into());
    }
    let format =
        image::guess_format(&bytes).map_err(|_| "Unsupported project photo format".to_string())?;
    let (extension, mime_type) = match format {
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::WebP => ("webp", "image/webp"),
        _ => return Err("Project photos must be JPEG, PNG, or WebP images".into()),
    };
    let reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| "The selected project photo is damaged or unsupported".to_string())?;
    if width == 0
        || height == 0
        || width > MAX_PHOTO_EDGE
        || height > MAX_PHOTO_EDGE
        || u64::from(width) * u64::from(height) > MAX_PHOTO_PIXELS
    {
        return Err("Project photo dimensions are too large".into());
    }
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| "The selected project photo is damaged or unsupported".to_string())?;
    std::fs::create_dir_all(media_dir)
        .map_err(|error| format!("Could not create project photo storage: {error}"))?;
    std::fs::create_dir_all(thumbnail_dir)
        .map_err(|error| format!("Could not create project photo preview storage: {error}"))?;
    let destination = media_dir.join(format!("{photo_id}.{extension}"));
    let temporary = media_dir.join(format!("{photo_id}.{extension}.partial"));
    std::fs::write(&temporary, &bytes)
        .map_err(|error| format!("Could not copy project photo: {error}"))?;
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("Could not finish copying project photo: {error}"))?;
    let thumbnail_path = thumbnail_dir.join(format!("{photo_id}.thumb.png"));
    let thumbnail_temporary = thumbnail_dir.join(format!("{photo_id}.thumb.png.partial"));
    let thumbnail = image.thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE);
    let result = File::create(&thumbnail_temporary)
        .map(BufWriter::new)
        .map_err(|error| error.to_string())
        .and_then(|mut writer| {
            thumbnail
                .write_to(&mut writer, ImageFormat::Png)
                .map_err(|error| error.to_string())
        })
        .and_then(|_| {
            std::fs::rename(&thumbnail_temporary, &thumbnail_path)
                .map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        let _ = std::fs::remove_file(&destination);
        let _ = std::fs::remove_file(&thumbnail_temporary);
        return Err(format!("Could not create project photo preview: {error}"));
    }
    Ok(ProcessedPhoto {
        extension,
        mime_type,
        byte_size: bytes.len() as u64,
        width,
        height,
    })
}

fn safe_media_path(data_dir: &Path, relative: &str) -> CommandResult<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Project photo has an unsafe storage path".into());
    }
    Ok(data_dir.join("media").join(relative_path))
}

pub(crate) async fn resolve_project_photo_path(
    model_id: &str,
    storage_path: &str,
    state: &AppState,
) -> CommandResult<PathBuf> {
    let Some(relative) = storage_path.strip_prefix("managed:") else {
        return safe_media_path(&state.data_dir, storage_path);
    };
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Project photo has an unsafe managed storage path".into());
    }
    let row = sqlx::query("SELECT f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ? AND m.storage_mode = 'managed'")
        .bind(model_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Managed project photo has no managed project".to_string())?;
    let root = PathBuf::from(row.get::<String, _>("root_path"));
    let project = root.join(row.get::<String, _>("relative_path"));
    let destination = project.join(relative);
    let canonical_root = root
        .canonicalize()
        .map_err(|_| "The managed library is unavailable".to_string())?;
    let canonical_parent = destination
        .parent()
        .ok_or_else(|| "Managed project photo path is invalid".to_string())?
        .canonicalize()
        .map_err(|_| "The managed project photo folder is unavailable".to_string())?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err("Managed project photo is outside the managed library".into());
    }
    Ok(destination)
}

pub(crate) async fn move_project_photo_originals_to_managed(
    model_id: &str,
    state: &AppState,
) -> CommandResult<ManagedPhotoMigration> {
    let project = sqlx::query("SELECT f.relative_path, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id WHERE m.id = ? AND m.storage_mode = 'managed'")
        .bind(model_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?;
    let Some(project) = project else {
        return Ok(ManagedPhotoMigration { files: Vec::new() });
    };
    let project_directory = PathBuf::from(project.get::<String, _>("root_path"))
        .join(project.get::<String, _>("relative_path"));
    let photo_directory = project_directory.join("Photos");
    std::fs::create_dir_all(&photo_directory)
        .map_err(|error| format!("Could not create managed photo storage: {error}"))?;
    let rows = sqlx::query("SELECT id, media_path FROM project_photos WHERE model_id = ? AND media_path NOT LIKE 'managed:%'")
        .bind(model_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let mut files = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let old_relative: String = row.get("media_path");
        let source = match safe_media_path(&state.data_dir, &old_relative) {
            Ok(source) => source,
            Err(error) => {
                remove_managed_photo_copies(&files);
                return Err(error);
            }
        };
        if !source.is_file() {
            remove_managed_photo_copies(&files);
            return Err(
                "A project photo is unavailable, so the project cannot be fully consolidated"
                    .into(),
            );
        }
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("jpg")
            .to_ascii_lowercase();
        let filename = format!("{id}.{extension}");
        let destination = photo_directory.join(&filename);
        if destination.exists() {
            remove_managed_photo_copies(&files);
            return Err("A managed project photo destination already exists".into());
        }
        let temporary = photo_directory.join(format!(".{filename}.{}.partial", Uuid::new_v4()));
        if let Err(error) = std::fs::copy(&source, &temporary) {
            remove_managed_photo_copies(&files);
            return Err(format!("Could not consolidate a project photo: {error}"));
        }
        let source_hash = parsers::full_hash(&source);
        let copied_hash = parsers::full_hash(&temporary);
        if source_hash.is_err() || copied_hash.is_err() || source_hash != copied_hash {
            let _ = std::fs::remove_file(&temporary);
            remove_managed_photo_copies(&files);
            return Err("A copied project photo did not pass hash verification".into());
        }
        if let Err(error) = std::fs::rename(&temporary, &destination) {
            let _ = std::fs::remove_file(&temporary);
            remove_managed_photo_copies(&files);
            return Err(format!(
                "Could not finish consolidating a project photo: {error}"
            ));
        }
        files.push(ManagedPhotoMigrationFile {
            id,
            old_relative,
            source,
            destination,
            managed_relative: format!("managed:Photos/{filename}"),
        });
    }
    let mut transaction = match state.pool.begin().await.map_err(db_error) {
        Ok(transaction) => transaction,
        Err(error) => {
            remove_managed_photo_copies(&files);
            return Err(error);
        }
    };
    for file in &files {
        if let Err(error) = sqlx::query(
            "UPDATE project_photos SET media_path = ?, updated_at = ? WHERE id = ? AND model_id = ?",
        )
        .bind(&file.managed_relative)
        .bind(now())
        .bind(&file.id)
        .bind(model_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)
        {
            let _ = transaction.rollback().await;
            remove_managed_photo_copies(&files);
            return Err(error);
        }
    }
    if let Err(error) = transaction.commit().await.map_err(db_error) {
        remove_managed_photo_copies(&files);
        return Err(error);
    }
    Ok(ManagedPhotoMigration { files })
}

pub(crate) struct ManagedPhotoMigration {
    files: Vec<ManagedPhotoMigrationFile>,
}

struct ManagedPhotoMigrationFile {
    id: String,
    old_relative: String,
    source: PathBuf,
    destination: PathBuf,
    managed_relative: String,
}

pub(crate) async fn rollback_managed_photo_migration(
    migration: ManagedPhotoMigration,
    state: &AppState,
) -> CommandResult<()> {
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    for file in &migration.files {
        sqlx::query("UPDATE project_photos SET media_path = ?, updated_at = ? WHERE id = ?")
            .bind(&file.old_relative)
            .bind(now())
            .bind(&file.id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    transaction.commit().await.map_err(db_error)?;
    remove_managed_photo_copies(&migration.files);
    Ok(())
}

pub(crate) fn finish_managed_photo_migration(migration: ManagedPhotoMigration) {
    for file in migration.files {
        if let Err(error) = std::fs::remove_file(&file.source) {
            log::warn!(
                "Could not remove migrated project photo {}: {error}",
                file.source.display()
            );
        }
    }
}

fn remove_managed_photo_copies(files: &[ManagedPhotoMigrationFile]) {
    for file in files {
        let _ = std::fs::remove_file(&file.destination);
    }
}

async fn ensure_project(model_id: &str, pool: &SqlitePool) -> CommandResult<()> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
        .bind(model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    if exists == 0 {
        Err("Project not found".into())
    } else {
        Ok(())
    }
}

async fn touch_project(model_id: &str, pool: &SqlitePool) -> CommandResult<()> {
    sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(model_id)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn photo_by_id(id: &str, pool: &SqlitePool) -> CommandResult<ProjectPhoto> {
    let row = sqlx::query("SELECT id, model_id, original_filename, mime_type, byte_size, width, height, caption, sort_order, is_cover, created_at, updated_at FROM project_photos WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project photo not found".to_string())?;
    Ok(photo_from_row(row))
}

fn photo_from_row(row: sqlx::sqlite::SqliteRow) -> ProjectPhoto {
    ProjectPhoto {
        id: row.get("id"),
        model_id: row.get("model_id"),
        original_filename: row.get("original_filename"),
        mime_type: row.get("mime_type"),
        byte_size: row.get("byte_size"),
        width: row.get("width"),
        height: row.get("height"),
        caption: row.get("caption"),
        sort_order: row.get("sort_order"),
        is_cover: row.get::<i64, _>("is_cover") != 0,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn field_from_row(row: sqlx::sqlite::SqliteRow) -> CustomFieldDefinition {
    CustomFieldDefinition {
        id: row.get("id"),
        name: row.get("name"),
        field_type: row.get("field_type"),
        unit: row.get("unit"),
        options: serde_json::from_str(&row.get::<String, _>("options_json")).unwrap_or_default(),
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn bom_from_row(row: sqlx::sqlite::SqliteRow) -> ProjectBomItem {
    ProjectBomItem {
        id: row.get("id"),
        model_id: row.get("model_id"),
        kind: row.get("kind"),
        linked_asset_id: row.get("linked_asset_id"),
        name: row.get("name"),
        quantity: row.get("quantity"),
        unit: row.get("unit"),
        url: row.get("url"),
        notes: row.get("notes"),
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn bom_by_id(id: &str, pool: &SqlitePool) -> CommandResult<ProjectBomItem> {
    let row = sqlx::query("SELECT id, model_id, kind, linked_asset_id, name, quantity, unit, url, notes, sort_order, created_at, updated_at FROM project_bom_items WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "BOM item not found".to_string())?;
    Ok(bom_from_row(row))
}

async fn step_by_id(id: &str, pool: &SqlitePool) -> CommandResult<AssemblyStep> {
    let row = sqlx::query("SELECT id, model_id, title, instructions, sort_order, created_at, updated_at FROM assembly_steps WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Assembly step not found".to_string())?;
    Ok(AssemblyStep {
        id: row.get("id"),
        model_id: row.get("model_id"),
        title: row.get("title"),
        instructions: row.get("instructions"),
        sort_order: row.get("sort_order"),
        bom_item_ids: sqlx::query_scalar(
            "SELECT bom_item_id FROM assembly_step_bom_items WHERE step_id = ? ORDER BY sort_order",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?,
        photo_ids: sqlx::query_scalar(
            "SELECT photo_id FROM assembly_step_photos WHERE step_id = ? ORDER BY sort_order",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(db_error)?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn validate_field_input(input: &CustomFieldInput) -> CommandResult<()> {
    if input.name.trim().is_empty() || input.name.trim().chars().count() > 80 {
        return Err("Custom field names must contain 1–80 characters".into());
    }
    if !["text", "number", "date", "boolean", "url", "select"].contains(&input.field_type.as_str())
    {
        return Err("Unknown custom field type".into());
    }
    if input
        .unit
        .as_deref()
        .is_some_and(|unit| unit.trim().chars().count() > 20)
    {
        return Err("Custom field units may contain up to 20 characters".into());
    }
    let options = normalize_options(&input.options)?;
    if input.field_type == "select" && options.is_empty() {
        return Err("Select fields need at least one option".into());
    }
    Ok(())
}

fn normalize_options(options: &[String]) -> CommandResult<Vec<String>> {
    if options.len() > 100 {
        return Err("Custom fields cannot contain more than 100 options".into());
    }
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for option in options {
        let option = option.trim();
        if option.is_empty() || option.chars().count() > 80 {
            return Err("Custom field options must contain 1–80 characters".into());
        }
        let key = option.to_lowercase();
        if seen.insert(key) {
            normalized.push(option.to_string());
        }
    }
    Ok(normalized)
}

pub(crate) fn validate_custom_value(
    field_type: &str,
    options_json: &str,
    value: &Value,
) -> CommandResult<()> {
    match field_type {
        "text" => {
            validate_string(value, 10_000, "Text field values")?;
        }
        "url" => {
            let url = validate_string(value, 2_048, "URL field values")?;
            normalize_url(Some(url))?;
        }
        "number" => {
            if !value.as_f64().is_some_and(f64::is_finite) {
                return Err("Number fields need a finite number".into());
            }
        }
        "date" => {
            let date = validate_string(value, 10, "Date field values")?;
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| "Date fields need a valid calendar date".to_string())?;
        }
        "boolean" if value.is_boolean() => {}
        "boolean" => return Err("Checkbox fields need true or false".into()),
        "select" => {
            let selected = validate_string(value, 80, "Select field values")?;
            let options: Vec<String> = serde_json::from_str(options_json).unwrap_or_default();
            if !options.iter().any(|option| option == selected) {
                return Err("Select field value is not one of its options".into());
            }
        }
        _ => return Err("Unknown custom field type".into()),
    }
    Ok(())
}

fn validate_string<'a>(value: &'a Value, max: usize, label: &str) -> CommandResult<&'a str> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("{label} must be text"))?;
    if text.chars().count() > max {
        return Err(format!("{label} may contain up to {max} characters"));
    }
    Ok(text)
}

async fn validate_bom_input(input: &ProjectBomItemInput, pool: &SqlitePool) -> CommandResult<()> {
    ensure_project(&input.model_id, pool).await?;
    if !["printed", "purchased"].contains(&input.kind.as_str()) {
        return Err("BOM items must be printed or purchased parts".into());
    }
    if input.name.trim().is_empty() || input.name.trim().chars().count() > 160 {
        return Err("BOM item names must contain 1–160 characters".into());
    }
    if !input.quantity.is_finite() || input.quantity <= 0.0 || input.quantity > 1_000_000.0 {
        return Err("BOM quantities must be greater than zero".into());
    }
    if input.unit.trim().is_empty() || input.unit.trim().chars().count() > 24 {
        return Err("BOM units must contain 1–24 characters".into());
    }
    if input.notes.chars().count() > 10_000 {
        return Err("BOM notes are too long".into());
    }
    normalize_url(input.url.as_deref())?;
    if let Some(asset_id) = &input.linked_asset_id {
        let belongs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM model_assets WHERE model_id = ? AND asset_id = ?",
        )
        .bind(&input.model_id)
        .bind(asset_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
        if belongs == 0 {
            return Err("Linked BOM file is not part of this project".into());
        }
    }
    Ok(())
}

async fn validate_step_input(input: &AssemblyStepInput, pool: &SqlitePool) -> CommandResult<()> {
    ensure_project(&input.model_id, pool).await?;
    if input.title.trim().is_empty() || input.title.trim().chars().count() > 160 {
        return Err("Assembly step titles must contain 1–160 characters".into());
    }
    if input.instructions.chars().count() > 50_000 {
        return Err("Assembly instructions are too long".into());
    }
    if input.bom_item_ids.len() > 500 || input.photo_ids.len() > 100 {
        return Err("Assembly steps contain too many linked items".into());
    }
    for item_id in &input.bom_item_ids {
        let belongs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_bom_items WHERE id = ? AND model_id = ?",
        )
        .bind(item_id)
        .bind(&input.model_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
        if belongs == 0 {
            return Err("Assembly step links a BOM item from another project".into());
        }
    }
    for photo_id in &input.photo_ids {
        let belongs: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM project_photos WHERE id = ? AND model_id = ?")
                .bind(photo_id)
                .bind(&input.model_id)
                .fetch_one(pool)
                .await
                .map_err(db_error)?;
        if belongs == 0 {
            return Err("Assembly step links a photo from another project".into());
        }
    }
    Ok(())
}

fn normalize_url(value: Option<&str>) -> CommandResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > 2_048 {
        return Err("URLs may contain up to 2,048 characters".into());
    }
    let parsed =
        reqwest::Url::parse(value).map_err(|_| "Enter a valid HTTP or HTTPS URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS URLs are supported".into());
    }
    Ok(Some(parsed.to_string()))
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
