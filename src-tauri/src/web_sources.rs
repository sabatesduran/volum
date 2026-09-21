use crate::{
    domain::{WebSource, WebSourcePreview},
    indexing,
    state::AppState,
};
use chrono::Utc;
use regex::Regex;
use reqwest::{header, redirect, Client, StatusCode, Url};
use sqlx::{sqlite::SqliteRow, Row};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;
const MAX_PAGE_BYTES: u64 = 3 * 1024 * 1024;
const SUPPORTED_ATTACHMENTS: &[&str] = &["3mf", "stl", "obj", "step", "stp"];

#[tauri::command]
pub async fn preview_web_source(url: String) -> CommandResult<WebSourcePreview> {
    let (provider, remote_id, canonical_url) = validate_and_canonicalize(&url)?;
    match fetch_provider_preview(&provider, remote_id.as_deref(), &canonical_url).await {
        Ok(preview) => Ok(preview),
        Err(api_error) => match fetch_html(canonical_url.clone()).await {
            Ok((final_url, html)) => {
                let (provider, remote_id, canonical_url) =
                    validate_and_canonicalize(final_url.as_str())?;
                Ok(parse_preview(&html, provider, remote_id, canonical_url))
            }
            Err(_) => Err(api_error),
        },
    }
}

#[tauri::command]
pub async fn list_web_sources(state: State<'_, AppState>) -> CommandResult<Vec<WebSource>> {
    let rows = sqlx::query(&format!("{WEB_SOURCE_SELECT} ORDER BY ws.updated_at DESC"))
        .fetch_all(&state.pool)
        .await
        .map_err(db_error)?;
    let sources = rows
        .into_iter()
        .map(web_source_from_row)
        .collect::<Vec<_>>();
    for source in &sources {
        if let Some(model_id) = &source.model_id {
            sqlx::query("UPDATE web_sources SET model_id = ?, status = 'imported' WHERE id = ? AND (model_id IS NULL OR model_id != ? OR status != 'imported')")
                .bind(model_id)
                .bind(&source.id)
                .bind(model_id)
                .execute(&state.pool)
                .await
                .map_err(db_error)?;
        }
    }
    Ok(sources)
}

#[tauri::command]
pub async fn save_web_source(
    preview: WebSourcePreview,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<WebSource> {
    let (provider, remote_id, canonical_url) = validate_and_canonicalize(&preview.canonical_url)?;
    if provider != preview.provider {
        return Err("The source provider does not match its URL".into());
    }
    let title = bounded_text(&preview.title, 200)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "The model title is empty".to_string())?;
    let creator = preview
        .creator
        .as_deref()
        .and_then(|value| bounded_text(value, 200));
    let description = preview
        .description
        .as_deref()
        .and_then(|value| bounded_text(value, 4_000));
    let license = preview
        .license
        .as_deref()
        .and_then(|value| bounded_text(value, 200));
    let image_url = preview.image_url.as_deref().and_then(valid_https_url);
    let filament_grams = preview
        .filament_grams
        .filter(|value| value.is_finite() && *value > 0.0);
    let timestamp = now();
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM web_sources WHERE canonical_url = ?")
            .bind(canonical_url.as_str())
            .fetch_optional(&state.pool)
            .await
            .map_err(db_error)?;
    let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO web_sources (id, provider, remote_id, canonical_url, title, creator, description, license, image_url, filament_grams, status, fetched_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'saved', ?, ?, ?) ON CONFLICT(canonical_url) DO UPDATE SET provider = excluded.provider, remote_id = excluded.remote_id, title = excluded.title, creator = excluded.creator, description = excluded.description, license = excluded.license, image_url = excluded.image_url, filament_grams = excluded.filament_grams, fetched_at = excluded.fetched_at, updated_at = excluded.updated_at")
        .bind(&id)
        .bind(provider)
        .bind(remote_id)
        .bind(canonical_url.as_str())
        .bind(title)
        .bind(creator)
        .bind(description)
        .bind(license)
        .bind(image_url)
        .bind(filament_grams)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = app.emit("web-sources-changed", &id);
    get_web_source(&id, &state.pool).await
}

#[tauri::command]
pub async fn delete_web_source(
    source_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    sqlx::query("DELETE FROM web_sources WHERE id = ?")
        .bind(&source_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = app.emit("web-sources-changed", &source_id);
    Ok(())
}

#[tauri::command]
pub async fn attach_web_source_file(
    source_id: String,
    local_path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<WebSource> {
    let source_path = PathBuf::from(local_path);
    if !source_path.is_file() {
        return Err("Choose a downloaded model file".into());
    }
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !SUPPORTED_ATTACHMENTS.contains(&extension.as_str()) {
        return Err("Volum can attach 3MF, STL, OBJ, STEP, or STP files".into());
    }
    let source_row = sqlx::query("SELECT provider, remote_id, canonical_url, title, creator, description, license, filament_grams FROM web_sources WHERE id = ?")
        .bind(&source_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Saved web model not found".to_string())?;
    let (root_id, root_canonical, import_directory, relative_import_directory) =
        resolve_import_directory(&state).await?;
    let provider: String = source_row.get("provider");
    let title: String = source_row.get("title");
    let remote_id: Option<String> = source_row.get("remote_id");
    let model_folder = match remote_id {
        Some(remote_id) => format!(
            "{} ({})",
            safe_component(&title),
            safe_component(&remote_id)
        ),
        None => safe_component(&title),
    };
    let relative_directory = relative_import_directory
        .join(provider_label(&provider))
        .join(&model_folder);
    let destination_directory = import_directory
        .join(provider_label(&provider))
        .join(model_folder);
    tokio::fs::create_dir_all(&destination_directory)
        .await
        .map_err(|error| format!("Could not create the import folder: {error}"))?;
    let destination_directory = std::fs::canonicalize(&destination_directory)
        .map_err(|error| format!("Could not verify the import folder: {error}"))?;
    if !destination_directory.starts_with(&root_canonical) {
        return Err("The import destination is outside the library".into());
    }
    let original_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("downloaded-model.3mf");
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(safe_component)
        .unwrap_or_else(|| "downloaded-model".into());
    let mut filename = safe_filename(original_name, &stem, &extension);
    let mut destination = destination_directory.join(&filename);
    let mut suffix = 2_u32;
    while destination.exists() {
        filename = format!("{stem}-{suffix}.{extension}");
        destination = destination_directory.join(&filename);
        suffix += 1;
    }
    let temporary = destination_directory.join(format!(".{filename}.{}.part", Uuid::new_v4()));
    tokio::fs::copy(&source_path, &temporary)
        .await
        .map_err(|error| format!("Could not copy the downloaded file: {error}"))?;
    if let Err(error) = tokio::fs::rename(&temporary, &destination).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("Could not finish the import: {error}"));
    }
    let sidecar_path = destination_directory.join(".volum-source.json");
    let sidecar_temporary =
        destination_directory.join(format!(".volum-source.{}.tmp", Uuid::new_v4()));
    let sidecar = serde_json::json!({
        "version": 1,
        "provider": &provider,
        "remoteId": source_row.get::<Option<String>, _>("remote_id"),
        "url": source_row.get::<String, _>("canonical_url"),
        "title": &title,
        "creator": source_row.get::<Option<String>, _>("creator"),
        "description": source_row.get::<Option<String>, _>("description"),
        "license": source_row.get::<Option<String>, _>("license"),
        "filamentGrams": source_row.get::<Option<f64>, _>("filament_grams"),
        "file": &filename,
        "importedAt": now()
    });
    if let Err(error) = tokio::fs::write(
        &sidecar_temporary,
        serde_json::to_vec_pretty(&sidecar).map_err(|error| error.to_string())?,
    )
    .await
    {
        let _ = tokio::fs::remove_file(&destination).await;
        return Err(format!("Could not save source attribution: {error}"));
    }
    if sidecar_path.exists() {
        let _ = tokio::fs::remove_file(&sidecar_path).await;
    }
    if let Err(error) = tokio::fs::rename(&sidecar_temporary, &sidecar_path).await {
        let _ = tokio::fs::remove_file(&sidecar_temporary).await;
        let _ = tokio::fs::remove_file(&destination).await;
        return Err(format!("Could not finish source attribution: {error}"));
    }
    let relative_path = relative_directory
        .join(filename)
        .to_string_lossy()
        .replace('\\', "/");
    sqlx::query("UPDATE web_sources SET status = 'importing', root_id = ?, relative_path = ?, model_id = NULL, updated_at = ? WHERE id = ?")
        .bind(&root_id)
        .bind(&relative_path)
        .bind(now())
        .bind(&source_id)
        .execute(&state.pool)
        .await
        .map_err(db_error)?;
    let _ = app.emit("web-sources-changed", &source_id);
    indexing::start(root_id, &state, app).await?;
    get_web_source(&source_id, &state.pool).await
}

async fn resolve_import_directory(
    state: &AppState,
) -> CommandResult<(String, PathBuf, PathBuf, PathBuf)> {
    let configured: Option<String> = sqlx::query_scalar("SELECT value_json FROM user_overrides WHERE entity_type = 'app' AND entity_id = 'global' AND key = 'web_import_folder'")
        .fetch_optional(&state.pool)
        .await
        .map_err(db_error)?
        .and_then(|value: String| serde_json::from_str(&value).ok())
        .filter(|value: &String| !value.is_empty());
    let rows = sqlx::query(
        "SELECT id, path FROM library_roots WHERE status != 'removed' ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(db_error)?;
    if let Some(configured) = configured {
        let folder = std::fs::canonicalize(&configured)
            .map_err(|_| "The configured web import folder is unavailable".to_string())?;
        let mut matches = rows
            .iter()
            .filter_map(|row| {
                let root = std::fs::canonicalize(row.get::<String, _>("path")).ok()?;
                folder.starts_with(&root).then_some((row, root))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(_, root)| std::cmp::Reverse(root.components().count()));
        if let Some((row, root)) = matches.into_iter().next() {
            let relative = folder
                .strip_prefix(&root)
                .map_err(|_| "Could not resolve the configured import folder".to_string())?
                .to_path_buf();
            return Ok((row.get("id"), root, folder, relative));
        }
        return Err("The configured web import folder is not inside an active library".into());
    }
    let row = rows
        .first()
        .ok_or_else(|| "Add a library before attaching web models".to_string())?;
    let root = std::fs::canonicalize(row.get::<String, _>("path"))
        .map_err(|_| "The default library folder is unavailable".to_string())?;
    Ok((
        row.get("id"),
        root.clone(),
        root.join("Web Imports"),
        PathBuf::from("Web Imports"),
    ))
}

pub async fn source_for_model(
    model_id: &str,
    state: &AppState,
) -> CommandResult<Option<WebSource>> {
    let row = sqlx::query(&format!(
        "{WEB_SOURCE_SELECT} WHERE ws.model_id = ? OR EXISTS (SELECT 1 FROM assets linked_a JOIN model_assets linked_ma ON linked_ma.asset_id = linked_a.id WHERE linked_ma.model_id = ? AND linked_a.root_id = ws.root_id AND linked_a.relative_path = ws.relative_path) LIMIT 1"
    ))
    .bind(model_id)
    .bind(model_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_error)?;
    Ok(row.map(web_source_from_row))
}

const WEB_SOURCE_SELECT: &str = "SELECT ws.id, ws.provider, ws.remote_id, ws.canonical_url, ws.title, ws.creator, ws.description, ws.license, ws.image_url, ws.filament_grams, ws.status, ws.root_id, ws.relative_path, ws.fetched_at, ws.created_at, ws.updated_at, COALESCE(ws.model_id, (SELECT linked_ma.model_id FROM assets linked_a JOIN model_assets linked_ma ON linked_ma.asset_id = linked_a.id WHERE linked_a.root_id = ws.root_id AND linked_a.relative_path = ws.relative_path LIMIT 1)) resolved_model_id FROM web_sources ws";

async fn get_web_source(id: &str, pool: &sqlx::SqlitePool) -> CommandResult<WebSource> {
    let row = sqlx::query(&format!("{WEB_SOURCE_SELECT} WHERE ws.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?
        .ok_or_else(|| "Saved web model not found".to_string())?;
    Ok(web_source_from_row(row))
}

fn web_source_from_row(row: SqliteRow) -> WebSource {
    let model_id: Option<String> = row.get("resolved_model_id");
    let relative_path: Option<String> = row.get("relative_path");
    let stored_status: String = row.get("status");
    let status = if model_id.is_some() {
        "imported".to_string()
    } else if relative_path.is_some() {
        "importing".to_string()
    } else {
        stored_status
    };
    WebSource {
        id: row.get("id"),
        preview: WebSourcePreview {
            provider: row.get("provider"),
            remote_id: row.get("remote_id"),
            canonical_url: row.get("canonical_url"),
            title: row.get("title"),
            creator: row.get("creator"),
            description: row.get("description"),
            license: row.get("license"),
            image_url: row.get("image_url"),
            filament_grams: row.get("filament_grams"),
        },
        status,
        root_id: row.get("root_id"),
        relative_path,
        model_id,
        fetched_at: row.get("fetched_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn fetch_provider_preview(
    provider: &str,
    remote_id: Option<&str>,
    canonical_url: &Url,
) -> CommandResult<WebSourcePreview> {
    let remote_id = remote_id.ok_or_else(|| "The model URL has no model ID".to_string())?;
    let client = Client::builder()
        .redirect(redirect::Policy::limited(3))
        .timeout(Duration::from_secs(18))
        .build()
        .map_err(|error| error.to_string())?;
    let payload: serde_json::Value = if provider == "printables" {
        let query = format!("{{print(id:{remote_id}){{id name description summary user{{publicUsername}} license{{name}} image{{filePath}}}}}}");
        fetch_json(
            client
                .get("https://api.printables.com/graphql/")
                .query(&[("query", query)]),
        )
        .await?
    } else {
        fetch_json(client.get(format!(
            "https://makerworld.com/api/v1/design-service/design/{remote_id}"
        )))
        .await?
    };
    if provider == "printables" {
        if let Some(message) = payload
            .get("errors")
            .and_then(|errors| errors.as_array())
            .and_then(|errors| errors.first())
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
        {
            return Err(format!("Printables could not find that model: {message}"));
        }
        let model = payload
            .pointer("/data/print")
            .filter(|value| !value.is_null())
            .ok_or_else(|| "Printables could not find that model".to_string())?;
        let image_url = model
            .pointer("/image/filePath")
            .and_then(|value| value.as_str())
            .and_then(printables_thumbnail_url);
        Ok(WebSourcePreview {
            provider: provider.into(),
            remote_id: Some(remote_id.into()),
            canonical_url: canonical_url.to_string(),
            title: json_text(model, &["name"], 200).unwrap_or_else(|| "Printables model".into()),
            creator: model
                .pointer("/user/publicUsername")
                .and_then(|value| value.as_str())
                .and_then(|value| bounded_text(value, 200)),
            description: json_text(model, &["summary", "description"], 4_000),
            license: model
                .pointer("/license/name")
                .and_then(|value| value.as_str())
                .and_then(|value| bounded_text(value, 200)),
            image_url,
            filament_grams: None,
        })
    } else {
        if payload.get("code").and_then(|value| value.as_i64()) == Some(404) {
            return Err("MakerWorld could not find that model".into());
        }
        Ok(WebSourcePreview {
            provider: provider.into(),
            remote_id: Some(remote_id.into()),
            canonical_url: canonical_url.to_string(),
            title: json_text(&payload, &["titleTranslated", "title"], 200)
                .unwrap_or_else(|| "MakerWorld model".into()),
            creator: payload
                .pointer("/designCreator/name")
                .and_then(|value| value.as_str())
                .and_then(|value| bounded_text(value, 200)),
            description: json_text(&payload, &["summaryTranslated", "summary"], 4_000),
            license: json_text(&payload, &["license"], 200),
            image_url: payload
                .get("coverUrl")
                .and_then(|value| value.as_str())
                .and_then(valid_https_url),
            filament_grams: makerworld_filament_grams(&payload),
        })
    }
}

async fn fetch_json(request: reqwest::RequestBuilder) -> CommandResult<serde_json::Value> {
    let response = request
        .header(header::ACCEPT, "application/json")
        .header(
            header::USER_AGENT,
            "Volum/0.1 metadata preview (+https://didac.dev)",
        )
        .send()
        .await
        .map_err(|error| format!("Could not reach the model provider: {error}"))?;
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err("The provider is rate limiting requests. Try again shortly.".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "The provider could not load that model ({})",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_PAGE_BYTES)
    {
        return Err("The provider response is too large to preview safely".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Could not read the provider response: {error}"))?;
    if bytes.len() as u64 > MAX_PAGE_BYTES {
        return Err("The provider response is too large to preview safely".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("The provider returned invalid metadata: {error}"))
}

fn json_text(value: &serde_json::Value, keys: &[&str], maximum: usize) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(key).and_then(|value| value.as_str()))
        .find_map(|value| bounded_text(value, maximum))
}

fn makerworld_filament_grams(payload: &serde_json::Value) -> Option<f64> {
    let instances = payload.get("instances")?.as_array()?;
    let default_id = payload.get("defaultInstanceId");
    let selected = default_id
        .and_then(|default_id| {
            instances
                .iter()
                .find(|instance| instance.get("id").is_some_and(|id| id == default_id))
        })
        .or_else(|| {
            instances.iter().find(|instance| {
                instance.get("isDefault").and_then(|value| value.as_bool()) == Some(true)
            })
        })
        .or_else(|| (instances.len() == 1).then(|| &instances[0]))?;
    let filament_values = selected
        .get("instanceFilaments")
        .and_then(|value| value.as_array())
        .and_then(|filaments| {
            filaments
                .iter()
                .map(|filament| filament.get("usedG").and_then(positive_json_number))
                .collect::<Option<Vec<_>>>()
        })
        .filter(|values| !values.is_empty());
    filament_values
        .map(|values| values.into_iter().sum())
        .or_else(|| selected.get("weight").and_then(positive_json_number))
}

fn positive_json_number(value: &serde_json::Value) -> Option<f64> {
    let value = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn printables_thumbnail_url(file_path: &str) -> Option<String> {
    if file_path.len() > 1_600 || file_path.contains("..") {
        return None;
    }
    let (parent, filename) = file_path.rsplit_once('/')?;
    valid_https_url(&format!(
        "https://media.printables.com/{parent}/thumbs/inside/640x480/jpg/{filename}"
    ))
}

async fn fetch_html(mut url: Url) -> CommandResult<(Url, String)> {
    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .timeout(Duration::from_secs(18))
        .build()
        .map_err(|error| error.to_string())?;
    for _ in 0..4 {
        let response = client
            .get(url.clone())
            .header(header::ACCEPT, "text/html,application/xhtml+xml")
            .header(
                header::USER_AGENT,
                "Volum/0.1 metadata preview (+https://didac.dev)",
            )
            .send()
            .await
            .map_err(|error| format!("Could not reach the model page: {error}"))?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "The model page returned an invalid redirect".to_string())?;
            let redirected = url
                .join(location)
                .map_err(|_| "The model page returned an invalid redirect".to_string())?;
            let (_, _, checked) = validate_and_canonicalize(redirected.as_str())?;
            url = checked;
            continue;
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err("The provider is rate limiting requests. Try again shortly.".into());
        }
        if !response.status().is_success() {
            return Err(format!(
                "The model page could not be loaded ({})",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_PAGE_BYTES)
        {
            return Err("The model page is too large to preview safely".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("Could not read the model page: {error}"))?;
        if bytes.len() as u64 > MAX_PAGE_BYTES {
            return Err("The model page is too large to preview safely".into());
        }
        return Ok((url, String::from_utf8_lossy(&bytes).into_owned()));
    }
    Err("The model page redirected too many times".into())
}

fn validate_and_canonicalize(input: &str) -> CommandResult<(String, Option<String>, Url)> {
    if input.len() > 2_048 {
        return Err("The URL is too long".into());
    }
    let mut url = Url::parse(input.trim()).map_err(|_| "Enter a valid HTTPS URL".to_string())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("Only public HTTPS model links are supported".into());
    }
    if url.port().is_some_and(|port| port != 443) {
        return Err("Custom URL ports are not supported".into());
    }
    url.set_port(None)
        .map_err(|_| "Invalid URL port".to_string())?;
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let provider = if matches!(host.as_str(), "makerworld.com" | "www.makerworld.com") {
        "makerworld"
    } else if matches!(host.as_str(), "printables.com" | "www.printables.com") {
        "printables"
    } else {
        return Err("Use a MakerWorld or Printables model URL".into());
    };
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let marker = if provider == "makerworld" {
        "models"
    } else {
        "model"
    };
    let marker_index = segments
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case(marker))
        .ok_or_else(|| "The URL must point to a model page".to_string())?;
    let slug = segments
        .get(marker_index + 1)
        .ok_or_else(|| "The URL must include a model ID".to_string())?;
    let remote_id = slug
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if remote_id.is_empty() {
        return Err("The URL must include a model ID".into());
    }
    let canonical_segments = &segments[..=marker_index + 1];
    url.set_path(&format!("/{}", canonical_segments.join("/")));
    url.set_query(None);
    url.set_fragment(None);
    Ok((provider.into(), Some(remote_id), url))
}

fn parse_preview(
    html: &str,
    provider: String,
    remote_id: Option<String>,
    canonical_url: Url,
) -> WebSourcePreview {
    let metadata = html_metadata(html);
    let slug = canonical_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or("Imported model");
    let fallback_title = slug
        .trim_start_matches(|character: char| character.is_ascii_digit() || character == '-')
        .replace(['-', '_'], " ");
    let title = metadata
        .get("og:title")
        .or_else(|| metadata.get("twitter:title"))
        .or_else(|| metadata.get("title"))
        .cloned()
        .unwrap_or(fallback_title);
    let creator = metadata
        .get("author")
        .or_else(|| metadata.get("article:author"))
        .or_else(|| metadata.get("og:author"))
        .cloned();
    let description = metadata
        .get("og:description")
        .or_else(|| metadata.get("description"))
        .or_else(|| metadata.get("twitter:description"))
        .cloned();
    let license = metadata.get("license").cloned();
    let image_url = metadata
        .get("og:image:secure_url")
        .or_else(|| metadata.get("og:image"))
        .or_else(|| metadata.get("twitter:image"))
        .and_then(|value| valid_https_url(value));
    WebSourcePreview {
        provider,
        remote_id,
        canonical_url: canonical_url.to_string(),
        title: clean_text(&title, 200),
        creator: creator
            .map(|value| clean_text(&value, 200))
            .filter(|value| !value.is_empty()),
        description: description
            .map(|value| clean_text(&value, 4_000))
            .filter(|value| !value.is_empty()),
        license: license
            .map(|value| clean_text(&value, 200))
            .filter(|value| !value.is_empty()),
        image_url,
        filament_grams: None,
    }
}

fn html_metadata(html: &str) -> HashMap<String, String> {
    let mut output = HashMap::new();
    let tag_regex = Regex::new(r"(?is)<meta\b[^>]*>").expect("valid meta regex");
    let attr_regex = Regex::new(r#"(?is)\b([a-z_:][-a-z0-9_:]*)\s*=\s*(?:\"([^\"]*)\"|'([^']*)')"#)
        .expect("valid attribute regex");
    for tag in tag_regex.find_iter(html).take(256) {
        let mut attributes = HashMap::new();
        for captures in attr_regex.captures_iter(tag.as_str()) {
            let name = captures
                .get(1)
                .map(|value| value.as_str().to_ascii_lowercase());
            let value = captures.get(2).or_else(|| captures.get(3));
            if let (Some(name), Some(value)) = (name, value) {
                attributes.insert(name, decode_entities(value.as_str()));
            }
        }
        if let (Some(key), Some(content)) = (
            attributes
                .get("property")
                .or_else(|| attributes.get("name")),
            attributes.get("content"),
        ) {
            output
                .entry(key.to_ascii_lowercase())
                .or_insert_with(|| content.clone());
        }
    }
    let title_regex = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("valid title regex");
    if let Some(title) = title_regex
        .captures(html)
        .and_then(|captures| captures.get(1))
    {
        output.insert("title".into(), decode_entities(title.as_str()));
    }
    let license_regex =
        Regex::new(r#"(?is)\"license\"\s*:\s*\"([^\"]{1,200})\""#).expect("valid license regex");
    if let Some(license) = license_regex
        .captures(html)
        .and_then(|captures| captures.get(1))
    {
        output
            .entry("license".into())
            .or_insert_with(|| decode_entities(license.as_str()));
    }
    output
}

fn clean_text(value: &str, maximum: usize) -> String {
    let tag_regex = Regex::new(r"(?is)<[^>]+>").expect("valid tag regex");
    let without_tags = tag_regex.replace_all(value, " ");
    let collapsed = decode_entities(&without_tags)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    collapsed.chars().take(maximum).collect()
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn bounded_text(value: &str, maximum: usize) -> Option<String> {
    let value = clean_text(value, maximum);
    (!value.is_empty()).then_some(value)
}

fn valid_https_url(value: &str) -> Option<String> {
    if value.len() > 2_048 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

fn safe_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['.', ' '])
        .chars()
        .take(80)
        .collect::<String>();
    if cleaned.is_empty() {
        "Imported model".into()
    } else {
        cleaned
    }
}

fn safe_filename(original: &str, stem: &str, extension: &str) -> String {
    let cleaned = safe_component(original);
    if cleaned
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        cleaned
    } else {
        format!("{stem}.{extension}")
    }
}

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "makerworld" => "MakerWorld",
        "printables" => "Printables",
        _ => "Web",
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_canonicalizes_supported_model_urls() {
        let (provider, id, url) = validate_and_canonicalize(
            "https://www.printables.com/model/12345-useful-bracket/files?utm_source=test#preview",
        )
        .unwrap();
        assert_eq!(provider, "printables");
        assert_eq!(id.as_deref(), Some("12345"));
        assert_eq!(
            url.as_str(),
            "https://www.printables.com/model/12345-useful-bracket"
        );
        let (provider, id, url) = validate_and_canonicalize(
            "https://makerworld.com/en/models/9876-lamp?from=search#profileId-2",
        )
        .unwrap();
        assert_eq!(provider, "makerworld");
        assert_eq!(id.as_deref(), Some("9876"));
        assert_eq!(url.as_str(), "https://makerworld.com/en/models/9876-lamp");
    }

    #[test]
    fn rejects_untrusted_and_non_model_urls() {
        assert!(validate_and_canonicalize("http://printables.com/model/123-test").is_err());
        assert!(validate_and_canonicalize("https://example.com/model/123-test").is_err());
        assert!(validate_and_canonicalize("https://printables.com/@maker").is_err());
    }

    #[test]
    fn extracts_open_graph_metadata_without_rendering_html() {
        let html = r#"<html><head>
          <meta property="og:title" content="Useful &amp; Strong Bracket">
          <meta name="author" content="Ada Maker">
          <meta property="og:description" content="A useful bracket.">
          <meta property="og:image" content="https://media.printables.com/bracket.jpg">
          <script type="application/ld+json">{"license":"CC BY 4.0"}</script>
        </head></html>"#;
        let preview = parse_preview(
            html,
            "printables".into(),
            Some("123".into()),
            Url::parse("https://printables.com/model/123-bracket").unwrap(),
        );
        assert_eq!(preview.title, "Useful & Strong Bracket");
        assert_eq!(preview.creator.as_deref(), Some("Ada Maker"));
        assert_eq!(preview.license.as_deref(), Some("CC BY 4.0"));
        assert_eq!(
            preview.image_url.as_deref(),
            Some("https://media.printables.com/bracket.jpg")
        );
    }

    #[test]
    fn creates_safe_import_components() {
        assert_eq!(safe_component("Lamp: Deluxe / Final"), "Lamp Deluxe Final");
        assert_eq!(safe_filename("part.3mf", "part", "3mf"), "part.3mf");
    }

    #[test]
    fn reads_weight_from_the_default_makerworld_print_profile() {
        let payload = serde_json::json!({
            "defaultInstanceId": 20,
            "instances": [
                { "id": 10, "weight": 99 },
                {
                    "id": 20,
                    "weight": 74,
                    "instanceFilaments": [
                        { "usedG": "70.25" },
                        { "usedG": "3.75" }
                    ]
                }
            ]
        });
        assert_eq!(makerworld_filament_grams(&payload), Some(74.0));
    }

    #[tokio::test]
    async fn resolves_default_and_configured_import_folders_inside_a_library() {
        let directory = tempfile::tempdir().unwrap();
        let data = directory.path().join("data");
        let library = directory.path().join("library");
        let configured = library.join("Downloads").join("Models");
        std::fs::create_dir_all(&configured).unwrap();
        let state = AppState::initialize(&data).await.unwrap();
        sqlx::query("INSERT INTO library_roots (id, path, display_name, status, created_at, updated_at) VALUES ('root', ?, 'Library', 'online', ?, ?)")
            .bind(library.to_string_lossy().as_ref())
            .bind(now())
            .bind(now())
            .execute(&state.pool)
            .await
            .unwrap();
        let (_, _, destination, relative) = resolve_import_directory(&state).await.unwrap();
        assert_eq!(
            destination,
            std::fs::canonicalize(&library).unwrap().join("Web Imports")
        );
        assert_eq!(relative, PathBuf::from("Web Imports"));
        sqlx::query("INSERT INTO user_overrides (id, entity_type, entity_id, key, value_json, updated_at) VALUES ('preference', 'app', 'global', 'web_import_folder', ?, ?)")
            .bind(serde_json::to_string(configured.to_string_lossy().as_ref()).unwrap())
            .bind(now())
            .execute(&state.pool)
            .await
            .unwrap();
        let (_, _, destination, relative) = resolve_import_directory(&state).await.unwrap();
        assert_eq!(destination, std::fs::canonicalize(&configured).unwrap());
        assert_eq!(relative, PathBuf::from("Downloads/Models"));
    }
}
