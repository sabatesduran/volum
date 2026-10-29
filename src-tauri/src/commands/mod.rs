use crate::{domain::*, indexing, parsers, previews, project_docs, state::AppState, web_sources};
use chrono::{NaiveDate, Utc};
use serde_json::json;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;

#[derive(Debug)]
struct DuplicateMatchRow {
    match_key: String,
    value: String,
    match_kind: String,
    confidence: f64,
    byte_size: i64,
    model_count: i64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelPageCursor {
    sort: String,
    primary: String,
    secondary: String,
    name: String,
    id: String,
}

#[tauri::command]
pub async fn list_roots(state: State<'_, AppState>) -> CommandResult<Vec<LibraryRoot>> {
    let rows = sqlx::query("SELECT r.id, r.path, r.display_name, r.status, r.last_scan_at, r.root_kind, COUNT(DISTINCT CASE WHEN m.missing_since IS NULL AND m.trashed_at IS NULL THEN m.id END) model_count FROM library_roots r LEFT JOIN folders f ON f.root_id = r.id LEFT JOIN models m ON m.folder_id = f.id GROUP BY r.id ORDER BY CASE r.root_kind WHEN 'managed' THEN 0 ELSE 1 END, r.created_at").fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| LibraryRoot {
            id: row.get("id"),
            path: row.get("path"),
            display_name: row.get("display_name"),
            status: row.get("status"),
            last_scan_at: row.get("last_scan_at"),
            model_count: row.get("model_count"),
            root_kind: row.get("root_kind"),
        })
        .collect())
}

#[tauri::command]
pub async fn add_library_root(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<LibraryRoot> {
    let canonical = std::fs::canonicalize(&path)
        .map_err(|_| "Choose a folder that is currently available".to_string())?;
    if !canonical.is_dir() {
        return Err("The selected path is not a folder".into());
    }
    let normalized = canonical.to_string_lossy().to_string();
    let managed_paths: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM library_roots WHERE root_kind = 'managed' AND status != 'removed'",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;
    if managed_paths
        .iter()
        .map(PathBuf::from)
        .any(|managed| canonical.starts_with(&managed) || managed.starts_with(&canonical))
    {
        return Err("A linked library cannot overlap the managed Volum library".into());
    }
    if let Some(row) = sqlx::query(
        "SELECT id, path, display_name, status, last_scan_at, root_kind FROM library_roots WHERE path = ?",
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?
    {
        return Ok(LibraryRoot {
            id: row.get("id"),
            path: row.get("path"),
            display_name: row.get("display_name"),
            status: row.get("status"),
            last_scan_at: row.get("last_scan_at"),
            model_count: 0,
            root_kind: row.get("root_kind"),
        });
    }
    let id = Uuid::new_v4().to_string();
    let display_name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Models")
        .to_string();
    let timestamp = now();
    sqlx::query("INSERT INTO library_roots (id, path, display_name, status, created_at, updated_at) VALUES (?, ?, ?, 'online', ?, ?)").bind(&id).bind(&normalized).bind(&display_name).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.map_err(db_error)?;
    Ok(LibraryRoot {
        id,
        path: normalized,
        display_name,
        status: "online".into(),
        last_scan_at: None,
        model_count: 0,
        root_kind: "linked".into(),
    })
}

#[tauri::command]
pub async fn remove_library_root(
    root_id: String,
    keep_metadata: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let kind: Option<String> =
        sqlx::query_scalar("SELECT root_kind FROM library_roots WHERE id = ?")
            .bind(&root_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    if kind.as_deref() == Some("managed") {
        return Err("The managed Volum library cannot be removed as a linked folder".into());
    }
    if keep_metadata {
        sqlx::query("UPDATE library_roots SET status = 'offline', updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(&root_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    } else {
        sqlx::query("DELETE FROM library_roots WHERE id = ?")
            .bind(&root_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    if let Ok(mut watchers) = state.watchers.lock() {
        watchers.remove(&root_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn reconnect_library_root(
    root_id: String,
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<LibraryRoot> {
    let canonical = std::fs::canonicalize(&path)
        .map_err(|_| "Choose a folder that is currently available".to_string())?;
    if !canonical.is_dir() {
        return Err("The selected path is not a folder".into());
    }
    let normalized = canonical.to_string_lossy().to_string();
    let root_kind: String = sqlx::query_scalar("SELECT root_kind FROM library_roots WHERE id = ?")
        .bind(&root_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Library not found".to_string())?;
    if root_kind == "managed" && !canonical.join(".volum-library.json").is_file() {
        return Err("Choose the original managed Volum library folder".into());
    }
    let other_roots = sqlx::query(
        "SELECT path, root_kind FROM library_roots WHERE id != ? AND status != 'removed'",
    )
    .bind(&root_id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;
    for other in other_roots {
        let other_path = PathBuf::from(other.get::<String, _>("path"));
        if (root_kind == "managed" || other.get::<String, _>("root_kind") == "managed")
            && (canonical.starts_with(&other_path) || other_path.starts_with(&canonical))
        {
            return Err("Managed and linked libraries cannot overlap".into());
        }
    }
    let conflict: Option<String> =
        sqlx::query_scalar("SELECT id FROM library_roots WHERE path = ? AND id != ?")
            .bind(&normalized)
            .bind(&root_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    if conflict.is_some() {
        return Err("That folder is already a Volum library".into());
    }
    let timestamp = now();
    let updated = sqlx::query(
        "UPDATE library_roots SET path = ?, status = 'online', updated_at = ? WHERE id = ?",
    )
    .bind(&normalized)
    .bind(&timestamp)
    .bind(&root_id)
    .execute(&state.pool)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 0 {
        return Err("Library not found".into());
    }
    if let Ok(mut watchers) = state.watchers.lock() {
        watchers.remove(&root_id);
    }
    let row = sqlx::query("SELECT id, path, display_name, status, last_scan_at, root_kind, (SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = library_roots.id AND m.missing_since IS NULL) model_count FROM library_roots WHERE id = ?")
        .bind(&root_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(LibraryRoot {
        id: row.get("id"),
        path: row.get("path"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        last_scan_at: row.get("last_scan_at"),
        model_count: row.get("model_count"),
        root_kind: row.get("root_kind"),
    })
}

#[tauri::command]
pub async fn start_scan(
    root_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    indexing::start(root_id, &state, Some(app)).await
}

#[tauri::command]
pub async fn pause_scan(root_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    indexing::pause(&root_id, &state).await
}

#[tauri::command]
pub async fn get_scan_status(
    root_id: String,
    state: State<'_, AppState>,
) -> CommandResult<ScanStatus> {
    Ok(indexing::status(&root_id, &state).await)
}

#[tauri::command]
pub async fn list_folders(
    root_id: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Folder>> {
    list_folders_impl(root_id, state.inner()).await
}

pub(crate) async fn list_folders_impl(
    root_id: Option<String>,
    state: &AppState,
) -> CommandResult<Vec<Folder>> {
    let rows = sqlx::query("SELECT f.id, f.root_id, f.parent_id, f.relative_path, f.name, COUNT(m.id) direct_count FROM folders f LEFT JOIN models m ON m.folder_id = f.id AND m.missing_since IS NULL AND m.trashed_at IS NULL WHERE (? IS NULL OR f.root_id = ?) AND f.relative_path != '' AND f.relative_path NOT LIKE 'Trash/%' GROUP BY f.id ORDER BY lower(f.relative_path)")
        .bind(&root_id)
        .bind(&root_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let mut folders = Vec::with_capacity(rows.len());
    let mut counts = HashMap::<String, i64>::with_capacity(rows.len());
    let mut parents = HashMap::<String, Option<String>>::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let parent_id: Option<String> = row.get("parent_id");
        counts.insert(id.clone(), row.get("direct_count"));
        parents.insert(id.clone(), parent_id.clone());
        folders.push(Folder {
            id,
            root_id: row.get("root_id"),
            parent_id,
            relative_path: row.get("relative_path"),
            name: row.get("name"),
            model_count: 0,
        });
    }
    let mut by_depth = folders
        .iter()
        .map(|folder| (folder.relative_path.matches('/').count(), folder.id.clone()))
        .collect::<Vec<_>>();
    by_depth.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
    for (_, id) in by_depth {
        let count = counts.get(&id).copied().unwrap_or_default();
        if let Some(Some(parent_id)) = parents.get(&id) {
            *counts.entry(parent_id.clone()).or_default() += count;
        }
    }
    for folder in &mut folders {
        folder.model_count = counts.get(&folder.id).copied().unwrap_or_default();
    }
    Ok(folders)
}

#[tauri::command]
pub async fn list_models(
    query: ModelQuery,
    state: State<'_, AppState>,
) -> CommandResult<Page<ModelSummary>> {
    list_models_impl(query, state.inner()).await
}

pub(crate) async fn list_models_impl(
    query: ModelQuery,
    state: &AppState,
) -> CommandResult<Page<ModelSummary>> {
    let search = fts_query(query.search.as_deref().unwrap_or(""));
    let search_match = if search.is_empty() {
        "\"__volum_empty_query__\"".to_string()
    } else {
        search.clone()
    };
    let folder = query.folder_id.clone().unwrap_or_default();
    let requested_collection = query.collection_id.clone().unwrap_or_default();
    let mut collection = requested_collection.clone();
    let mut smart_rule = SmartCollectionRule::default();
    if !requested_collection.is_empty() {
        if let Some(row) = sqlx::query("SELECT kind, rule_json FROM collections WHERE id = ?")
            .bind(&requested_collection)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
        {
            if row.get::<String, _>("kind") == "smart" {
                collection.clear();
                smart_rule =
                    serde_json::from_str(&row.get::<String, _>("rule_json")).unwrap_or_default();
            }
        }
    }
    let format = query
        .format
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace("stp", "step");
    let availability = query.availability.clone().unwrap_or_default();
    let tag = query.tag_id.clone().unwrap_or_default();
    let rich_smart_sql = smart_rule_sql(&smart_rule)?;
    let smart_tag = smart_rule.tag_id.unwrap_or_default();
    let smart_format = smart_rule
        .format
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace("stp", "step");
    let smart_availability = smart_rule.availability.unwrap_or_default();
    let date_from = query.date_from.clone().unwrap_or_default();
    let date_to = query.date_to.clone().unwrap_or_default();
    let date_column = if query.date_field.as_deref() == Some("added") {
        "m.added_at"
    } else {
        "COALESCE(m.asset_modified_at, m.updated_at)"
    };
    let favorite = i64::from(query.favorite.unwrap_or(false));
    let recent = i64::from(query.recent.unwrap_or(false));
    let project_filter_sql = model_query_sql(&query)?;
    let duplicates = 0_i64;
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let trash_condition = if query.trashed.unwrap_or(false) {
        "m.trashed_at IS NOT NULL"
    } else {
        "m.trashed_at IS NULL"
    };
    let where_sql = format!(" WHERE {trash_condition} AND (? = '' OR m.id IN (SELECT model_id FROM model_search WHERE model_search MATCH ?)) AND (? = '' OR m.folder_id IN (SELECT descendant.id FROM folders descendant JOIN folders selected ON selected.id = ? WHERE descendant.root_id = selected.root_id AND (descendant.relative_path = selected.relative_path OR substr(descendant.relative_path, 1, length(selected.relative_path) + 1) = selected.relative_path || '/'))) AND (? = '' OR EXISTS (SELECT 1 FROM collection_items ci WHERE ci.model_id = m.id AND ci.collection_id = ?)) AND (? = 0 OR m.favorite = 1) AND (? = 0 OR m.last_opened_at IS NOT NULL) AND (? = '' OR EXISTS (SELECT 1 FROM model_assets fma JOIN assets fa ON fa.id = fma.asset_id WHERE fma.model_id = m.id AND CASE fa.extension WHEN 'stp' THEN 'step' ELSE fa.extension END = ?)) AND (? = '' OR (? = 'available' AND m.missing_since IS NULL) OR (? = 'offline' AND m.missing_since IS NOT NULL)) AND (? = '' OR EXISTS (SELECT 1 FROM model_tags mt WHERE mt.model_id = m.id AND mt.tag_id = ?)) AND (? = '' OR EXISTS (SELECT 1 FROM model_assets sfma JOIN assets sfa ON sfa.id = sfma.asset_id WHERE sfma.model_id = m.id AND CASE sfa.extension WHEN 'stp' THEN 'step' ELSE sfa.extension END = ?)) AND (? = '' OR (? = 'available' AND m.missing_since IS NULL) OR (? = 'offline' AND m.missing_since IS NOT NULL)) AND (? = '' OR EXISTS (SELECT 1 FROM model_tags smt WHERE smt.model_id = m.id AND smt.tag_id = ?)) AND (? = '' OR date({date_column}) >= date(?)) AND (? = '' OR date({date_column}) <= date(?)) AND (? = 0 OR EXISTS (SELECT 1 FROM model_assets dma JOIN assets da ON da.id = dma.asset_id LEFT JOIN asset_geometry dg ON dg.asset_id = da.id WHERE dma.model_id = m.id AND ((da.content_hash IS NOT NULL AND EXISTS (SELECT 1 FROM model_assets oma JOIN assets oa ON oa.id = oma.asset_id WHERE oma.model_id != m.id AND oa.content_hash = da.content_hash)) OR (dg.geometry_hash IS NOT NULL AND EXISTS (SELECT 1 FROM model_assets oma JOIN asset_geometry og ON og.asset_id = oma.asset_id WHERE oma.model_id != m.id AND og.geometry_hash = dg.geometry_hash))))) AND ({rich_smart_sql}) AND ({project_filter_sql}) ");
    let count_sql = format!("SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id{where_sql}");
    let total: i64 = if query.cursor.is_none() && offset == 0 {
        sqlx::query_scalar(&count_sql)
            .bind(&search)
            .bind(&search_match)
            .bind(&folder)
            .bind(&folder)
            .bind(&collection)
            .bind(&collection)
            .bind(favorite)
            .bind(recent)
            .bind(&format)
            .bind(&format)
            .bind(&availability)
            .bind(&availability)
            .bind(&availability)
            .bind(&tag)
            .bind(&tag)
            .bind(&smart_format)
            .bind(&smart_format)
            .bind(&smart_availability)
            .bind(&smart_availability)
            .bind(&smart_availability)
            .bind(&smart_tag)
            .bind(&smart_tag)
            .bind(&date_from)
            .bind(&date_from)
            .bind(&date_to)
            .bind(&date_to)
            .bind(duplicates)
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?
    } else {
        -1
    };
    let order = match query.sort.as_deref() {
        Some("name") => "m.display_name COLLATE NOCASE ASC, m.id ASC",
        Some("added") => "m.added_at DESC, m.display_name COLLATE NOCASE ASC, m.id ASC",
        Some("opened") => {
            "COALESCE(m.last_opened_at, '') DESC, COALESCE(m.asset_modified_at, m.updated_at) DESC, m.display_name COLLATE NOCASE ASC, m.id ASC"
        }
        _ => "COALESCE(m.asset_modified_at, m.updated_at) DESC, m.display_name COLLATE NOCASE ASC, m.id ASC",
    };
    let cursor = query
        .cursor
        .as_deref()
        .map(serde_json::from_str::<ModelPageCursor>)
        .transpose()
        .map_err(|_| "Invalid model page cursor".to_string())?;
    let cursor_condition = cursor
        .as_ref()
        .map(|cursor| model_cursor_condition(query.sort.as_deref(), cursor))
        .transpose()?;
    let page_where_sql = match cursor_condition {
        Some(condition) => format!("{where_sql} AND ({condition})"),
        None => where_sql,
    };
    let data_sql = format!("SELECT m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(m.asset_modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, m.bundle_mode, m.storage_mode, (SELECT pp.id FROM project_photos pp WHERE pp.model_id = m.id AND pp.is_cover = 1 LIMIT 1) cover_photo_id, m.asset_count, COALESCE(pa.metadata_json, '{{}}') metadata_json FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id{page_where_sql} ORDER BY {order} LIMIT ? OFFSET ?");
    let rows = sqlx::query(&data_sql)
        .bind(&search)
        .bind(&search_match)
        .bind(&folder)
        .bind(&folder)
        .bind(&collection)
        .bind(&collection)
        .bind(favorite)
        .bind(recent)
        .bind(&format)
        .bind(&format)
        .bind(&availability)
        .bind(&availability)
        .bind(&availability)
        .bind(&tag)
        .bind(&tag)
        .bind(&smart_format)
        .bind(&smart_format)
        .bind(&smart_availability)
        .bind(&smart_availability)
        .bind(&smart_availability)
        .bind(&smart_tag)
        .bind(&smart_tag)
        .bind(&date_from)
        .bind(&date_from)
        .bind(&date_to)
        .bind(&date_to)
        .bind(duplicates)
        .bind(limit + 1)
        .bind(if cursor.is_some() { 0 } else { offset })
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let mut items = rows
        .into_iter()
        .map(summary_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = items.len() as i64 > limit;
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items
            .last()
            .map(|item| encode_model_cursor(query.sort.as_deref(), item))
            .transpose()?
    } else {
        None
    };
    Ok(Page {
        items,
        total,
        next_offset: if query.cursor.is_none() && total >= 0 && offset + limit < total {
            Some(offset + limit)
        } else {
            None
        },
        next_cursor,
    })
}

#[tauri::command]
pub async fn get_model(model_id: String, state: State<'_, AppState>) -> CommandResult<ModelDetail> {
    get_model_impl(model_id, state.inner()).await
}

pub(crate) async fn get_model_impl(
    model_id: String,
    state: &AppState,
) -> CommandResult<ModelDetail> {
    let row = sqlx::query("SELECT m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(m.asset_modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, m.bundle_mode, m.storage_mode, m.asset_count, COALESCE(pa.metadata_json, '{}') metadata_json, m.notes, r.id root_id, r.display_name root_name, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id WHERE m.id = ?").bind(&model_id).fetch_optional(&state.pool).await.map_err(db_error)?.ok_or_else(|| "Project not found".to_string())?;
    let notes = row.get("notes");
    let root_id = row.get("root_id");
    let root_name = row.get("root_name");
    let root_path = row.get("root_path");
    let summary = summary_from_row(row)?;
    let rows = sqlx::query("SELECT a.id, a.filename, a.extension, a.relative_path, a.byte_size, a.modified_at, a.parse_status, a.metadata_json, a.missing_since, ma.role FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? ORDER BY ma.sort_order, CASE a.extension WHEN '3mf' THEN 0 WHEN 'step' THEN 1 WHEN 'stp' THEN 1 WHEN 'stl' THEN 2 WHEN 'obj' THEN 3 ELSE 4 END, a.filename").bind(&model_id).fetch_all(&state.pool).await.map_err(db_error)?;
    let assets = rows
        .into_iter()
        .map(|row| -> Result<Asset, String> {
            Ok(Asset {
                id: row.get("id"),
                filename: row.get("filename"),
                extension: row.get("extension"),
                relative_path: row.get("relative_path"),
                byte_size: row.get("byte_size"),
                modified_at: row.get("modified_at"),
                parse_status: row.get("parse_status"),
                metadata: serde_json::from_str(&row.get::<String, _>("metadata_json"))
                    .unwrap_or_default(),
                missing: row.get::<Option<String>, _>("missing_since").is_some(),
                role: row.get("role"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let collection_ids =
        sqlx::query_scalar("SELECT collection_id FROM collection_items WHERE model_id = ?")
            .bind(&model_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let tag_rows = sqlx::query("SELECT t.id, t.name, t.color, (SELECT COUNT(*) FROM model_tags all_tags WHERE all_tags.tag_id = t.id) model_count FROM tags t JOIN model_tags mt ON mt.tag_id = t.id WHERE mt.model_id = ? ORDER BY lower(t.name)")
        .bind(&model_id)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let tags = tag_rows.into_iter().map(tag_from_row).collect();
    let estimate = get_estimate(&model_id, state).await?;
    let web_source = web_sources::source_for_model(&model_id, state).await?;
    Ok(ModelDetail {
        summary,
        notes,
        root_id,
        root_name,
        root_path,
        assets,
        collection_ids,
        tags,
        estimate,
        web_source,
    })
}

#[tauri::command]
pub async fn toggle_favorite(model_id: String, state: State<'_, AppState>) -> CommandResult<bool> {
    sqlx::query("UPDATE models SET favorite = NOT favorite, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(&model_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT favorite FROM models WHERE id = ?")
            .bind(&model_id)
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?
            != 0,
    )
}

#[tauri::command]
pub async fn set_favorite(
    model_ids: Vec<String>,
    favorite: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    for model_id in model_ids {
        sqlx::query("UPDATE models SET favorite = ?, updated_at = ? WHERE id = ?")
            .bind(i64::from(favorite))
            .bind(now())
            .bind(model_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_notes(
    model_id: String,
    notes: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if notes.len() > 100_000 {
        return Err("Notes are too long".into());
    }
    sqlx::query("UPDATE models SET notes = ?, updated_at = ? WHERE id = ?")
        .bind(notes)
        .bind(now())
        .bind(&model_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    indexing::refresh_search(&model_id, &state.pool).await
}

#[tauri::command]
pub async fn list_saved_searches(state: State<'_, AppState>) -> CommandResult<Vec<SavedSearch>> {
    list_saved_searches_impl(state.inner()).await
}

pub(crate) async fn list_saved_searches_impl(state: &AppState) -> CommandResult<Vec<SavedSearch>> {
    let rows = sqlx::query("SELECT id, name, query_json, created_at, updated_at FROM saved_searches ORDER BY sort_order, lower(name)")
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| SavedSearch {
            id: row.get("id"),
            name: row.get("name"),
            query: serde_json::from_str(&row.get::<String, _>("query_json")).unwrap_or_default(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

#[tauri::command]
pub async fn save_saved_search(
    input: SavedSearchInput,
    state: State<'_, AppState>,
) -> CommandResult<SavedSearch> {
    save_saved_search_impl(input, &state).await
}

pub(crate) async fn save_saved_search_impl(
    input: SavedSearchInput,
    state: &AppState,
) -> CommandResult<SavedSearch> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("Saved search names must contain 1–80 characters".into());
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut query = input.query;
    query.offset = None;
    query.limit = None;
    let query_json = serde_json::to_string(&query).map_err(|error| error.to_string())?;
    let timestamp = now();
    let created_at: Option<String> =
        sqlx::query_scalar("SELECT created_at FROM saved_searches WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    let created_at = created_at.unwrap_or_else(|| timestamp.clone());
    let order: f64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM saved_searches",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    sqlx::query("INSERT INTO saved_searches (id, name, query_json, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, query_json = excluded.query_json, updated_at = excluded.updated_at")
        .bind(&id)
        .bind(name)
        .bind(query_json)
        .bind(order)
        .bind(&created_at)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(SavedSearch {
        id,
        name: name.to_string(),
        query,
        created_at,
        updated_at: timestamp,
    })
}

#[tauri::command]
pub async fn delete_saved_search(
    search_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    delete_saved_search_impl(&search_id, &state).await
}

pub(crate) async fn delete_saved_search_impl(
    search_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    sqlx::query("DELETE FROM saved_searches WHERE id = ?")
        .bind(search_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> CommandResult<Vec<Collection>> {
    list_collections_impl(state.inner()).await
}

pub(crate) async fn list_collections_impl(state: &AppState) -> CommandResult<Vec<Collection>> {
    let rows = sqlx::query("SELECT c.id, c.name, c.symbol, c.color, c.created_at, c.kind, c.rule_json, COUNT(ci.model_id) model_count FROM collections c LEFT JOIN collection_items ci ON ci.collection_id = c.id GROUP BY c.id ORDER BY c.sort_order, lower(c.name)").fetch_all(&state.pool).await.map_err(db_error)?;
    let mut collections = Vec::with_capacity(rows.len());
    let mut smart_count_queries = Vec::new();
    for row in rows {
        let smart = row.get::<String, _>("kind") == "smart";
        let rule = smart.then(|| {
            serde_json::from_str::<SmartCollectionRule>(&row.get::<String, _>("rule_json"))
                .unwrap_or_default()
        });
        let model_count = row.get("model_count");
        let id: String = row.get("id");
        if let Some(rule) = &rule {
            smart_count_queries.push(format!(
                "SELECT {} collection_id, ({}) model_count",
                sql_string(&id),
                smart_collection_count_sql(rule)?
            ));
        }
        collections.push(Collection {
            id,
            name: row.get("name"),
            symbol: row.get("symbol"),
            color: row.get("color"),
            model_count,
            created_at: row.get("created_at"),
            smart,
            rule,
        });
    }
    if !smart_count_queries.is_empty() {
        let smart_counts = sqlx::query(&smart_count_queries.join(" UNION ALL "))
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
        let counts = smart_counts
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("collection_id"),
                    row.get::<i64, _>("model_count"),
                )
            })
            .collect::<HashMap<_, _>>();
        for collection in &mut collections {
            if let Some(count) = counts.get(&collection.id) {
                collection.model_count = *count;
            }
        }
    }
    Ok(collections)
}

#[tauri::command]
pub async fn create_collection(
    input: CollectionInput,
    state: State<'_, AppState>,
) -> CommandResult<Collection> {
    create_collection_impl(input, state.inner()).await
}

pub(crate) async fn create_collection_impl(
    mut input: CollectionInput,
    state: &AppState,
) -> CommandResult<Collection> {
    input.name = input.name.trim().to_string();
    validate_collection(&input)?;
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let order: f64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(MAX(sort_order), 0) + 1 AS REAL) FROM collections",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(db_error)?;
    let kind = if input.smart { "smart" } else { "manual" };
    let rule_json = serde_json::to_string(&input.rule.clone().unwrap_or_default())
        .map_err(|error| error.to_string())?;
    sqlx::query("INSERT INTO collections (id, name, symbol, color, sort_order, created_at, updated_at, kind, rule_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)").bind(&id).bind(&input.name).bind(&input.symbol).bind(&input.color).bind(order).bind(&timestamp).bind(&timestamp).bind(kind).bind(rule_json).execute(&state.pool).await.map_err(|error| {
        if error.to_string().contains("UNIQUE") { "A collection with that name already exists".into() } else { db_error(error) }
    })?;
    let model_count = if input.smart {
        count_smart_collection(input.rule.as_ref().unwrap(), &state.pool).await?
    } else {
        0
    };
    Ok(Collection {
        id,
        name: input.name,
        symbol: input.symbol,
        color: input.color,
        model_count,
        created_at: timestamp,
        smart: input.smart,
        rule: input.rule,
    })
}

#[tauri::command]
pub async fn update_collection(
    collection_id: String,
    input: CollectionInput,
    state: State<'_, AppState>,
) -> CommandResult<Collection> {
    update_collection_impl(&collection_id, input, state.inner()).await
}

pub(crate) async fn update_collection_impl(
    collection_id: &str,
    mut input: CollectionInput,
    state: &AppState,
) -> CommandResult<Collection> {
    input.name = input.name.trim().to_string();
    validate_collection(&input)?;
    let kind = if input.smart { "smart" } else { "manual" };
    let rule_json = serde_json::to_string(&input.rule.clone().unwrap_or_default())
        .map_err(|error| error.to_string())?;
    let result = sqlx::query(
        "UPDATE collections SET name = ?, symbol = ?, color = ?, kind = ?, rule_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.symbol)
    .bind(&input.color)
    .bind(kind)
    .bind(rule_json)
    .bind(now())
    .bind(collection_id)
    .execute(&state.pool)
    .await
    .map_err(|error| {
        if error.to_string().contains("UNIQUE") { "A collection with that name already exists".into() } else { db_error(error) }
    })?;
    if result.rows_affected() == 0 {
        return Err("Collection not found".into());
    }
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM collection_items WHERE collection_id = ?")
            .bind(collection_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let timestamp = now();
    for id in &model_ids {
        sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
            .bind(&timestamp)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(id, &state.pool).await?;
    }
    list_collections_impl(state)
        .await?
        .into_iter()
        .find(|collection| collection.id == collection_id)
        .ok_or_else(|| "Collection not found".into())
}

#[tauri::command]
pub async fn delete_collection(
    collection_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    delete_collection_impl(&collection_id, state.inner()).await
}

pub(crate) async fn delete_collection_impl(
    collection_id: &str,
    state: &AppState,
) -> CommandResult<()> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM collection_items WHERE collection_id = ?")
            .bind(collection_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let result = sqlx::query("DELETE FROM collections WHERE id = ?")
        .bind(collection_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err("Collection not found".into());
    }
    let timestamp = now();
    for id in ids {
        sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
            .bind(&timestamp)
            .bind(&id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(&id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn add_models_to_collection(
    collection_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    set_models_collection_impl(&collection_id, &model_ids, true, state.inner()).await
}

#[tauri::command]
pub async fn remove_models_from_collection(
    collection_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    set_models_collection_impl(&collection_id, &model_ids, false, state.inner()).await
}

pub(crate) async fn set_models_collection_impl(
    collection_id: &str,
    model_ids: &[String],
    selected: bool,
    state: &AppState,
) -> CommandResult<()> {
    ensure_manual_collection(collection_id, &state.pool).await?;
    let timestamp = now();
    let mut changed = Vec::new();
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    for model_id in model_ids {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM models WHERE id = ? AND trashed_at IS NULL)",
        )
        .bind(model_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(db_error)?;
        if !exists {
            return Err("Project not found".into());
        }
        let result = if selected {
            sqlx::query("INSERT OR IGNORE INTO collection_items (collection_id, model_id, position, added_at) VALUES (?, ?, (SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM collection_items WHERE collection_id = ?), ?)")
                .bind(collection_id)
                .bind(model_id)
                .bind(collection_id)
                .bind(&timestamp)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?
        } else {
            sqlx::query("DELETE FROM collection_items WHERE collection_id = ? AND model_id = ?")
                .bind(collection_id)
                .bind(model_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?
        };
        if result.rows_affected() > 0 {
            sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
                .bind(&timestamp)
                .bind(model_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
            changed.push(model_id.clone());
        }
    }
    transaction.commit().await.map_err(db_error)?;
    for model_id in changed {
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> CommandResult<Vec<Tag>> {
    list_tags_impl(state.inner()).await
}

pub(crate) async fn list_tags_impl(state: &AppState) -> CommandResult<Vec<Tag>> {
    let rows = sqlx::query("SELECT t.id, t.name, t.color, COUNT(mt.model_id) model_count FROM tags t LEFT JOIN model_tags mt ON mt.tag_id = t.id GROUP BY t.id ORDER BY lower(t.name)")
        .fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows.into_iter().map(tag_from_row).collect())
}

#[tauri::command]
pub async fn save_tag(input: TagInput, state: State<'_, AppState>) -> CommandResult<Tag> {
    save_tag_impl(input, state.inner()).await
}

pub(crate) async fn save_tag_impl(input: TagInput, state: &AppState) -> CommandResult<Tag> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Tag names must contain 1–40 characters".into());
    }
    if !valid_color(&input.color) {
        return Err("Tag color must be a hex color".into());
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM model_tags WHERE tag_id = ?")
            .bind(&id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let timestamp = now();
    sqlx::query("INSERT INTO tags (id, name, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color, updated_at = excluded.updated_at")
        .bind(&id).bind(name).bind(&input.color).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.map_err(|error| {
            if error.to_string().contains("UNIQUE") { "A tag with that name already exists".into() } else { db_error(error) }
        })?;
    for model_id in model_ids {
        sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
            .bind(&timestamp)
            .bind(&model_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    list_tags_impl(state)
        .await?
        .into_iter()
        .find(|tag| tag.id == id)
        .ok_or_else(|| "Tag not found".into())
}

#[tauri::command]
pub async fn delete_tag(tag_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    delete_tag_impl(&tag_id, state.inner()).await
}

pub(crate) async fn delete_tag_impl(tag_id: &str, state: &AppState) -> CommandResult<()> {
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM model_tags WHERE tag_id = ?")
            .bind(tag_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    let result = sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(tag_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err("Tag not found".into());
    }
    let timestamp = now();
    for id in model_ids {
        sqlx::query("UPDATE models SET updated_at = ? WHERE id = ?")
            .bind(&timestamp)
            .bind(&id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(&id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_model_tags(
    model_id: String,
    tag_ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if tag_ids.len() > 100 {
        return Err("A model cannot have more than 100 tags".into());
    }
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    sqlx::query("DELETE FROM model_tags WHERE model_id = ?")
        .bind(&model_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    let timestamp = now();
    for tag_id in tag_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO model_tags (model_id, tag_id, added_at) VALUES (?, ?, ?)",
        )
        .bind(&model_id)
        .bind(tag_id)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    }
    transaction.commit().await.map_err(db_error)?;
    indexing::refresh_search(&model_id, &state.pool).await
}

#[tauri::command]
pub async fn add_models_to_tag(
    tag_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
        .bind(&tag_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    if exists == 0 {
        return Err("Tag not found".into());
    }
    let timestamp = now();
    for model_id in model_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO model_tags (model_id, tag_id, added_at) VALUES (?, ?, ?)",
        )
        .bind(&model_id)
        .bind(&tag_id)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_related_models(
    model_id: String,
    state: State<'_, AppState>,
) -> CommandResult<Vec<RelatedModel>> {
    list_related_models_impl(&model_id, state.inner()).await
}

pub(crate) async fn list_related_models_impl(
    model_id: &str,
    state: &AppState,
) -> CommandResult<Vec<RelatedModel>> {
    let version_key: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(version_key, '') FROM models WHERE id = ?")
            .bind(model_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .flatten();
    let rows = sqlx::query("SELECT DISTINCT other.id, other.display_name, f.relative_path, COALESCE(pa.extension, '') primary_extension, COALESCE((SELECT MAX(project_asset.modified_at) FROM model_assets project_membership JOIN assets project_asset ON project_asset.id = project_membership.asset_id WHERE project_membership.model_id = other.id), other.updated_at) modified_at, CASE WHEN EXISTS (SELECT 1 FROM model_assets mine JOIN assets mine_asset ON mine_asset.id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN assets their_asset ON their_asset.id = theirs.asset_id AND their_asset.content_hash = mine_asset.content_hash WHERE mine.model_id = ? AND mine_asset.content_hash IS NOT NULL) THEN 'duplicate' WHEN EXISTS (SELECT 1 FROM model_assets mine JOIN asset_geometry mine_geometry ON mine_geometry.asset_id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN asset_geometry their_geometry ON their_geometry.asset_id = theirs.asset_id AND their_geometry.geometry_hash = mine_geometry.geometry_hash WHERE mine.model_id = ?) THEN 'geometry' ELSE 'version' END relationship FROM models other JOIN folders f ON f.id = other.folder_id LEFT JOIN assets pa ON pa.id = other.primary_asset_id WHERE other.id != ? AND (EXISTS (SELECT 1 FROM model_assets mine JOIN assets mine_asset ON mine_asset.id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN assets their_asset ON their_asset.id = theirs.asset_id AND their_asset.content_hash = mine_asset.content_hash WHERE mine.model_id = ? AND mine_asset.content_hash IS NOT NULL) OR EXISTS (SELECT 1 FROM model_assets mine JOIN asset_geometry mine_geometry ON mine_geometry.asset_id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN asset_geometry their_geometry ON their_geometry.asset_id = theirs.asset_id AND their_geometry.geometry_hash = mine_geometry.geometry_hash WHERE mine.model_id = ?) OR (? != '' AND other.version_key = ?)) ORDER BY relationship, modified_at DESC LIMIT 50")
        .bind(model_id).bind(model_id).bind(model_id).bind(model_id).bind(model_id).bind(version_key.clone().unwrap_or_default()).bind(version_key.unwrap_or_default()).fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| RelatedModel {
            id: row.get("id"),
            display_name: row.get("display_name"),
            relative_folder: row.get("relative_path"),
            primary_extension: row.get("primary_extension"),
            modified_at: row.get("modified_at"),
            relationship: row.get("relationship"),
        })
        .collect())
}

#[tauri::command]
pub async fn get_duplicate_stats(state: State<'_, AppState>) -> CommandResult<DuplicateStats> {
    get_duplicate_stats_impl(&state).await
}

pub(crate) async fn get_duplicate_stats_impl(state: &AppState) -> CommandResult<DuplicateStats> {
    let row = sqlx::query(&format!("WITH matches AS ({}) SELECT COUNT(*) groups, COALESCE(SUM(model_count), 0) models, COALESCE(SUM(model_count - 1), 0) redundant_copies FROM matches", duplicate_matches_sql()))
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(DuplicateStats {
        groups: row.get("groups"),
        models: row.get("models"),
        redundant_copies: row.get("redundant_copies"),
    })
}

#[tauri::command]
pub async fn list_duplicate_groups(
    offset: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> CommandResult<Page<DuplicateGroup>> {
    list_duplicate_groups_impl(offset, limit, &state).await
}

pub(crate) async fn list_duplicate_groups_impl(
    offset: i64,
    limit: i64,
    state: &AppState,
) -> CommandResult<Page<DuplicateGroup>> {
    let offset = offset.max(0);
    let limit = limit.clamp(1, 100);
    let (matches, total) = duplicate_match_rows_page(&state.pool, offset, limit).await?;
    let exact_values = matches
        .iter()
        .filter(|item| item.match_kind == "exact")
        .map(|item| item.value.clone())
        .collect::<Vec<_>>();
    let geometry_values = matches
        .iter()
        .filter(|item| item.match_kind == "geometry")
        .map(|item| item.value.clone())
        .collect::<Vec<_>>();
    let (exact_models, geometry_models) = tokio::try_join!(
        duplicate_models_for_values(&state.pool, "exact", &exact_values),
        duplicate_models_for_values(&state.pool, "geometry", &geometry_values)
    )?;
    let mut groups = Vec::with_capacity(limit as usize);
    for match_row in matches {
        let models = if match_row.match_kind == "exact" {
            exact_models
                .get(&match_row.value)
                .cloned()
                .unwrap_or_default()
        } else {
            geometry_models
                .get(&match_row.value)
                .cloned()
                .unwrap_or_default()
        };
        groups.push(DuplicateGroup {
            id: match_row.match_key.clone(),
            match_key: match_row.match_key,
            match_kind: match_row.match_kind,
            confidence: match_row.confidence,
            model_count: match_row.model_count,
            byte_size: match_row.byte_size,
            models,
        });
    }
    Ok(Page {
        items: groups,
        total,
        next_offset: (offset + limit < total).then_some(offset + limit),
        next_cursor: None,
    })
}

#[tauri::command]
pub async fn merge_projects(
    input: MergeProjectsInput,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    merge_projects_impl(&input.keeper_id, &input.project_ids, &state.pool).await
}

#[tauri::command]
pub async fn split_project(
    input: SplitProjectInput,
    state: State<'_, AppState>,
) -> CommandResult<String> {
    let storage_mode: String = sqlx::query_scalar("SELECT storage_mode FROM models WHERE id = ?")
        .bind(&input.project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Project not found".to_string())?;
    if storage_mode == "managed" {
        return Err("Use New Project with Move to split files from a managed project".into());
    }
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 160 {
        return Err("Project names must contain 1–160 characters".into());
    }
    if input.asset_ids.is_empty() {
        return Err("Choose at least one file to split into a project".into());
    }
    if input.asset_ids.len() > 1_000 {
        return Err("A project split cannot include more than 1,000 files".into());
    }
    if input.asset_ids.iter().collect::<HashSet<_>>().len() != input.asset_ids.len() {
        return Err("A project split cannot contain the same file more than once".into());
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_assets WHERE model_id = ?")
        .bind(&input.project_id)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    let selected: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_assets WHERE model_id = ? AND asset_id IN (SELECT value FROM json_each(?))")
        .bind(&input.project_id)
        .bind(serde_json::to_string(&input.asset_ids).map_err(|error| error.to_string())?)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    if selected != input.asset_ids.len() as i64 || selected >= total {
        return Err("A split must leave at least one file in the original project".into());
    }
    let folder_id: String = sqlx::query_scalar("SELECT a.folder_id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.id = ?")
        .bind(&input.project_id)
        .bind(&input.asset_ids[0])
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    let project_id = Uuid::new_v4().to_string();
    let timestamp = now();
    let mut transaction = state.pool.begin().await.map_err(db_error)?;
    sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, bundle_mode, added_at, updated_at) VALUES (?, ?, ?, ?, '', 'manual', ?, ?)")
        .bind(&project_id)
        .bind(&folder_id)
        .bind(name)
        .bind(format!("manual:{project_id}"))
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    for asset_id in &input.asset_ids {
        sqlx::query("UPDATE model_assets SET model_id = ? WHERE model_id = ? AND asset_id = ?")
            .bind(&project_id)
            .bind(&input.project_id)
            .bind(asset_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE project_bom_items SET linked_asset_id = NULL, updated_at = ? WHERE model_id = ? AND linked_asset_id IN (SELECT value FROM json_each(?))")
        .bind(&timestamp)
        .bind(&input.project_id)
        .bind(serde_json::to_string(&input.asset_ids).map_err(|error| error.to_string())?)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    set_best_primary(&project_id, &mut transaction).await?;
    set_best_primary(&input.project_id, &mut transaction).await?;
    sqlx::query("UPDATE models SET grouping_key = 'manual:' || id, bundle_mode = 'manual', updated_at = ? WHERE id = ?")
        .bind(&timestamp)
        .bind(&input.project_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    indexing::refresh_search(&project_id, &state.pool).await?;
    indexing::refresh_search(&input.project_id, &state.pool).await?;
    Ok(project_id)
}

#[tauri::command]
pub async fn set_project_primary_asset(
    project_id: String,
    asset_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let belongs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = ? AND ma.asset_id = ? AND a.missing_since IS NULL")
            .bind(&project_id)
            .bind(&asset_id)
            .fetch_one(&state.pool)
            .await
            .map_err(db_error)?;
    if belongs == 0 {
        return Err("The selected file is unavailable or is not part of this project".into());
    }
    sqlx::query("UPDATE models SET primary_asset_id = ?, grouping_key = 'manual:' || id, bundle_mode = 'manual', updated_at = ? WHERE id = ?")
        .bind(asset_id)
        .bind(now())
        .bind(project_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn dismiss_duplicate_match(
    match_key: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if !(match_key.starts_with("exact:") || match_key.starts_with("geometry:")) {
        return Err("Unknown duplicate match".into());
    }
    sqlx::query("INSERT OR IGNORE INTO duplicate_dismissals (match_key, created_at) VALUES (?, ?)")
        .bind(match_key)
        .bind(now())
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn cleanup_duplicate_group(
    input: DuplicateCleanupInput,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if input.duplicate_ids.is_empty() || input.duplicate_ids.iter().any(|id| id == &input.keeper_id)
    {
        return Err("Choose one keeper and at least one other project".into());
    }
    if input.duplicate_ids.len() > 100
        || input.duplicate_ids.iter().collect::<HashSet<_>>().len() != input.duplicate_ids.len()
    {
        return Err("Duplicate cleanup contains too many or repeated projects".into());
    }
    let (match_kind, match_value) = input
        .match_key
        .split_once(':')
        .ok_or_else(|| "Unknown duplicate match".to_string())?;
    let asset_sql = match match_kind {
        "exact" => "SELECT ma.asset_id FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id IN (SELECT value FROM json_each(?)) AND a.content_hash = ?",
        "geometry" => "SELECT ma.asset_id FROM model_assets ma JOIN asset_geometry g ON g.asset_id = ma.asset_id WHERE ma.model_id IN (SELECT value FROM json_each(?)) AND g.geometry_hash = ?",
        _ => return Err("Unknown duplicate match".into()),
    };
    let asset_ids: Vec<String> = sqlx::query_scalar(asset_sql)
        .bind(serde_json::to_string(&input.duplicate_ids).map_err(|error| error.to_string())?)
        .bind(match_value)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    if asset_ids.is_empty() {
        return Err("The selected projects no longer contain this duplicate".into());
    }
    let matched_projects: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT model_id) FROM model_assets WHERE model_id IN (SELECT value FROM json_each(?)) AND asset_id IN (SELECT value FROM json_each(?))")
        .bind(serde_json::to_string(&input.duplicate_ids).map_err(|error| error.to_string())?)
        .bind(serde_json::to_string(&asset_ids).map_err(|error| error.to_string())?)
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    if matched_projects != input.duplicate_ids.len() as i64 {
        return Err(
            "Every project selected for cleanup must belong to this duplicate group".into(),
        );
    }
    let keeper_matches: i64 = match match_kind {
        "exact" => sqlx::query_scalar("SELECT COUNT(*) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = ? AND a.content_hash = ?")
            .bind(&input.keeper_id).bind(match_value).fetch_one(&state.pool).await.map_err(db_error)?,
        "geometry" => sqlx::query_scalar("SELECT COUNT(*) FROM model_assets ma JOIN asset_geometry g ON g.asset_id = ma.asset_id WHERE ma.model_id = ? AND g.geometry_hash = ?")
            .bind(&input.keeper_id).bind(match_value).fetch_one(&state.pool).await.map_err(db_error)?,
        _ => 0,
    };
    if keeper_matches == 0 {
        return Err("The keeper project does not belong to this duplicate group".into());
    }
    if input.move_to_trash {
        let mut paths = Vec::with_capacity(asset_ids.len());
        for asset_id in &asset_ids {
            paths.push(asset_path(asset_id, &state).await?.0);
        }
        tokio::task::spawn_blocking(move || {
            for path in paths {
                trash::delete(&path).map_err(|error| {
                    format!("Unable to move {} to Trash: {error}", path.display())
                })?;
            }
            Ok::<_, String>(())
        })
        .await
        .map_err(|error| error.to_string())??;
    }
    merge_projects_impl(&input.keeper_id, &input.duplicate_ids, &state.pool).await?;
    if input.move_to_trash {
        let timestamp = now();
        for asset_id in asset_ids {
            sqlx::query(
                "UPDATE assets SET missing_since = COALESCE(missing_since, ?) WHERE id = ?",
            )
            .bind(&timestamp)
            .bind(asset_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        }
        let primary: Option<String> = sqlx::query_scalar("SELECT a.id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.missing_since IS NULL ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 WHEN 'step' THEN 3 WHEN 'stp' THEN 3 ELSE 4 END, a.filename LIMIT 1")
            .bind(&input.keeper_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
        sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
            .bind(primary)
            .bind(&input.keeper_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("INSERT OR IGNORE INTO duplicate_dismissals (match_key, created_at) VALUES (?, ?)")
        .bind(input.match_key)
        .bind(now())
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

pub(crate) async fn merge_projects_impl(
    keeper_id: &str,
    project_ids: &[String],
    pool: &sqlx::SqlitePool,
) -> CommandResult<()> {
    if project_ids.len() > 500 {
        return Err("A project bundle cannot include more than 500 projects".into());
    }
    let mut sources = project_ids
        .iter()
        .filter(|id| id.as_str() != keeper_id)
        .cloned()
        .collect::<Vec<_>>();
    sources.sort_unstable();
    sources.dedup();
    if sources.is_empty() {
        return Err("Choose at least one project to bundle".into());
    }
    let keeper_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
        .bind(keeper_id)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    if keeper_exists == 0 {
        return Err("Keeper project not found".into());
    }
    let managed_with_files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models m WHERE m.id IN (SELECT value FROM json_each(?)) AND m.storage_mode = 'managed' AND EXISTS (SELECT 1 FROM model_assets WHERE model_id = m.id)")
        .bind(serde_json::to_string(&sources).map_err(|error| error.to_string())?)
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    let keeper_managed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ? AND storage_mode = 'managed'")
            .bind(keeper_id)
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
    let source_files: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM model_assets WHERE model_id IN (SELECT value FROM json_each(?))",
    )
    .bind(serde_json::to_string(&sources).map_err(|error| error.to_string())?)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if managed_with_files > 0 || (keeper_managed > 0 && source_files > 0) {
        return Err("Managed projects must be combined with New Project and Move so every file is consolidated safely".into());
    }
    let timestamp = now();
    let mut transaction = pool.begin().await.map_err(db_error)?;
    for source_id in &sources {
        let row = sqlx::query("SELECT notes, favorite FROM models WHERE id = ?")
            .bind(source_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(db_error)?
            .ok_or_else(|| "Project to bundle was not found".to_string())?;
        let notes: String = row.get("notes");
        if !notes.trim().is_empty() {
            sqlx::query("UPDATE models SET notes = CASE WHEN trim(notes) = '' THEN ? ELSE notes || '\n\n' || ? END WHERE id = ?")
                .bind(&notes)
                .bind(&notes)
                .bind(keeper_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        if row.get::<i64, _>("favorite") != 0 {
            sqlx::query("UPDATE models SET favorite = 1 WHERE id = ?")
                .bind(keeper_id)
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        sqlx::query("INSERT OR IGNORE INTO model_tags (model_id, tag_id, added_at) SELECT ?, tag_id, added_at FROM model_tags WHERE model_id = ?")
            .bind(keeper_id).bind(source_id).execute(&mut *transaction).await.map_err(db_error)?;
        sqlx::query("INSERT OR IGNORE INTO collection_items (collection_id, model_id, position, added_at) SELECT collection_id, ?, position, added_at FROM collection_items WHERE model_id = ?")
            .bind(keeper_id).bind(source_id).execute(&mut *transaction).await.map_err(db_error)?;
        sqlx::query("UPDATE web_sources SET model_id = ?, updated_at = ? WHERE model_id = ?")
            .bind(keeper_id)
            .bind(&timestamp)
            .bind(source_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        sqlx::query("UPDATE cost_estimates SET model_id = ?, updated_at = ? WHERE model_id = ?")
            .bind(keeper_id)
            .bind(&timestamp)
            .bind(source_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        project_docs::transfer_documentation(source_id, keeper_id, &mut transaction).await?;
        sqlx::query("UPDATE model_assets SET model_id = ? WHERE model_id = ?")
            .bind(keeper_id)
            .bind(source_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(source_id)
            .execute(&mut *transaction)
            .await
            .map_err(db_error)?;
    }
    sqlx::query("UPDATE models SET grouping_key = 'manual:' || id, bundle_mode = 'manual', missing_since = NULL, updated_at = ? WHERE id = ?")
        .bind(&timestamp).bind(keeper_id).execute(&mut *transaction).await.map_err(db_error)?;
    set_best_primary(keeper_id, &mut transaction).await?;
    transaction.commit().await.map_err(db_error)?;
    indexing::refresh_search(keeper_id, pool).await?;
    Ok(())
}

async fn set_best_primary(
    project_id: &str,
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> CommandResult<()> {
    let primary: Option<String> = sqlx::query_scalar("SELECT a.id FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? AND a.missing_since IS NULL ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 WHEN 'step' THEN 3 WHEN 'stp' THEN 3 ELSE 4 END, ma.sort_order, a.filename LIMIT 1")
        .bind(project_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE models SET primary_asset_id = ? WHERE id = ?")
        .bind(primary)
        .bind(project_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn duplicate_matches_sql() -> &'static str {
    "SELECT 'exact' match_kind, a.content_hash match_value, MAX(a.byte_size) byte_size, COUNT(DISTINCT ma.model_id) model_count, MAX(a.modified_at) latest_modified FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE a.missing_since IS NULL AND a.content_hash IS NOT NULL AND NOT EXISTS (SELECT 1 FROM duplicate_dismissals d WHERE d.match_key = 'exact:' || a.content_hash) GROUP BY a.content_hash HAVING COUNT(DISTINCT ma.model_id) > 1 UNION ALL SELECT 'geometry' match_kind, g.geometry_hash match_value, MAX(a.byte_size) byte_size, COUNT(DISTINCT ma.model_id) model_count, MAX(a.modified_at) latest_modified FROM asset_geometry g JOIN assets a ON a.id = g.asset_id JOIN model_assets ma ON ma.asset_id = a.id WHERE a.missing_since IS NULL AND NOT EXISTS (SELECT 1 FROM duplicate_dismissals d WHERE d.match_key = 'geometry:' || g.geometry_hash) GROUP BY g.geometry_hash HAVING COUNT(DISTINCT ma.model_id) > 1 AND COUNT(DISTINCT COALESCE(a.content_hash, a.id)) > 1"
}

async fn duplicate_match_rows_page(
    pool: &sqlx::SqlitePool,
    offset: i64,
    limit: i64,
) -> CommandResult<(Vec<DuplicateMatchRow>, i64)> {
    let sql = format!(
        "WITH matches AS ({}) SELECT match_kind, match_value, byte_size, model_count, latest_modified, COUNT(*) OVER() total_count FROM matches ORDER BY latest_modified DESC, match_kind, match_value LIMIT ? OFFSET ?",
        duplicate_matches_sql()
    );
    let rows = sqlx::query(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let total = rows
        .first()
        .map(|row| row.get::<i64, _>("total_count"))
        .unwrap_or_default();
    let matches = rows
        .into_iter()
        .map(|row| {
            let value: String = row.get("match_value");
            let match_kind: String = row.get("match_kind");
            DuplicateMatchRow {
                match_key: format!("{match_kind}:{value}"),
                value,
                confidence: if match_kind == "exact" { 1.0 } else { 0.96 },
                match_kind,
                byte_size: row.get("byte_size"),
                model_count: row.get("model_count"),
            }
        })
        .collect();
    Ok((matches, total))
}

async fn duplicate_models_for_values(
    pool: &sqlx::SqlitePool,
    kind: &str,
    values: &[String],
) -> CommandResult<HashMap<String, Vec<ModelSummary>>> {
    if values.is_empty() {
        return Ok(HashMap::new());
    }
    let (value_column, geometry_join) = if kind == "exact" {
        ("matching_asset.content_hash", "")
    } else {
        (
            "matching_geometry.geometry_hash",
            " JOIN asset_geometry matching_geometry ON matching_geometry.asset_id = matching_asset.id",
        )
    };
    let sql = format!("SELECT {value_column} match_value, m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(m.asset_modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, m.bundle_mode, m.storage_mode, (SELECT pp.id FROM project_photos pp WHERE pp.model_id = m.id AND pp.is_cover = 1 LIMIT 1) cover_photo_id, m.asset_count, COALESCE(pa.metadata_json, '{{}}') metadata_json FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id JOIN model_assets matching_ma ON matching_ma.model_id = m.id JOIN assets matching_asset ON matching_asset.id = matching_ma.asset_id{geometry_join} WHERE {value_column} IN (SELECT value FROM json_each(?)) AND matching_asset.missing_since IS NULL GROUP BY match_value, m.id ORDER BY match_value, lower(m.display_name), f.relative_path");
    let rows = sqlx::query(&sql)
        .bind(serde_json::to_string(values).map_err(|error| error.to_string())?)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    let mut grouped = HashMap::<String, Vec<ModelSummary>>::new();
    for row in rows {
        let value: String = row.get("match_value");
        grouped
            .entry(value)
            .or_default()
            .push(summary_from_row(row)?);
    }
    Ok(grouped)
}

#[tauri::command]
pub async fn list_materials(state: State<'_, AppState>) -> CommandResult<Vec<Material>> {
    list_materials_impl(state.inner()).await
}

pub(crate) async fn list_materials_impl(state: &AppState) -> CommandResult<Vec<Material>> {
    let rows = sqlx::query("SELECT id, name, material_type, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density, updated_at FROM materials ORDER BY lower(name)").fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows.into_iter().map(material_from_row).collect())
}

#[tauri::command]
pub async fn save_material(
    input: MaterialInput,
    state: State<'_, AppState>,
) -> CommandResult<Material> {
    save_material_impl(input, &state).await
}

pub(crate) async fn save_material_impl(
    input: MaterialInput,
    state: &AppState,
) -> CommandResult<Material> {
    if input.name.trim().is_empty() || input.spool_weight_g <= 0.0 || input.spool_price_minor < 0 {
        return Err("Enter a name, positive spool weight, and valid price".into());
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    sqlx::query("INSERT INTO materials (id, name, material_type, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, material_type = excluded.material_type, color_name = excluded.color_name, color_hex = excluded.color_hex, spool_price_minor = excluded.spool_price_minor, currency = excluded.currency, spool_weight_g = excluded.spool_weight_g, density = excluded.density, updated_at = excluded.updated_at")
        .bind(&id).bind(&input.name).bind(&input.material_type).bind(&input.color_name).bind(&input.color_hex).bind(input.spool_price_minor).bind(&input.currency).bind(input.spool_weight_g).bind(input.density).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.map_err(db_error)?;
    Ok(Material {
        id,
        name: input.name,
        material_type: input.material_type,
        color_name: input.color_name,
        color_hex: input.color_hex,
        spool_price_minor: input.spool_price_minor,
        currency: input.currency,
        spool_weight_g: input.spool_weight_g,
        density: input.density,
        updated_at: timestamp,
    })
}

#[tauri::command]
pub fn calculate_cost(input: CostCalculationInput) -> CommandResult<CostCalculation> {
    if input.plastic_g < 0.0
        || input.quantity < 1
        || input.spool_price_minor < 0.0
        || input.spool_weight_g <= 0.0
    {
        return Err("Cost inputs must be positive".into());
    }
    let cost_per_piece_minor = input.plastic_g / input.spool_weight_g * input.spool_price_minor;
    Ok(CostCalculation {
        cost_per_piece_minor,
        batch_cost_minor: cost_per_piece_minor * input.quantity as f64,
    })
}

#[tauri::command]
pub async fn save_cost_estimate(
    input: CostEstimateInput,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    save_cost_estimate_impl(input, state.inner()).await
}

pub(crate) async fn save_cost_estimate_impl(
    input: CostEstimateInput,
    state: &AppState,
) -> CommandResult<()> {
    if input.plastic_g < 0.0 || input.quantity < 1 {
        return Err("Plastic usage and quantity must be positive".into());
    }
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM cost_estimates WHERE model_id = ? ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(&input.model_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?;
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    let fingerprint: Option<String> = if let Some(asset_id) = &input.asset_id {
        sqlx::query_scalar("SELECT partial_fingerprint FROM assets WHERE id = ?")
            .bind(asset_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .flatten()
    } else {
        None
    };
    sqlx::query("INSERT INTO cost_estimates (id, model_id, asset_id, material_id, plastic_g, quantity, source, source_fingerprint, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET asset_id = excluded.asset_id, material_id = excluded.material_id, plastic_g = excluded.plastic_g, quantity = excluded.quantity, source = excluded.source, source_fingerprint = excluded.source_fingerprint, updated_at = excluded.updated_at")
        .bind(id).bind(input.model_id).bind(input.asset_id).bind(input.material_id).bind(input.plastic_g).bind(input.quantity).bind(input.source).bind(fingerprint).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn get_preview_payload(
    asset_id: String,
    state: State<'_, AppState>,
) -> CommandResult<PreviewPayload> {
    let (path, extension) = asset_path(&asset_id, &state).await?;
    tokio::task::spawn_blocking(move || parsers::preview_payload(&path, &extension, asset_id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_3mf_plate_thumbnail(
    asset_id: String,
    plate_index: u32,
    state: State<'_, AppState>,
) -> CommandResult<tauri::ipc::Response> {
    let bytes = plate_thumbnail_bytes(&asset_id, plate_index, state.inner()).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub(crate) async fn plate_thumbnail_bytes(
    asset_id: &str,
    plate_index: u32,
    state: &AppState,
) -> CommandResult<Vec<u8>> {
    let (path, extension) = asset_path(asset_id, state).await?;
    if extension != "3mf" {
        return Err("Plate thumbnails are only available for 3MF files".into());
    }
    tokio::task::spawn_blocking(move || parsers::embedded_3mf_plate_thumbnail(&path, plate_index))
        .await
        .map_err(|error| error.to_string())??
        .ok_or_else(|| "Plate thumbnail is not available".to_string())
}

#[tauri::command]
pub async fn get_viewer_mesh(
    asset_id: String,
    plate_index: Option<u32>,
    state: State<'_, AppState>,
) -> CommandResult<tauri::ipc::Response> {
    let bytes = viewer_mesh_bytes(&asset_id, plate_index, state.inner()).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

pub(crate) async fn viewer_mesh_bytes(
    asset_id: &str,
    plate_index: Option<u32>,
    state: &AppState,
) -> CommandResult<Vec<u8>> {
    let started = std::time::Instant::now();
    log::info!("Viewer mesh requested: asset={asset_id} plate={plate_index:?}");
    let (path, extension) = asset_path(asset_id, state).await?;
    log::info!("Viewer mesh asset resolved: asset={asset_id}");
    let fingerprint = current_thumbnail_revision(asset_id, &state.pool).await?;
    let plate_key = plate_index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "all".into());
    if let Some(row) = sqlx::query("SELECT id, cache_path FROM viewer_meshes WHERE asset_id = ? AND fingerprint = ? AND plate_key = ? AND renderer_version = 'viewer-2'")
        .bind(asset_id)
        .bind(&fingerprint)
        .bind(&plate_key)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
    {
        if let Ok(bytes) = tokio::fs::read(PathBuf::from(row.get::<String, _>("cache_path"))).await {
            let _ = sqlx::query("UPDATE viewer_meshes SET last_accessed_at = ? WHERE id = ? AND julianday(last_accessed_at) < julianday('now', '-1 hour')")
                .bind(now())
                .bind(row.get::<String, _>("id"))
                .execute(&state.pool)
                .await;
            log::info!(
                "Viewer mesh cache hit: asset={asset_id} bytes={} elapsed_ms={}",
                bytes.len(),
                started.elapsed().as_millis()
            );
            return Ok(bytes);
        }
    }
    log::info!("Viewer mesh cache miss: asset={asset_id}");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("volum-viewer-mesh".into())
        .spawn(move || {
            let _ = sender.send(previews::viewer_mesh(&path, &extension, plate_index));
        })
        .map_err(|error| format!("Could not start the viewer mesh worker: {error}"))?;
    let bytes = receiver
        .await
        .map_err(|_| "The viewer mesh worker stopped unexpectedly".to_string())??;
    log::info!(
        "Viewer mesh generated: asset={asset_id} bytes={} elapsed_ms={}",
        bytes.len(),
        started.elapsed().as_millis()
    );
    persist_viewer_mesh(
        asset_id,
        &fingerprint,
        &plate_key,
        &bytes,
        &state.pool,
        &state.data_dir,
    )
    .await?;
    log::info!(
        "Viewer mesh cached: asset={asset_id} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(bytes)
}

#[tauri::command]
pub async fn get_cached_thumbnail(
    asset_id: String,
    state: State<'_, AppState>,
) -> CommandResult<tauri::ipc::Response> {
    let bytes = read_cached_thumbnail(&asset_id, &state.pool)
        .await?
        .unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ThumbnailError {
    asset_id: String,
    message: String,
}

#[tauri::command]
pub async fn request_thumbnail(
    asset_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<bool> {
    if read_cached_thumbnail(&asset_id, &state.pool)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    let (path, extension) = asset_path(&asset_id, &state).await?;
    if !matches!(extension.as_str(), "stl" | "obj" | "3mf" | "step" | "stp") {
        return Ok(false);
    }
    {
        let mut pending = state.thumbnail_pending.lock().await;
        if !pending.insert(asset_id.clone()) {
            return Ok(false);
        }
        if pending.len() > 256 {
            pending.remove(&asset_id);
            return Err(
                "The preview queue is busy; this preview can be requested again when visible"
                    .into(),
            );
        }
    }
    let cancellation = tokio_util::sync::CancellationToken::new();
    state
        .thumbnail_cancellations
        .lock()
        .await
        .insert(asset_id.clone(), cancellation.clone());
    let pool = state.pool.clone();
    let data_dir = state.data_dir.clone();
    let workers = state.thumbnail_workers.clone();
    let pending = state.thumbnail_pending.clone();
    let cancellations = state.thumbnail_cancellations.clone();
    let queued_asset_id = asset_id.clone();
    tauri::async_runtime::spawn(async move {
        let result: CommandResult<bool> = async {
            let _worker = tokio::select! {
                worker = workers.acquire_owned() => worker.map_err(|_| "Thumbnail worker queue is unavailable".to_string())?,
                _ = cancellation.cancelled() => return Ok(false),
            };
            if cancellation.is_cancelled() {
                return Ok(false);
            }
            if read_cached_thumbnail(&queued_asset_id, &pool)
                .await?
                .is_some()
            {
                return Ok(true);
            }
            let revision = current_thumbnail_revision(&queued_asset_id, &pool).await?;
            let render_path = path.clone();
            let render_extension = extension.clone();
            let bytes = tokio::task::spawn_blocking(move || -> CommandResult<Vec<u8>> {
                if render_extension == "3mf" {
                    if let Some(bytes) = parsers::embedded_3mf_thumbnail(&render_path)? {
                        return Ok(bytes);
                    }
                }
                previews::render_thumbnail(&render_path, &render_extension)
            })
            .await
            .map_err(|error| error.to_string())??;
            if cancellation.is_cancelled() {
                return Ok(false);
            }
            if current_thumbnail_revision(&queued_asset_id, &pool)
                .await
                .ok()
                .as_deref()
                == Some(revision.as_str())
            {
                persist_thumbnail(&queued_asset_id, &revision, &bytes, &pool, &data_dir).await?;
            }
            Ok(true)
        }
        .await;
        pending.lock().await.remove(&queued_asset_id);
        cancellations.lock().await.remove(&queued_asset_id);
        match result {
            Ok(true) => {
                let _ = app.emit("preview-ready", &queued_asset_id);
            }
            Ok(false) => {}
            Err(message) => {
                log::warn!("Thumbnail generation failed for {queued_asset_id}: {message}");
                let _ = app.emit(
                    "preview-error",
                    ThumbnailError {
                        asset_id: queued_asset_id,
                        message,
                    },
                );
            }
        }
    });
    Ok(true)
}

#[tauri::command]
pub async fn cancel_thumbnail_request(
    asset_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if let Some(cancellation) = state.thumbnail_cancellations.lock().await.get(&asset_id) {
        cancellation.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> CommandResult<HashMap<String, String>> {
    let rows = sqlx::query("SELECT key, value_json FROM user_overrides WHERE entity_type = 'app' AND entity_id = 'global'").fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            serde_json::from_str::<String>(&row.get::<String, _>("value_json"))
                .ok()
                .map(|value| (row.get::<String, _>("key"), value))
        })
        .collect())
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguredSlicer {
    id: String,
    name: String,
    path: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlicerApplication {
    id: String,
    name: String,
    path: Option<String>,
    installed: bool,
    custom: bool,
    brand_color: String,
    icon_png: Option<Vec<u8>>,
}

struct KnownSlicer {
    id: &'static str,
    name: &'static str,
    brand_color: &'static str,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    aliases: &'static [&'static str],
    executables: &'static [&'static str],
}

#[tauri::command]
pub async fn list_slicer_apps(
    custom_apps: Option<Vec<ConfiguredSlicer>>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<SlicerApplication>> {
    let data_dir = state.data_dir.clone();
    tokio::task::spawn_blocking(move || {
        discover_slicer_apps(custom_apps.unwrap_or_default(), &data_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_preference(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    const ALLOWED: &[&str] = &[
        "slicer_path",
        "slicer_name",
        "slicer_config",
        "web_import_folder",
        "ui_theme",
        "ui_language",
        "ui_density",
        "ui_model_sort",
    ];
    if !ALLOWED.contains(&key.as_str()) {
        return Err("Unknown preference".into());
    }
    if value.len() > 32 * 1024 {
        return Err("Preference value is too long".into());
    }
    if key == "slicer_path" && !value.is_empty() && !Path::new(&value).exists() {
        return Err("The selected application is unavailable".into());
    }
    let value = if key == "web_import_folder" && !value.is_empty() {
        let folder = std::fs::canonicalize(&value)
            .map_err(|_| "The selected import folder is unavailable".to_string())?;
        if !folder.is_dir() {
            return Err("Choose a folder for web imports".into());
        }
        let roots: Vec<String> = sqlx::query_scalar(
            "SELECT path FROM library_roots WHERE status != 'removed' ORDER BY created_at",
        )
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
        let inside_library = roots
            .iter()
            .any(|root| std::fs::canonicalize(root).is_ok_and(|root| folder.starts_with(root)));
        if !inside_library {
            return Err("Choose a folder inside one of your Volum libraries".into());
        }
        folder.to_string_lossy().into_owned()
    } else {
        value
    };
    sqlx::query("INSERT INTO user_overrides (id, entity_type, entity_id, key, value_json, updated_at) VALUES (?, 'app', 'global', ?, ?, ?) ON CONFLICT(entity_type, entity_id, key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(Uuid::new_v4().to_string()).bind(key).bind(serde_json::to_string(&value).map_err(|error| error.to_string())?).bind(now()).execute(&state.pool).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn open_asset(
    asset_id: String,
    app_path: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let (path, _) = asset_path(&asset_id, &state).await?;
    if let Some(application) = app_path {
        open_with(&path, &application)?;
    } else {
        open::that(&path).map_err(|error| error.to_string())?;
    }
    sqlx::query("UPDATE models SET last_opened_at = ? WHERE id IN (SELECT model_id FROM model_assets WHERE asset_id = ?)").bind(now()).bind(asset_id).execute(&state.pool).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> CommandResult<()> {
    if url.len() > 2048
        || !url.starts_with("https://")
        || url.chars().any(char::is_whitespace)
        || url[8..].split('/').next().unwrap_or_default().is_empty()
    {
        return Err("Only valid HTTPS links can be opened".into());
    }
    open::that(url).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reveal_asset(asset_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    let (path, _) = asset_path(&asset_id, &state).await?;
    reveal(&path)
}

#[tauri::command]
pub async fn export_metadata(path: String, state: State<'_, AppState>) -> CommandResult<()> {
    let destination = PathBuf::from(path);
    if destination.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("Metadata exports must use a .json filename".into());
    }
    let collections = list_collections(state.clone()).await?;
    let materials = list_materials(state.clone()).await?;
    let web_sources = web_sources::list_web_sources(state.clone()).await?;
    let notes = sqlx::query("SELECT id, notes, favorite FROM models WHERE notes != '' OR favorite = 1").fetch_all(&state.pool).await.map_err(db_error)?.into_iter().map(|row| json!({ "modelId": row.get::<String, _>("id"), "notes": row.get::<String, _>("notes"), "favorite": row.get::<i64, _>("favorite") != 0 })).collect::<Vec<_>>();
    let document = json!({ "format": "volum-metadata", "version": 2, "exportedAt": now(), "collections": collections, "materials": materials, "webSources": web_sources, "modelOverrides": notes });
    let temporary = destination.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, destination).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_diagnostics(
    path: String,
    redact_paths: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let destination = PathBuf::from(path);
    if destination.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("Diagnostics exports must use a .json filename".into());
    }
    let roots = sqlx::query("SELECT id, path, display_name, status, last_scan_at FROM library_roots ORDER BY created_at")
        .fetch_all(&state.pool).await.map_err(db_error)?
        .into_iter().map(|row| {
            let path: String = row.get("path");
            json!({
                "id": row.get::<String, _>("id"),
                "path": if redact_paths { format!("<redacted:{}>", &blake3::hash(path.as_bytes()).to_hex()[..10]) } else { path },
                "displayName": row.get::<String, _>("display_name"),
                "status": row.get::<String, _>("status"),
                "lastScanAt": row.get::<Option<String>, _>("last_scan_at")
            })
        }).collect::<Vec<_>>();
    let format_counts = sqlx::query("SELECT extension, parse_status, COUNT(*) count FROM assets GROUP BY extension, parse_status ORDER BY extension")
        .fetch_all(&state.pool).await.map_err(db_error)?
        .into_iter().map(|row| json!({ "format": row.get::<String, _>("extension"), "status": row.get::<String, _>("parse_status"), "count": row.get::<i64, _>("count") })).collect::<Vec<_>>();
    let document = json!({ "format": "volum-diagnostics", "version": 1, "appVersion": env!("CARGO_PKG_VERSION"), "exportedAt": now(), "pathsRedacted": redact_paths, "roots": roots, "assetSummary": format_counts });
    write_json_atomic(&destination, &document)
}

async fn get_estimate(model_id: &str, state: &AppState) -> CommandResult<Option<CostEstimate>> {
    let row = sqlx::query("SELECT e.id, e.model_id, e.asset_id, e.material_id, e.plastic_g, e.quantity, e.source, m.spool_price_minor, m.spool_weight_g FROM cost_estimates e LEFT JOIN materials m ON m.id = e.material_id WHERE e.model_id = ? ORDER BY e.updated_at DESC LIMIT 1").bind(model_id).fetch_optional(&state.pool).await.map_err(db_error)?;
    Ok(row.map(|row| {
        let each = match (
            row.try_get::<i64, _>("spool_price_minor").ok(),
            row.try_get::<f64, _>("spool_weight_g").ok(),
        ) {
            (Some(price), Some(weight)) if weight > 0.0 => {
                Some(row.get::<f64, _>("plastic_g") / weight * price as f64)
            }
            _ => None,
        };
        CostEstimate {
            id: row.get("id"),
            model_id: row.get("model_id"),
            asset_id: row.get("asset_id"),
            material_id: row.get("material_id"),
            plastic_g: row.get("plastic_g"),
            quantity: row.get("quantity"),
            source: row.get("source"),
            cost_per_piece_minor: each,
            batch_cost_minor: each.map(|value| value * row.get::<i64, _>("quantity") as f64),
        }
    }))
}

pub(crate) async fn read_cached_thumbnail(
    asset_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<Option<Vec<u8>>> {
    let row = sqlx::query("SELECT t.id, t.cache_path, t.fingerprint, a.partial_fingerprint, a.modified_at FROM thumbnails t JOIN assets a ON a.id = t.asset_id WHERE t.asset_id = ? AND t.renderer_version = 'cpu-4' AND t.width = 480 AND t.height = 360 ORDER BY t.created_at DESC LIMIT 1")
        .bind(asset_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let current = cache_revision(
        &row.get::<Option<String>, _>("partial_fingerprint")
            .unwrap_or_default(),
        &row.get::<String, _>("modified_at"),
    );
    if row.get::<String, _>("fingerprint") != current {
        return Ok(None);
    }
    let path = PathBuf::from(row.get::<String, _>("cache_path"));
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let _ = sqlx::query("UPDATE thumbnails SET created_at = ? WHERE id = ? AND julianday(created_at) < julianday('now', '-1 hour')")
                .bind(now())
                .bind(row.get::<String, _>("id"))
                .execute(pool)
                .await;
            Ok(Some(bytes))
        }
        Err(_) => Ok(None),
    }
}

pub(crate) async fn thumbnail_bytes_impl(
    asset_id: &str,
    state: &AppState,
) -> CommandResult<Option<Vec<u8>>> {
    if let Some(bytes) = read_cached_thumbnail(asset_id, &state.pool).await? {
        return Ok(Some(bytes));
    }
    let (path, extension) = asset_path(asset_id, state).await?;
    if !matches!(extension.as_str(), "stl" | "obj" | "3mf" | "step" | "stp") {
        return Ok(None);
    }
    let _worker = state
        .thumbnail_workers
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Thumbnail worker queue is unavailable".to_string())?;
    if let Some(bytes) = read_cached_thumbnail(asset_id, &state.pool).await? {
        return Ok(Some(bytes));
    }
    let revision = current_thumbnail_revision(asset_id, &state.pool).await?;
    let bytes = tokio::task::spawn_blocking(move || -> CommandResult<Vec<u8>> {
        if extension == "3mf" {
            if let Some(bytes) = parsers::embedded_3mf_thumbnail(&path)? {
                return Ok(bytes);
            }
        }
        previews::render_thumbnail(&path, &extension)
    })
    .await
    .map_err(|error| error.to_string())??;
    if current_thumbnail_revision(asset_id, &state.pool)
        .await
        .ok()
        .as_deref()
        == Some(revision.as_str())
    {
        persist_thumbnail(asset_id, &revision, &bytes, &state.pool, &state.data_dir).await?;
    }
    Ok(Some(bytes))
}

async fn current_thumbnail_revision(
    asset_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<String> {
    let row = sqlx::query("SELECT partial_fingerprint, modified_at FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Asset fingerprint is unavailable".to_string())?;
    Ok(cache_revision(
        &row.get::<Option<String>, _>("partial_fingerprint")
            .unwrap_or_default(),
        &row.get::<String, _>("modified_at"),
    ))
}

fn cache_revision(partial_fingerprint: &str, modified_at: &str) -> String {
    blake3::hash(format!("{partial_fingerprint}|{modified_at}").as_bytes())
        .to_hex()
        .to_string()
}

async fn persist_thumbnail(
    asset_id: &str,
    fingerprint: &str,
    bytes: &[u8],
    pool: &sqlx::SqlitePool,
    data_dir: &Path,
) -> CommandResult<()> {
    if bytes.len() > 2 * 1024 * 1024 || !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return Err("Generated thumbnail is not a valid PNG".into());
    }
    let stale = sqlx::query("SELECT id, cache_path FROM thumbnails WHERE asset_id = ? AND renderer_version LIKE 'cpu-%' AND (renderer_version != 'cpu-4' OR fingerprint != ?)")
        .bind(asset_id)
        .bind(fingerprint)
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    for row in stale {
        let _ = tokio::fs::remove_file(PathBuf::from(row.get::<String, _>("cache_path"))).await;
        sqlx::query("DELETE FROM thumbnails WHERE id = ?")
            .bind(row.get::<String, _>("id"))
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    let directory = data_dir.join("thumbnails");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| error.to_string())?;
    let path = directory.join(format!("{asset_id}-{fingerprint}-cpu-4.png"));
    let temporary = directory.join(format!("{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("INSERT INTO thumbnails (id, asset_id, fingerprint, renderer_version, preset, width, height, cache_path, byte_size, created_at) VALUES (?, ?, ?, 'cpu-4', 'studio', 480, 360, ?, ?, ?) ON CONFLICT(asset_id, fingerprint, renderer_version, preset, width, height) DO UPDATE SET cache_path = excluded.cache_path, byte_size = excluded.byte_size, created_at = excluded.created_at")
        .bind(Uuid::new_v4().to_string())
        .bind(asset_id)
        .bind(fingerprint)
        .bind(path.to_string_lossy().to_string())
        .bind(bytes.len() as i64)
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn persist_viewer_mesh(
    asset_id: &str,
    fingerprint: &str,
    plate_key: &str,
    bytes: &[u8],
    pool: &sqlx::SqlitePool,
    data_dir: &Path,
) -> CommandResult<()> {
    let directory = data_dir.join("viewer-meshes");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| error.to_string())?;
    let cache_name =
        blake3::hash(format!("{asset_id}|{fingerprint}|{plate_key}|viewer-2").as_bytes())
            .to_hex()
            .to_string();
    let path = directory.join(format!("{cache_name}.vlm"));
    let temporary = directory.join(format!("{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|error| error.to_string())?;
    let timestamp = now();
    sqlx::query("INSERT INTO viewer_meshes (id, asset_id, fingerprint, plate_key, renderer_version, cache_path, byte_size, last_accessed_at, created_at) VALUES (?, ?, ?, ?, 'viewer-2', ?, ?, ?, ?) ON CONFLICT(asset_id, fingerprint, plate_key, renderer_version) DO UPDATE SET cache_path = excluded.cache_path, byte_size = excluded.byte_size, last_accessed_at = excluded.last_accessed_at")
        .bind(Uuid::new_v4().to_string())
        .bind(asset_id)
        .bind(fingerprint)
        .bind(plate_key)
        .bind(path.to_string_lossy().to_string())
        .bind(bytes.len() as i64)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(pool)
        .await
        .map_err(db_error)?;
    cleanup_viewer_mesh_cache(pool, 1024 * 1024 * 1024).await
}

async fn cleanup_viewer_mesh_cache(pool: &sqlx::SqlitePool, max_bytes: i64) -> CommandResult<()> {
    let total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(byte_size), 0) FROM viewer_meshes")
        .fetch_one(pool)
        .await
        .map_err(db_error)?;
    if total <= max_bytes {
        return Ok(());
    }
    let rows = sqlx::query(
        "SELECT id, cache_path, byte_size FROM viewer_meshes ORDER BY last_accessed_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let mut retained = 0_i64;
    for row in rows {
        let size = row.get::<i64, _>("byte_size").max(0);
        if retained.saturating_add(size) <= max_bytes {
            retained += size;
            continue;
        }
        let _ = tokio::fs::remove_file(PathBuf::from(row.get::<String, _>("cache_path"))).await;
        sqlx::query("DELETE FROM viewer_meshes WHERE id = ?")
            .bind(row.get::<String, _>("id"))
            .execute(pool)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

pub(crate) async fn asset_path(
    asset_id: &str,
    state: &AppState,
) -> CommandResult<(PathBuf, String)> {
    let row = sqlx::query("SELECT r.path root_path, a.relative_path, a.extension FROM assets a JOIN library_roots r ON r.id = a.root_id WHERE a.id = ?").bind(asset_id).fetch_optional(&state.pool).await.map_err(db_error)?.ok_or_else(|| "Asset not found".to_string())?;
    let root = PathBuf::from(row.get::<String, _>("root_path"));
    let path = root.join(row.get::<String, _>("relative_path"));
    let canonical_root = root
        .canonicalize()
        .map_err(|_| "Library is offline".to_string())?;
    let canonical_path = path
        .canonicalize()
        .map_err(|_| "The model file is unavailable".to_string())?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err("Asset path is outside its approved library".into());
    }
    Ok((canonical_path, row.get("extension")))
}

fn summary_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ModelSummary, String> {
    let metadata: AssetMetadata =
        serde_json::from_str(&row.get::<String, _>("metadata_json")).unwrap_or_default();
    Ok(ModelSummary {
        id: row.get("id"),
        display_name: row.get("display_name"),
        folder_id: row.get("folder_id"),
        folder_name: row.get("folder_name"),
        relative_folder: row.get("relative_path"),
        primary_asset_id: row.get("primary_asset_id"),
        primary_extension: row.get("primary_extension"),
        favorite: row.get::<i64, _>("favorite") != 0,
        added_at: row.get("added_at"),
        modified_at: row.get("modified_at"),
        last_opened_at: row.get("last_opened_at"),
        missing: row.get::<Option<String>, _>("missing_since").is_some(),
        asset_count: row.get("asset_count"),
        bundle_mode: row
            .try_get("bundle_mode")
            .unwrap_or_else(|_| "automatic".to_string()),
        storage_mode: row
            .try_get("storage_mode")
            .unwrap_or_else(|_| "linked".to_string()),
        cover_photo_id: row.try_get("cover_photo_id").unwrap_or(None),
        dimensions_mm: metadata.dimensions_mm,
    })
}

fn material_from_row(row: sqlx::sqlite::SqliteRow) -> Material {
    Material {
        id: row.get("id"),
        name: row.get("name"),
        material_type: row.get("material_type"),
        color_name: row.get("color_name"),
        color_hex: row.get("color_hex"),
        spool_price_minor: row.get("spool_price_minor"),
        currency: row.get("currency"),
        spool_weight_g: row.get("spool_weight_g"),
        density: row.get("density"),
        updated_at: row.get("updated_at"),
    }
}
fn fts_query(input: &str) -> String {
    input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"*", token.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn model_sort_name(sort: Option<&str>) -> &'static str {
    match sort {
        Some("name") => "name",
        Some("added") => "added",
        Some("opened") => "opened",
        _ => "modified",
    }
}

fn model_cursor_condition(sort: Option<&str>, cursor: &ModelPageCursor) -> CommandResult<String> {
    let sort = model_sort_name(sort);
    if cursor.sort != sort {
        return Err("Model page cursor does not match the selected sort".into());
    }
    let primary = sql_string(&cursor.primary);
    let secondary = sql_string(&cursor.secondary);
    let name = sql_string(&cursor.name);
    let id = sql_string(&cursor.id);
    let name_tail = format!("((m.display_name COLLATE NOCASE) > ({name} COLLATE NOCASE) OR ((m.display_name COLLATE NOCASE) = ({name} COLLATE NOCASE) AND m.id > {id}))");
    Ok(match sort {
        "name" => name_tail,
        "added" => format!("m.added_at < {primary} OR (m.added_at = {primary} AND {name_tail})"),
        "opened" => {
            let opened = "COALESCE(m.last_opened_at, '')";
            let modified = "COALESCE(m.asset_modified_at, m.updated_at)";
            format!("{opened} < {primary} OR ({opened} = {primary} AND ({modified} < {secondary} OR ({modified} = {secondary} AND {name_tail})))")
        }
        _ => {
            let modified = "COALESCE(m.asset_modified_at, m.updated_at)";
            format!("{modified} < {primary} OR ({modified} = {primary} AND {name_tail})")
        }
    })
}

fn encode_model_cursor(sort: Option<&str>, item: &ModelSummary) -> CommandResult<String> {
    let sort = model_sort_name(sort);
    let primary = match sort {
        "name" => item.display_name.to_lowercase(),
        "added" => item.added_at.clone(),
        "opened" => item.last_opened_at.clone().unwrap_or_default(),
        _ => item.modified_at.clone(),
    };
    serde_json::to_string(&ModelPageCursor {
        sort: sort.into(),
        primary,
        secondary: if sort == "opened" {
            item.modified_at.clone()
        } else {
            String::new()
        },
        name: item.display_name.to_lowercase(),
        id: item.id.clone(),
    })
    .map_err(|error| error.to_string())
}

fn model_query_sql(query: &ModelQuery) -> CommandResult<String> {
    let mut conditions = Vec::new();
    if let Some(library_id) = query
        .library_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        conditions.push(format!("f.root_id = {}", sql_string(library_id)));
    }
    if let Some(status) = query
        .parse_status
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        if !["pending", "ready", "warning", "error"].contains(&status) {
            return Err("Unknown parsing status".into());
        }
        conditions.push(format!("EXISTS (SELECT 1 FROM model_assets qma JOIN assets qa ON qa.id = qma.asset_id WHERE qma.model_id = m.id AND qa.parse_status = {})", sql_string(status)));
    }
    if let Some(expected) = query.has_web_source {
        let exists = "EXISTS (SELECT 1 FROM web_sources qw WHERE qw.model_id = m.id)";
        conditions.push(if expected {
            exists.into()
        } else {
            format!("NOT ({exists})")
        });
    }
    if let Some(minimum) = query.min_asset_count {
        if minimum < 0 {
            return Err("Minimum file count cannot be negative".into());
        }
        conditions.push(format!("m.asset_count >= {minimum}"));
    }
    if let Some(maximum) = query.max_asset_count {
        if maximum < 0 {
            return Err("Maximum file count cannot be negative".into());
        }
        conditions.push(format!("m.asset_count <= {maximum}"));
    }
    if query.duplicates.unwrap_or(false) || query.duplicate_kind.is_some() {
        let kind = query.duplicate_kind.as_deref().unwrap_or("any");
        conditions.push(duplicate_rule_sql(kind)?);
    }
    Ok(if conditions.is_empty() {
        "1 = 1".into()
    } else {
        conditions.join(" AND ")
    })
}

fn validate_collection(input: &CollectionInput) -> CommandResult<()> {
    if input.name.trim().is_empty() || input.name.chars().count() > 80 {
        Err("Collection names must contain 1–80 characters".into())
    } else if !valid_color(&input.color) {
        Err("Collection color must be a hex color".into())
    } else if input.smart {
        let Some(rule) = &input.rule else {
            return Err("Smart collections need at least one rule".into());
        };
        if rule.tag_id.as_deref().unwrap_or("").is_empty()
            && rule.format.as_deref().unwrap_or("").is_empty()
            && rule.availability.as_deref().unwrap_or("").is_empty()
            && rule.rules.is_empty()
        {
            return Err("Smart collections need at least one rule".into());
        }
        if rule
            .format
            .as_deref()
            .is_some_and(|format| !["stl", "3mf", "obj", "step", "stp"].contains(&format))
        {
            return Err("Unknown smart collection format".into());
        }
        if rule
            .availability
            .as_deref()
            .is_some_and(|value| !["available", "offline"].contains(&value))
        {
            return Err("Unknown smart collection availability".into());
        }
        smart_rule_sql(rule).map(|_| ())
    } else {
        Ok(())
    }
}

fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn tag_from_row(row: sqlx::sqlite::SqliteRow) -> Tag {
    Tag {
        id: row.get("id"),
        name: row.get("name"),
        color: row.get("color"),
        model_count: row.get("model_count"),
    }
}

async fn ensure_manual_collection(
    collection_id: &str,
    pool: &sqlx::SqlitePool,
) -> CommandResult<()> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM collections WHERE id = ?")
        .bind(collection_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    match kind.as_deref() {
        Some("manual") => Ok(()),
        Some(_) => {
            Err("Smart collections update automatically and cannot be edited manually".into())
        }
        None => Err("Collection not found".into()),
    }
}

async fn count_smart_collection(
    rule: &SmartCollectionRule,
    pool: &sqlx::SqlitePool,
) -> CommandResult<i64> {
    sqlx::query_scalar(&smart_collection_count_sql(rule)?)
        .fetch_one(pool)
        .await
        .map_err(db_error)
}

fn smart_collection_count_sql(rule: &SmartCollectionRule) -> CommandResult<String> {
    let format = rule
        .format
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace("stp", "step");
    let availability = rule.availability.clone().unwrap_or_default();
    let tag = rule.tag_id.clone().unwrap_or_default();
    let rich = smart_rule_sql(rule)?;
    let format = sql_string(&format);
    let availability = sql_string(&availability);
    let tag = sql_string(&tag);
    Ok(format!("SELECT COUNT(*) FROM models m LEFT JOIN assets pa ON pa.id = m.primary_asset_id WHERE ({format} = '' OR EXISTS (SELECT 1 FROM model_assets fma JOIN assets fa ON fa.id = fma.asset_id WHERE fma.model_id = m.id AND CASE fa.extension WHEN 'stp' THEN 'step' ELSE fa.extension END = {format})) AND ({availability} = '' OR ({availability} = 'available' AND m.missing_since IS NULL) OR ({availability} = 'offline' AND m.missing_since IS NOT NULL)) AND ({tag} = '' OR EXISTS (SELECT 1 FROM model_tags mt WHERE mt.model_id = m.id AND mt.tag_id = {tag})) AND ({rich})"))
}

fn smart_rule_sql(rule: &SmartCollectionRule) -> CommandResult<String> {
    if ![1, 2].contains(&rule.version) {
        return Err("Unsupported smart collection rule version".into());
    }
    if rule.version < 2 && rule.rules.iter().any(|item| item.field == "custom") {
        return Err("Custom-field rules require smart collection rule version 2".into());
    }
    if rule.rules.is_empty() {
        return Ok("1 = 1".into());
    }
    let mut conditions = Vec::with_capacity(rule.rules.len());
    for item in &rule.rules {
        let string_value = item.value.as_str().unwrap_or_default();
        let quoted = sql_string(string_value);
        let condition = match item.field.as_str() {
            "text" => {
                let query = fts_query(string_value);
                if query.is_empty() {
                    return Err("Text rules need a searchable value".into());
                }
                let condition = format!("m.id IN (SELECT model_id FROM model_search WHERE model_search MATCH {})", sql_string(&query));
                match item.operator.as_str() {
                    "is" => condition,
                    "isNot" => format!("NOT ({condition})"),
                    _ => return Err("Unknown text rule operator".into()),
                }
            }
            "folder" => match item.operator.as_str() {
                "is" => format!("m.folder_id = {quoted}"),
                "isNot" => format!("m.folder_id != {quoted}"),
                _ => return Err("Unknown folder rule operator".into()),
            },
            "library" => match item.operator.as_str() {
                "is" => format!("EXISTS (SELECT 1 FROM folders rf WHERE rf.id = m.folder_id AND rf.root_id = {quoted})"),
                "isNot" => format!("NOT EXISTS (SELECT 1 FROM folders rf WHERE rf.id = m.folder_id AND rf.root_id = {quoted})"),
                _ => return Err("Unknown library rule operator".into()),
            },
            "tag" => match item.operator.as_str() {
                "is" => format!("EXISTS (SELECT 1 FROM model_tags rt WHERE rt.model_id = m.id AND rt.tag_id = {quoted})"),
                "isNot" => format!("NOT EXISTS (SELECT 1 FROM model_tags rt WHERE rt.model_id = m.id AND rt.tag_id = {quoted})"),
                _ => return Err("Unknown tag rule operator".into()),
            },
            "format" => {
                if !["stl", "3mf", "obj", "step", "stp"].contains(&string_value) {
                    return Err("Unknown smart collection format".into());
                }
                let quoted = sql_string(if string_value == "stp" { "step" } else { string_value });
                match item.operator.as_str() {
                    "is" => format!("EXISTS (SELECT 1 FROM model_assets rma JOIN assets ra ON ra.id = rma.asset_id WHERE rma.model_id = m.id AND CASE ra.extension WHEN 'stp' THEN 'step' ELSE ra.extension END = {quoted})"),
                    "isNot" => format!("NOT EXISTS (SELECT 1 FROM model_assets rma JOIN assets ra ON ra.id = rma.asset_id WHERE rma.model_id = m.id AND CASE ra.extension WHEN 'stp' THEN 'step' ELSE ra.extension END = {quoted})"),
                    _ => return Err("Unknown format rule operator".into()),
                }
            }
            "availability" => {
                let condition = match string_value {
                    "available" => "m.missing_since IS NULL",
                    "offline" => "m.missing_since IS NOT NULL",
                    _ => return Err("Unknown availability rule".into()),
                };
                match item.operator.as_str() {
                    "is" => condition.into(),
                    "isNot" => format!("NOT ({condition})"),
                    _ => return Err("Unknown availability rule operator".into()),
                }
            }
            "favorite" => boolean_condition("m.favorite = 1", &item.value, &item.operator)?,
            "webSource" => boolean_condition("EXISTS (SELECT 1 FROM web_sources rw WHERE rw.model_id = m.id)", &item.value, &item.operator)?,
            "parseStatus" => match item.operator.as_str() {
                "is" => format!("EXISTS (SELECT 1 FROM model_assets rma JOIN assets ra ON ra.id = rma.asset_id WHERE rma.model_id = m.id AND ra.parse_status = {quoted})"),
                "isNot" => format!("NOT EXISTS (SELECT 1 FROM model_assets rma JOIN assets ra ON ra.id = rma.asset_id WHERE rma.model_id = m.id AND ra.parse_status = {quoted})"),
                _ => return Err("Unknown parsing-status rule operator".into()),
            },
            "duplicate" => {
                let condition = duplicate_rule_sql(string_value)?;
                match item.operator.as_str() {
                    "is" => condition,
                    "isNot" => format!("NOT ({condition})"),
                    _ => return Err("Unknown duplicate rule operator".into()),
                }
            }
            "added" => date_rule_sql("m.added_at", &item.operator, string_value)?,
            "modified" => date_rule_sql("COALESCE(m.asset_modified_at, m.updated_at)", &item.operator, string_value)?,
            "opened" => date_rule_sql("m.last_opened_at", &item.operator, string_value)?,
            "fileCount" => numeric_rule_sql("m.asset_count", &item.operator, &item.value)?,
            "width" => numeric_rule_sql("CAST(json_extract(pa.metadata_json, '$.dimensionsMm[0]') AS REAL)", &item.operator, &item.value)?,
            "depth" => numeric_rule_sql("CAST(json_extract(pa.metadata_json, '$.dimensionsMm[1]') AS REAL)", &item.operator, &item.value)?,
            "height" => numeric_rule_sql("CAST(json_extract(pa.metadata_json, '$.dimensionsMm[2]') AS REAL)", &item.operator, &item.value)?,
            "custom" => custom_field_rule_sql(item)?,
            _ => return Err(format!("Unknown smart collection field: {}", item.field)),
        };
        conditions.push(format!("({condition})"));
    }
    let joiner = if rule.match_mode == "any" {
        " OR "
    } else if rule.match_mode == "all" {
        " AND "
    } else {
        return Err("Smart collection match mode must be all or any".into());
    };
    Ok(conditions.join(joiner))
}

fn custom_field_rule_sql(rule: &QueryRule) -> CommandResult<String> {
    let field_id = rule
        .custom_field_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Custom-field rules need a field".to_string())?;
    let field = sql_string(field_id);
    let value_sql = "json_extract(rcv.value_json, '$')";
    let comparison = if let Some(value) = rule.value.as_f64() {
        if !value.is_finite() {
            return Err("Custom number rules need a finite number".into());
        }
        let operator = match rule.operator.as_str() {
            "is" => "=",
            "greaterThan" => ">",
            "lessThan" => "<",
            "atLeast" => ">=",
            "atMost" => "<=",
            _ => return Err("Unknown custom number rule operator".into()),
        };
        format!("CAST({value_sql} AS REAL) {operator} {value}")
    } else if let Some(value) = rule.value.as_bool() {
        if rule.operator != "is" && rule.operator != "isNot" {
            return Err("Unknown custom checkbox rule operator".into());
        }
        format!("CAST({value_sql} AS INTEGER) = {}", i64::from(value))
    } else {
        let value = rule
            .value
            .as_str()
            .ok_or_else(|| "Custom-field rules need a scalar value".to_string())?;
        let quoted = sql_string(value);
        match rule.operator.as_str() {
            "is" | "isNot" => format!("CAST({value_sql} AS TEXT) = {quoted}"),
            "before" => {
                NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| "Custom date rules need a valid calendar date".to_string())?;
                format!("date(CAST({value_sql} AS TEXT)) < date({quoted})")
            }
            "after" => {
                NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| "Custom date rules need a valid calendar date".to_string())?;
                format!("date(CAST({value_sql} AS TEXT)) > date({quoted})")
            }
            "on" => {
                NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| "Custom date rules need a valid calendar date".to_string())?;
                format!("date(CAST({value_sql} AS TEXT)) = date({quoted})")
            }
            _ => return Err("Unknown custom text rule operator".into()),
        }
    };
    let exists = format!("EXISTS (SELECT 1 FROM project_custom_values rcv WHERE rcv.model_id = m.id AND rcv.field_id = {field} AND {comparison})");
    Ok(if rule.operator == "isNot" {
        format!("NOT ({exists})")
    } else {
        exists
    })
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn boolean_condition(
    sql: &str,
    value: &serde_json::Value,
    operator: &str,
) -> CommandResult<String> {
    let expected = value
        .as_bool()
        .ok_or_else(|| "Boolean rules need true or false".to_string())?;
    match operator {
        "is" => Ok(if expected {
            sql.into()
        } else {
            format!("NOT ({sql})")
        }),
        "isNot" => Ok(if expected {
            format!("NOT ({sql})")
        } else {
            sql.into()
        }),
        _ => Err("Unknown boolean rule operator".into()),
    }
}

fn date_rule_sql(column: &str, operator: &str, value: &str) -> CommandResult<String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "Date rules need a valid calendar date".to_string())?;
    let comparison = match operator {
        "before" => "<",
        "after" => ">",
        "on" | "is" => "=",
        _ => return Err("Unknown date rule operator".into()),
    };
    Ok(format!(
        "date({column}) {comparison} date({})",
        sql_string(value)
    ))
}

fn numeric_rule_sql(
    column: &str,
    operator: &str,
    value: &serde_json::Value,
) -> CommandResult<String> {
    let number = value
        .as_f64()
        .ok_or_else(|| "Numeric rules need a number".to_string())?;
    if !number.is_finite() {
        return Err("Numeric rule values must be finite".into());
    }
    let comparison = match operator {
        "is" => "=",
        "greaterThan" => ">",
        "lessThan" => "<",
        "atLeast" => ">=",
        "atMost" => "<=",
        _ => return Err("Unknown numeric rule operator".into()),
    };
    Ok(format!("{column} {comparison} {number}"))
}

fn duplicate_rule_sql(value: &str) -> CommandResult<String> {
    let exact = "EXISTS (SELECT 1 FROM model_assets rma JOIN assets ra ON ra.id = rma.asset_id WHERE rma.model_id = m.id AND ra.content_hash IS NOT NULL AND NOT EXISTS (SELECT 1 FROM duplicate_dismissals rd WHERE rd.match_key = 'exact:' || ra.content_hash) AND EXISTS (SELECT 1 FROM model_assets oma JOIN assets oa ON oa.id = oma.asset_id WHERE oma.model_id != m.id AND oa.content_hash = ra.content_hash))";
    let geometry = "EXISTS (SELECT 1 FROM model_assets rma JOIN asset_geometry rg ON rg.asset_id = rma.asset_id WHERE rma.model_id = m.id AND NOT EXISTS (SELECT 1 FROM duplicate_dismissals rd WHERE rd.match_key = 'geometry:' || rg.geometry_hash) AND EXISTS (SELECT 1 FROM model_assets oma JOIN asset_geometry og ON og.asset_id = oma.asset_id WHERE oma.model_id != m.id AND og.geometry_hash = rg.geometry_hash))";
    match value {
        "exact" => Ok(exact.into()),
        "geometry" => Ok(geometry.into()),
        "any" => Ok(format!("({exact} OR {geometry})")),
        "none" => Ok(format!("NOT ({exact} OR {geometry})")),
        _ => Err("Unknown duplicate rule".into()),
    }
}

fn discover_slicer_apps(
    custom_apps: Vec<ConfiguredSlicer>,
    data_dir: &Path,
) -> CommandResult<Vec<SlicerApplication>> {
    let catalog = [
        KnownSlicer {
            id: "bambu-studio",
            name: "Bambu Studio",
            brand_color: "#00ae42",
            aliases: &["BambuStudio", "Bambu Studio"],
            executables: &["bambu-studio", "BambuStudio.exe"],
        },
        KnownSlicer {
            id: "orca-slicer",
            name: "OrcaSlicer",
            brand_color: "#1f9ad6",
            aliases: &["OrcaSlicer", "Orca Slicer"],
            executables: &["orca-slicer", "orca-slicer.exe", "OrcaSlicer.exe"],
        },
        KnownSlicer {
            id: "prusa-slicer",
            name: "PrusaSlicer",
            brand_color: "#f58220",
            aliases: &["PrusaSlicer", "Prusa Slicer"],
            executables: &["prusa-slicer", "prusa-slicer.exe"],
        },
        KnownSlicer {
            id: "ultimaker-cura",
            name: "UltiMaker Cura",
            brand_color: "#00a1e0",
            aliases: &["UltiMaker Cura", "Ultimaker Cura", "Cura"],
            executables: &["cura", "UltiMaker-Cura.exe", "Cura.exe"],
        },
        KnownSlicer {
            id: "creality-print",
            name: "Creality Print",
            brand_color: "#ff6a00",
            aliases: &["Creality Print", "CrealityPrint"],
            executables: &["creality-print", "CrealityPrint.exe"],
        },
        KnownSlicer {
            id: "lychee-slicer",
            name: "Lychee Slicer",
            brand_color: "#7d4dff",
            aliases: &["LycheeSlicer", "Lychee Slicer"],
            executables: &["LycheeSlicer", "LycheeSlicer.exe"],
        },
        KnownSlicer {
            id: "ideamaker",
            name: "ideaMaker",
            brand_color: "#e64b38",
            aliases: &["ideaMaker", "Ideamaker"],
            executables: &["ideamaker", "ideaMaker.exe"],
        },
        KnownSlicer {
            id: "superslicer",
            name: "SuperSlicer",
            brand_color: "#d44636",
            aliases: &["SuperSlicer", "Super Slicer"],
            executables: &["superslicer", "superslicer.exe"],
        },
        KnownSlicer {
            id: "anycubic-slicer-next",
            name: "Anycubic Slicer Next",
            brand_color: "#315df5",
            aliases: &["Anycubic Slicer Next", "AnycubicSlicerNext"],
            executables: &["anycubic-slicer-next", "AnycubicSlicerNext.exe"],
        },
    ];
    let icon_dir = data_dir.join("slicer-icons");
    let mut applications = Vec::with_capacity(catalog.len() + custom_apps.len());
    for known in catalog {
        let path = discover_known_slicer(&known);
        let icon_png = path
            .as_deref()
            .and_then(|path| application_icon(path, &icon_dir));
        applications.push(SlicerApplication {
            id: known.id.into(),
            name: known.name.into(),
            installed: path.is_some(),
            path: path.map(|path| path.to_string_lossy().into_owned()),
            custom: false,
            brand_color: known.brand_color.into(),
            icon_png,
        });
    }
    for custom in custom_apps {
        let path = PathBuf::from(&custom.path);
        let installed = path.exists();
        applications.push(SlicerApplication {
            id: custom.id,
            name: custom.name,
            path: installed.then(|| path.to_string_lossy().into_owned()),
            installed,
            custom: true,
            brand_color: "#6f6d67".into(),
            icon_png: installed
                .then(|| application_icon(&path, &icon_dir))
                .flatten(),
        });
    }
    Ok(applications)
}

#[cfg(target_os = "macos")]
fn discover_known_slicer(slicer: &KnownSlicer) -> Option<PathBuf> {
    let _ = slicer.executables;
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    for root in &roots {
        for alias in slicer.aliases {
            let candidate = root.join(format!("{alias}.app"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    let aliases: Vec<String> = slicer
        .aliases
        .iter()
        .map(|name| normalize_application_name(name))
        .collect();
    roots.into_iter().find_map(|root| {
        std::fs::read_dir(root).ok()?.flatten().find_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("app") {
                return None;
            }
            let name = normalize_application_name(
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
            );
            aliases
                .iter()
                .any(|alias| name == *alias || name.starts_with(alias))
                .then_some(path)
        })
    })
}

#[cfg(target_os = "linux")]
fn discover_known_slicer(slicer: &KnownSlicer) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|directory| {
        slicer
            .executables
            .iter()
            .map(|name| directory.join(name))
            .find(|candidate| candidate.exists())
    })
}

#[cfg(target_os = "windows")]
fn discover_known_slicer(slicer: &KnownSlicer) -> Option<PathBuf> {
    let roots = [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("LOCALAPPDATA"),
    ];
    roots.into_iter().flatten().find_map(|root| {
        walkdir::WalkDir::new(root)
            .max_depth(4)
            .into_iter()
            .filter_map(Result::ok)
            .map(|entry| entry.into_path())
            .find(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| {
                        slicer
                            .executables
                            .iter()
                            .any(|executable| name.eq_ignore_ascii_case(executable))
                    })
            })
    })
}

#[cfg(target_os = "macos")]
fn application_icon(application: &Path, cache_dir: &Path) -> Option<Vec<u8>> {
    let info = application.join("Contents/Info.plist");
    let resources = application.join("Contents/Resources");
    let configured = std::process::Command::new("plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&info)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());
    let source = configured
        .map(|name| {
            let name = if name.to_ascii_lowercase().ends_with(".icns") {
                name
            } else {
                format!("{name}.icns")
            };
            resources.join(name)
        })
        .filter(|path| path.exists())
        .or_else(|| {
            std::fs::read_dir(&resources)
                .ok()?
                .flatten()
                .find_map(|entry| {
                    let path = entry.path();
                    path.extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("icns"))
                        .then_some(path)
                })
        })?;
    let _ = std::fs::create_dir_all(cache_dir);
    let key = blake3::hash(application.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    let cached = cache_dir.join(format!("{key}.png"));
    if !cached.exists() {
        let temporary = cached.with_extension("png.tmp");
        let success = std::process::Command::new("sips")
            .args(["-s", "format", "png", "-Z", "128"])
            .arg(source)
            .arg("--out")
            .arg(&temporary)
            .output()
            .ok()
            .is_some_and(|output| output.status.success());
        if !success {
            return None;
        }
        if std::fs::rename(&temporary, &cached).is_err() {
            let _ = std::fs::remove_file(temporary);
        }
    }
    std::fs::read(cached).ok()
}

#[cfg(not(target_os = "macos"))]
fn application_icon(_application: &Path, _cache_dir: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "macos")]
fn normalize_application_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn open_with(path: &Path, application: &str) -> CommandResult<()> {
    let app = Path::new(application);
    if !app.exists() {
        return Err("Configured application is unavailable".into());
    }
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open")
        .arg("-a")
        .arg(app)
        .arg(path)
        .status();
    #[cfg(target_os = "windows")]
    return std::process::Command::new(app)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string());
    #[cfg(target_os = "linux")]
    let status = std::process::Command::new(app).arg(path).status();
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    status
        .map_err(|error| error.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("Application failed to open the file".into())
            }
        })
}

pub(crate) fn reveal(path: &Path) -> CommandResult<()> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
    result.map(|_| ()).map_err(|error| error.to_string())
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn write_json_atomic(destination: &Path, document: &serde_json::Value) -> CommandResult<()> {
    let temporary = destination.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, destination).map_err(|error| error.to_string())
}
fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_cost_without_early_rounding() {
        let result = calculate_cost(CostCalculationInput {
            plastic_g: 84.0,
            quantity: 10,
            spool_price_minor: 1599.0,
            spool_weight_g: 1000.0,
        })
        .unwrap();
        assert!((result.cost_per_piece_minor - 134.316).abs() < 0.0001);
        assert!((result.batch_cost_minor - 1343.16).abs() < 0.0001);
    }

    #[test]
    fn fts_query_ignores_punctuation() {
        assert_eq!(
            fts_query("tool-wall (bracket)"),
            "\"tool\"* AND \"wall\"* AND \"bracket\"*"
        );
    }

    #[test]
    fn thumbnail_revision_changes_with_source_timestamp() {
        assert_ne!(
            cache_revision("same-content-sample", "2026-09-20T10:00:00Z"),
            cache_revision("same-content-sample", "2026-09-20T10:00:01Z")
        );
    }

    #[test]
    fn smart_collections_require_a_valid_rule() {
        let empty = CollectionInput {
            name: "Automatic".into(),
            symbol: "sparkles".into(),
            color: "#ff5a36".into(),
            smart: true,
            rule: Some(SmartCollectionRule::default()),
        };
        assert!(validate_collection(&empty).is_err());
        let tagged = CollectionInput {
            rule: Some(SmartCollectionRule {
                tag_id: Some("tag-1".into()),
                ..Default::default()
            }),
            ..empty
        };
        assert!(validate_collection(&tagged).is_ok());
    }

    #[test]
    fn rich_smart_rules_compile_only_known_fields() {
        let rule = SmartCollectionRule {
            version: 1,
            match_mode: "any".into(),
            rules: vec![
                QueryRule {
                    field: "format".into(),
                    operator: "is".into(),
                    value: serde_json::json!("step"),
                    custom_field_id: None,
                },
                QueryRule {
                    field: "fileCount".into(),
                    operator: "atLeast".into(),
                    value: serde_json::json!(3),
                    custom_field_id: None,
                },
            ],
            ..Default::default()
        };
        let sql = smart_rule_sql(&rule).unwrap();
        assert!(sql.contains(" OR "));
        assert!(sql.contains("ELSE ra.extension END = 'step'"));
        assert!(sql.contains("m.asset_count"));

        let unknown = SmartCollectionRule {
            rules: vec![QueryRule {
                field: "rawSql".into(),
                operator: "is".into(),
                value: serde_json::json!("1 = 1"),
                custom_field_id: None,
            }],
            ..Default::default()
        };
        assert!(smart_rule_sql(&unknown).is_err());

        let unsupported = SmartCollectionRule {
            version: 3,
            rules: vec![QueryRule {
                field: "favorite".into(),
                operator: "is".into(),
                value: serde_json::json!(true),
                custom_field_id: None,
            }],
            ..Default::default()
        };
        assert!(smart_rule_sql(&unsupported).is_err());

        let custom = SmartCollectionRule {
            version: 2,
            rules: vec![QueryRule {
                field: "custom".into(),
                operator: "is".into(),
                value: serde_json::json!("Complete"),
                custom_field_id: Some("field-status".into()),
            }],
            ..Default::default()
        };
        let custom_sql = smart_rule_sql(&custom).unwrap();
        assert!(custom_sql.contains("project_custom_values"));
        assert!(custom_sql.contains("field-status"));
    }

    #[test]
    fn saved_search_project_filters_are_validated() {
        let sql = model_query_sql(&ModelQuery {
            library_id: Some("library-id".into()),
            parse_status: Some("ready".into()),
            has_web_source: Some(true),
            min_asset_count: Some(2),
            duplicate_kind: Some("geometry".into()),
            ..Default::default()
        })
        .unwrap();
        assert!(sql.contains("f.root_id"));
        assert!(sql.contains("qa.parse_status"));
        assert!(sql.contains("geometry_hash"));
        assert!(model_query_sql(&ModelQuery {
            parse_status: Some("raw-sql".into()),
            ..Default::default()
        })
        .is_err());
    }

    #[tokio::test]
    async fn merging_projects_preserves_files_and_organization() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path()).await.unwrap();
        let timestamp = now();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, created_at, updated_at) VALUES ('root', ?, 'Root', 'online', ?, ?)")
            .bind(directory.path().to_string_lossy().to_string()).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO folders (id, root_id, relative_path, name) VALUES ('folder', 'root', '', 'Root')")
            .execute(&state.pool).await.unwrap();
        for (id, name, favorite) in [("keeper", "Project", 0), ("source", "Project v2", 1)] {
            sqlx::query("INSERT INTO models (id, folder_id, display_name, grouping_key, version_key, favorite, added_at, updated_at) VALUES (?, 'folder', ?, ?, 'project', ?, ?, ?)")
                .bind(id).bind(name).bind(id).bind(favorite).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
            let asset_id = format!("asset-{id}");
            sqlx::query("INSERT INTO assets (id, root_id, folder_id, relative_path, filename, extension, byte_size, modified_at) VALUES (?, 'root', 'folder', ?, ?, 'stl', 1, ?)")
                .bind(&asset_id).bind(format!("{id}.stl")).bind(format!("{id}.stl")).bind(&timestamp).execute(&state.pool).await.unwrap();
            sqlx::query(
                "INSERT INTO model_assets (model_id, asset_id, role) VALUES (?, ?, 'printable')",
            )
            .bind(id)
            .bind(&asset_id)
            .execute(&state.pool)
            .await
            .unwrap();
        }
        sqlx::query("INSERT INTO tags (id, name, color, created_at, updated_at) VALUES ('tag', 'Useful', '#ff5a36', ?, ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO model_tags (model_id, tag_id, added_at) VALUES ('source', 'tag', ?)",
        )
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO custom_field_definitions (id, name, field_type, options_json, created_at, updated_at) VALUES ('field', 'Finish', 'text', '[]', ?, ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO project_custom_values (model_id, field_id, value_json, updated_at) VALUES ('keeper', 'field', '\"matte\"', ?), ('source', 'field', '\"glossy\"', ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO project_photos (id, model_id, media_path, thumbnail_path, original_filename, mime_type, byte_size, width, height, caption, is_cover, created_at, updated_at) VALUES ('photo', 'source', 'project-photos/source/photo.jpg', 'project-photos/source/photo.thumb.png', 'photo.jpg', 'image/jpeg', 1, 1, 1, 'Finished', 1, ?, ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO project_bom_items (id, model_id, kind, linked_asset_id, name, quantity, unit, created_at, updated_at) VALUES ('bom', 'source', 'printed', 'asset-source', 'Printed body', 1, 'pcs', ?, ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query("INSERT INTO assembly_steps (id, model_id, title, instructions, created_at, updated_at) VALUES ('step', 'source', 'Attach body', 'Press fit', ?, ?)")
            .bind(&timestamp).bind(&timestamp).execute(&state.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO assembly_step_bom_items (step_id, bom_item_id) VALUES ('step', 'bom')",
        )
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO assembly_step_photos (step_id, photo_id) VALUES ('step', 'photo')",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        merge_projects_impl("keeper", &["source".into()], &state.pool)
            .await
            .unwrap();

        let asset_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM model_assets WHERE model_id = 'keeper'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let source_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = 'source'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let row = sqlx::query("SELECT favorite, bundle_mode FROM models WHERE id = 'keeper'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let tag_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM model_tags WHERE model_id = 'keeper' AND tag_id = 'tag'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        let custom_value_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_custom_values WHERE model_id = 'keeper'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        let photo_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM project_photos WHERE model_id = 'keeper'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let bom_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_bom_items WHERE model_id = 'keeper' AND linked_asset_id = 'asset-source'")
            .fetch_one(&state.pool).await.unwrap();
        let step_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM assembly_steps WHERE model_id = 'keeper'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(asset_count, 2);
        assert_eq!(source_count, 0);
        assert_eq!(row.get::<i64, _>("favorite"), 1);
        assert_eq!(row.get::<String, _>("bundle_mode"), "manual");
        assert_eq!(tag_count, 1);
        assert_eq!(custom_value_count, 2);
        assert_eq!(photo_count, 1);
        assert_eq!(bom_count, 1);
        assert_eq!(step_count, 1);
    }
}
