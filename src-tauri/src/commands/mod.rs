use crate::{domain::*, indexing, parsers, previews, state::AppState, web_sources};
use chrono::Utc;
use serde_json::json;
use sqlx::Row;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;

#[tauri::command]
pub async fn list_roots(state: State<'_, AppState>) -> CommandResult<Vec<LibraryRoot>> {
    let rows = sqlx::query("SELECT r.id, r.path, r.display_name, r.status, r.last_scan_at, COUNT(DISTINCT CASE WHEN m.missing_since IS NULL THEN m.id END) model_count FROM library_roots r LEFT JOIN folders f ON f.root_id = r.id LEFT JOIN models m ON m.folder_id = f.id GROUP BY r.id ORDER BY r.created_at").fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| LibraryRoot {
            id: row.get("id"),
            path: row.get("path"),
            display_name: row.get("display_name"),
            status: row.get("status"),
            last_scan_at: row.get("last_scan_at"),
            model_count: row.get("model_count"),
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
    if let Some(row) = sqlx::query(
        "SELECT id, path, display_name, status, last_scan_at FROM library_roots WHERE path = ?",
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
    })
}

#[tauri::command]
pub async fn remove_library_root(
    root_id: String,
    keep_metadata: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
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
    let row = sqlx::query("SELECT id, path, display_name, status, last_scan_at, (SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id WHERE f.root_id = library_roots.id AND m.missing_since IS NULL) model_count FROM library_roots WHERE id = ?")
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
    })
}

#[tauri::command]
pub async fn start_scan(
    root_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    indexing::start(root_id, &state, app).await
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
    let rows = sqlx::query("SELECT f.id, f.root_id, f.parent_id, f.relative_path, f.name, (SELECT COUNT(DISTINCT descendant_model.id) FROM folders descendant JOIN models descendant_model ON descendant_model.folder_id = descendant.id WHERE descendant.root_id = f.root_id AND descendant_model.missing_since IS NULL AND (descendant.relative_path = f.relative_path OR substr(descendant.relative_path, 1, length(f.relative_path) + 1) = f.relative_path || '/')) model_count FROM folders f WHERE (? IS NULL OR f.root_id = ?) AND f.relative_path != '' ORDER BY lower(f.relative_path)").bind(&root_id).bind(&root_id).fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| Folder {
            id: row.get("id"),
            root_id: row.get("root_id"),
            parent_id: row.get("parent_id"),
            relative_path: row.get("relative_path"),
            name: row.get("name"),
            model_count: row.get("model_count"),
        })
        .collect())
}

#[tauri::command]
pub async fn list_models(
    query: ModelQuery,
    state: State<'_, AppState>,
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
        .to_ascii_lowercase();
    let availability = query.availability.clone().unwrap_or_default();
    let tag = query.tag_id.clone().unwrap_or_default();
    let smart_tag = smart_rule.tag_id.unwrap_or_default();
    let smart_format = smart_rule.format.unwrap_or_default().to_ascii_lowercase();
    let smart_availability = smart_rule.availability.unwrap_or_default();
    let date_from = query.date_from.clone().unwrap_or_default();
    let date_to = query.date_to.clone().unwrap_or_default();
    let date_column = if query.date_field.as_deref() == Some("added") {
        "m.added_at"
    } else {
        "COALESCE(pa.modified_at, m.updated_at)"
    };
    let favorite = i64::from(query.favorite.unwrap_or(false));
    let recent = i64::from(query.recent.unwrap_or(false));
    let duplicates = i64::from(query.duplicates.unwrap_or(false));
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let where_sql = format!(" WHERE (? = '' OR m.id IN (SELECT model_id FROM model_search WHERE model_search MATCH ?)) AND (? = '' OR m.folder_id IN (SELECT descendant.id FROM folders descendant JOIN folders selected ON selected.id = ? WHERE descendant.root_id = selected.root_id AND (descendant.relative_path = selected.relative_path OR substr(descendant.relative_path, 1, length(selected.relative_path) + 1) = selected.relative_path || '/'))) AND (? = '' OR EXISTS (SELECT 1 FROM collection_items ci WHERE ci.model_id = m.id AND ci.collection_id = ?)) AND (? = 0 OR m.favorite = 1) AND (? = 0 OR m.last_opened_at IS NOT NULL) AND (? = '' OR pa.extension = ?) AND (? = '' OR (? = 'available' AND m.missing_since IS NULL) OR (? = 'offline' AND m.missing_since IS NOT NULL)) AND (? = '' OR EXISTS (SELECT 1 FROM model_tags mt WHERE mt.model_id = m.id AND mt.tag_id = ?)) AND (? = '' OR pa.extension = ?) AND (? = '' OR (? = 'available' AND m.missing_since IS NULL) OR (? = 'offline' AND m.missing_since IS NOT NULL)) AND (? = '' OR EXISTS (SELECT 1 FROM model_tags smt WHERE smt.model_id = m.id AND smt.tag_id = ?)) AND (? = '' OR date({date_column}) >= date(?)) AND (? = '' OR date({date_column}) <= date(?)) AND (? = 0 OR EXISTS (SELECT 1 FROM model_assets dma JOIN assets da ON da.id = dma.asset_id WHERE dma.model_id = m.id AND da.content_hash IS NOT NULL AND EXISTS (SELECT 1 FROM model_assets oma JOIN assets oa ON oa.id = oma.asset_id WHERE oma.model_id != m.id AND oa.content_hash = da.content_hash))) ");
    let count_sql = format!("SELECT COUNT(*) FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id{where_sql}");
    let total: i64 = sqlx::query_scalar(&count_sql)
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
        .map_err(db_error)?;
    let order = match query.sort.as_deref() {
        Some("name") => "lower(m.display_name) ASC, m.id ASC",
        Some("added") => "m.added_at DESC, lower(m.display_name) ASC, m.id ASC",
        Some("opened") => {
            "m.last_opened_at DESC, m.updated_at DESC, lower(m.display_name) ASC, m.id ASC"
        }
        _ => "COALESCE(pa.modified_at, m.updated_at) DESC, lower(m.display_name) ASC, m.id ASC",
    };
    let data_sql = format!("SELECT m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(pa.modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, COUNT(DISTINCT ma.asset_id) asset_count, COALESCE(pa.metadata_json, '{{}}') metadata_json FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id LEFT JOIN model_assets ma ON ma.model_id = m.id{where_sql}GROUP BY m.id ORDER BY {order} LIMIT ? OFFSET ?");
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
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let items = rows
        .into_iter()
        .map(summary_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Page {
        items,
        total,
        next_offset: if offset + limit < total {
            Some(offset + limit)
        } else {
            None
        },
    })
}

#[tauri::command]
pub async fn get_model(model_id: String, state: State<'_, AppState>) -> CommandResult<ModelDetail> {
    let row = sqlx::query("SELECT m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(pa.modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, COUNT(DISTINCT ma.asset_id) asset_count, COALESCE(pa.metadata_json, '{}') metadata_json, m.notes, r.id root_id, r.display_name root_name, r.path root_path FROM models m JOIN folders f ON f.id = m.folder_id JOIN library_roots r ON r.id = f.root_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id LEFT JOIN model_assets ma ON ma.model_id = m.id WHERE m.id = ? GROUP BY m.id").bind(&model_id).fetch_optional(&state.pool).await.map_err(db_error)?.ok_or_else(|| "Model not found".to_string())?;
    let notes = row.get("notes");
    let root_id = row.get("root_id");
    let root_name = row.get("root_name");
    let root_path = row.get("root_path");
    let summary = summary_from_row(row)?;
    let rows = sqlx::query("SELECT a.id, a.filename, a.extension, a.relative_path, a.byte_size, a.modified_at, a.parse_status, a.metadata_json, a.missing_since FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE ma.model_id = ? ORDER BY CASE a.extension WHEN '3mf' THEN 0 WHEN 'stl' THEN 1 WHEN 'obj' THEN 2 ELSE 3 END, a.filename").bind(&model_id).fetch_all(&state.pool).await.map_err(db_error)?;
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
    let estimate = get_estimate(&model_id, &state).await?;
    let web_source = web_sources::source_for_model(&model_id, &state).await?;
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
pub async fn list_collections(state: State<'_, AppState>) -> CommandResult<Vec<Collection>> {
    let rows = sqlx::query("SELECT c.id, c.name, c.symbol, c.color, c.created_at, c.kind, c.rule_json, COUNT(ci.model_id) model_count FROM collections c LEFT JOIN collection_items ci ON ci.collection_id = c.id GROUP BY c.id ORDER BY c.sort_order, lower(c.name)").fetch_all(&state.pool).await.map_err(db_error)?;
    let mut collections = Vec::with_capacity(rows.len());
    for row in rows {
        let smart = row.get::<String, _>("kind") == "smart";
        let rule = smart.then(|| {
            serde_json::from_str::<SmartCollectionRule>(&row.get::<String, _>("rule_json"))
                .unwrap_or_default()
        });
        let model_count = if let Some(rule) = &rule {
            count_smart_collection(rule, &state.pool).await?
        } else {
            row.get("model_count")
        };
        collections.push(Collection {
            id: row.get("id"),
            name: row.get("name"),
            symbol: row.get("symbol"),
            color: row.get("color"),
            model_count,
            created_at: row.get("created_at"),
            smart,
            rule,
        });
    }
    Ok(collections)
}

#[tauri::command]
pub async fn create_collection(
    input: CollectionInput,
    state: State<'_, AppState>,
) -> CommandResult<Collection> {
    validate_collection(&input)?;
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let order: f64 = sqlx::query_scalar("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections")
        .fetch_one(&state.pool)
        .await
        .map_err(db_error)?;
    let kind = if input.smart { "smart" } else { "manual" };
    let rule_json = serde_json::to_string(&input.rule.clone().unwrap_or_default())
        .map_err(|error| error.to_string())?;
    sqlx::query("INSERT INTO collections (id, name, symbol, color, sort_order, created_at, updated_at, kind, rule_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)").bind(&id).bind(&input.name).bind(&input.symbol).bind(&input.color).bind(order).bind(&timestamp).bind(&timestamp).bind(kind).bind(rule_json).execute(&state.pool).await.map_err(db_error)?;
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
    validate_collection(&input)?;
    let kind = if input.smart { "smart" } else { "manual" };
    let rule_json = serde_json::to_string(&input.rule.clone().unwrap_or_default())
        .map_err(|error| error.to_string())?;
    sqlx::query(
        "UPDATE collections SET name = ?, symbol = ?, color = ?, kind = ?, rule_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.symbol)
    .bind(&input.color)
    .bind(kind)
    .bind(rule_json)
    .bind(now())
    .bind(&collection_id)
    .execute(&state.pool)
    .await
    .map_err(db_error)?;
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM collection_items WHERE collection_id = ?")
            .bind(&collection_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    for id in &model_ids {
        indexing::refresh_search(id, &state.pool).await?;
    }
    let model_count = if input.smart {
        count_smart_collection(input.rule.as_ref().unwrap(), &state.pool).await?
    } else {
        model_ids.len() as i64
    };
    Ok(Collection {
        id: collection_id,
        name: input.name,
        symbol: input.symbol,
        color: input.color,
        model_count,
        created_at: now(),
        smart: input.smart,
        rule: input.rule,
    })
}

#[tauri::command]
pub async fn delete_collection(
    collection_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM collection_items WHERE collection_id = ?")
            .bind(&collection_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    sqlx::query("DELETE FROM collections WHERE id = ?")
        .bind(&collection_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    for id in ids {
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
    ensure_manual_collection(&collection_id, &state.pool).await?;
    let timestamp = now();
    for (index, model_id) in model_ids.iter().enumerate() {
        sqlx::query("INSERT OR IGNORE INTO collection_items (collection_id, model_id, position, added_at) VALUES (?, ?, ?, ?)").bind(&collection_id).bind(model_id).bind(index as f64).bind(&timestamp).execute(&state.pool).await.map_err(db_error)?;
        indexing::refresh_search(model_id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_models_from_collection(
    collection_id: String,
    model_ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    ensure_manual_collection(&collection_id, &state.pool).await?;
    for model_id in model_ids {
        sqlx::query("DELETE FROM collection_items WHERE collection_id = ? AND model_id = ?")
            .bind(&collection_id)
            .bind(&model_id)
            .execute(&state.pool)
            .await
            .map_err(db_error)?;
        indexing::refresh_search(&model_id, &state.pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> CommandResult<Vec<Tag>> {
    let rows = sqlx::query("SELECT t.id, t.name, t.color, COUNT(mt.model_id) model_count FROM tags t LEFT JOIN model_tags mt ON mt.tag_id = t.id GROUP BY t.id ORDER BY lower(t.name)")
        .fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows.into_iter().map(tag_from_row).collect())
}

#[tauri::command]
pub async fn save_tag(input: TagInput, state: State<'_, AppState>) -> CommandResult<Tag> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Tag names must contain 1–40 characters".into());
    }
    if !valid_color(&input.color) {
        return Err("Tag color must be a hex color".into());
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = now();
    sqlx::query("INSERT INTO tags (id, name, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color, updated_at = excluded.updated_at")
        .bind(&id).bind(name).bind(&input.color).bind(&timestamp).bind(&timestamp).execute(&state.pool).await.map_err(|error| {
            if error.to_string().contains("UNIQUE") { "A tag with that name already exists".into() } else { db_error(error) }
        })?;
    Ok(Tag {
        id,
        name: name.to_string(),
        color: input.color,
        model_count: 0,
    })
}

#[tauri::command]
pub async fn delete_tag(tag_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    let model_ids: Vec<String> =
        sqlx::query_scalar("SELECT model_id FROM model_tags WHERE tag_id = ?")
            .bind(&tag_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db_error)?;
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(tag_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    for id in model_ids {
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
    let version_key: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(version_key, '') FROM models WHERE id = ?")
            .bind(&model_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?
            .flatten();
    let rows = sqlx::query("SELECT DISTINCT other.id, other.display_name, f.relative_path, COALESCE(pa.extension, '') primary_extension, COALESCE(pa.modified_at, other.updated_at) modified_at, CASE WHEN EXISTS (SELECT 1 FROM model_assets mine JOIN assets mine_asset ON mine_asset.id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN assets their_asset ON their_asset.id = theirs.asset_id AND their_asset.content_hash = mine_asset.content_hash WHERE mine.model_id = ? AND mine_asset.content_hash IS NOT NULL) THEN 'duplicate' ELSE 'version' END relationship FROM models other JOIN folders f ON f.id = other.folder_id LEFT JOIN assets pa ON pa.id = other.primary_asset_id WHERE other.id != ? AND (EXISTS (SELECT 1 FROM model_assets mine JOIN assets mine_asset ON mine_asset.id = mine.asset_id JOIN model_assets theirs ON theirs.model_id = other.id JOIN assets their_asset ON their_asset.id = theirs.asset_id AND their_asset.content_hash = mine_asset.content_hash WHERE mine.model_id = ? AND mine_asset.content_hash IS NOT NULL) OR (? != '' AND other.version_key = ?)) ORDER BY relationship, modified_at DESC LIMIT 50")
        .bind(&model_id).bind(&model_id).bind(&model_id).bind(version_key.clone().unwrap_or_default()).bind(version_key.unwrap_or_default()).fetch_all(&state.pool).await.map_err(db_error)?;
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
    let row = sqlx::query("SELECT COUNT(*) groups_count, COALESCE(SUM(model_count), 0) models_count, COALESCE(SUM(model_count - 1), 0) redundant_count FROM (SELECT a.content_hash, COUNT(DISTINCT ma.model_id) model_count FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE a.missing_since IS NULL AND a.content_hash IS NOT NULL GROUP BY a.content_hash HAVING COUNT(DISTINCT ma.model_id) > 1)")
        .fetch_one(&state.pool).await.map_err(db_error)?;
    Ok(DuplicateStats {
        groups: row.get("groups_count"),
        models: row.get("models_count"),
        redundant_copies: row.get("redundant_count"),
    })
}

#[tauri::command]
pub async fn list_duplicate_groups(
    offset: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> CommandResult<Page<DuplicateGroup>> {
    let offset = offset.max(0);
    let limit = limit.clamp(1, 100);
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT a.content_hash FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE a.missing_since IS NULL AND a.content_hash IS NOT NULL GROUP BY a.content_hash HAVING COUNT(DISTINCT ma.model_id) > 1)")
        .fetch_one(&state.pool).await.map_err(db_error)?;
    let hashes = sqlx::query("SELECT a.content_hash, MAX(a.byte_size) byte_size, COUNT(DISTINCT ma.model_id) model_count, MAX(a.modified_at) latest_modified FROM assets a JOIN model_assets ma ON ma.asset_id = a.id WHERE a.missing_since IS NULL AND a.content_hash IS NOT NULL GROUP BY a.content_hash HAVING COUNT(DISTINCT ma.model_id) > 1 ORDER BY latest_modified DESC LIMIT ? OFFSET ?")
        .bind(limit).bind(offset).fetch_all(&state.pool).await.map_err(db_error)?;
    let mut groups = Vec::with_capacity(hashes.len());
    for hash_row in hashes {
        let hash: String = hash_row.get("content_hash");
        let rows = sqlx::query("SELECT m.id, m.display_name, m.folder_id, f.name folder_name, f.relative_path, m.primary_asset_id, COALESCE(pa.extension, '') primary_extension, m.favorite, m.added_at, COALESCE(pa.modified_at, m.updated_at) modified_at, m.last_opened_at, m.missing_since, COUNT(DISTINCT all_ma.asset_id) asset_count, COALESCE(pa.metadata_json, '{}') metadata_json FROM models m JOIN folders f ON f.id = m.folder_id LEFT JOIN assets pa ON pa.id = m.primary_asset_id JOIN model_assets matching_ma ON matching_ma.model_id = m.id JOIN assets matching_asset ON matching_asset.id = matching_ma.asset_id LEFT JOIN model_assets all_ma ON all_ma.model_id = m.id WHERE matching_asset.content_hash = ? AND matching_asset.missing_since IS NULL GROUP BY m.id ORDER BY lower(m.display_name), f.relative_path")
            .bind(&hash).fetch_all(&state.pool).await.map_err(db_error)?;
        let models = rows
            .into_iter()
            .map(summary_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        groups.push(DuplicateGroup {
            id: hash,
            model_count: hash_row.get("model_count"),
            byte_size: hash_row.get("byte_size"),
            models,
        });
    }
    Ok(Page {
        items: groups,
        total,
        next_offset: (offset + limit < total).then_some(offset + limit),
    })
}

#[tauri::command]
pub async fn list_materials(state: State<'_, AppState>) -> CommandResult<Vec<Material>> {
    let rows = sqlx::query("SELECT id, name, material_type, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density, updated_at FROM materials ORDER BY lower(name)").fetch_all(&state.pool).await.map_err(db_error)?;
    Ok(rows.into_iter().map(material_from_row).collect())
}

#[tauri::command]
pub async fn save_material(
    input: MaterialInput,
    state: State<'_, AppState>,
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
) -> CommandResult<Option<Vec<u8>>> {
    let (path, extension) = asset_path(&asset_id, &state).await?;
    if extension != "3mf" {
        return Err("Plate thumbnails are only available for 3MF files".into());
    }
    tokio::task::spawn_blocking(move || parsers::embedded_3mf_plate_thumbnail(&path, plate_index))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_viewer_mesh(
    asset_id: String,
    plate_index: Option<u32>,
    state: State<'_, AppState>,
) -> CommandResult<tauri::ipc::Response> {
    let (path, extension) = asset_path(&asset_id, &state).await?;
    let bytes =
        tokio::task::spawn_blocking(move || previews::viewer_mesh(&path, &extension, plate_index))
            .await
            .map_err(|error| error.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn get_cached_thumbnail(
    asset_id: String,
    state: State<'_, AppState>,
) -> CommandResult<Option<Vec<u8>>> {
    read_cached_thumbnail(&asset_id, &state.pool).await
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
    if !matches!(extension.as_str(), "stl" | "obj" | "3mf") {
        return Ok(false);
    }
    {
        let mut pending = state.thumbnail_pending.lock().await;
        if !pending.insert(asset_id.clone()) {
            return Ok(false);
        }
    }
    let pool = state.pool.clone();
    let data_dir = state.data_dir.clone();
    let workers = state.thumbnail_workers.clone();
    let pending = state.thumbnail_pending.clone();
    let queued_asset_id = asset_id.clone();
    tauri::async_runtime::spawn(async move {
        let result: CommandResult<()> = async {
            let _worker = workers
                .acquire_owned()
                .await
                .map_err(|_| "Thumbnail worker queue is unavailable".to_string())?;
            if read_cached_thumbnail(&queued_asset_id, &pool)
                .await?
                .is_some()
            {
                return Ok(());
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
            if current_thumbnail_revision(&queued_asset_id, &pool)
                .await
                .ok()
                .as_deref()
                == Some(revision.as_str())
            {
                persist_thumbnail(&queued_asset_id, &revision, &bytes, &pool, &data_dir).await?;
            }
            Ok(())
        }
        .await;
        pending.lock().await.remove(&queued_asset_id);
        match result {
            Ok(()) => {
                let _ = app.emit("preview-ready", &queued_asset_id);
            }
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

async fn get_estimate(
    model_id: &str,
    state: &State<'_, AppState>,
) -> CommandResult<Option<CostEstimate>> {
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

async fn read_cached_thumbnail(
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
            let _ = sqlx::query("UPDATE thumbnails SET created_at = ? WHERE id = ?")
                .bind(now())
                .bind(row.get::<String, _>("id"))
                .execute(pool)
                .await;
            Ok(Some(bytes))
        }
        Err(_) => Ok(None),
    }
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
    sqlx::query("INSERT INTO thumbnails (id, asset_id, fingerprint, renderer_version, preset, width, height, cache_path, created_at) VALUES (?, ?, ?, 'cpu-4', 'studio', 480, 360, ?, ?) ON CONFLICT(asset_id, fingerprint, renderer_version, preset, width, height) DO UPDATE SET cache_path = excluded.cache_path, created_at = excluded.created_at")
        .bind(Uuid::new_v4().to_string())
        .bind(asset_id)
        .bind(fingerprint)
        .bind(path.to_string_lossy().to_string())
        .bind(now())
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn asset_path(
    asset_id: &str,
    state: &State<'_, AppState>,
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
        Ok(())
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
    let format = rule.format.clone().unwrap_or_default().to_ascii_lowercase();
    let availability = rule.availability.clone().unwrap_or_default();
    let tag = rule.tag_id.clone().unwrap_or_default();
    sqlx::query_scalar("SELECT COUNT(*) FROM models m LEFT JOIN assets pa ON pa.id = m.primary_asset_id WHERE (? = '' OR pa.extension = ?) AND (? = '' OR (? = 'available' AND m.missing_since IS NULL) OR (? = 'offline' AND m.missing_since IS NOT NULL)) AND (? = '' OR EXISTS (SELECT 1 FROM model_tags mt WHERE mt.model_id = m.id AND mt.tag_id = ?))")
        .bind(&format).bind(&format).bind(&availability).bind(&availability).bind(&availability).bind(&tag).bind(&tag).fetch_one(pool).await.map_err(db_error)
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

fn open_with(path: &Path, application: &str) -> CommandResult<()> {
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

fn reveal(path: &Path) -> CommandResult<()> {
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
}
