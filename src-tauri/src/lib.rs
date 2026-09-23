mod backups;
mod commands;
mod domain;
mod indexing;
mod parsers;
mod previews;
mod state;
mod web_sources;

use sqlx::Row;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            backups::apply_pending_restore(&data_dir)?;
            backups::cleanup_stale_work(&data_dir);
            let state = tauri::async_runtime::block_on(AppState::initialize(&data_dir))?;
            tauri::async_runtime::block_on(backups::reconcile_backup_credentials(&state))?;
            tauri::async_runtime::block_on(seed_defaults(&state))?;
            app.manage(state);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                if let Ok(rows) =
                    sqlx::query("SELECT id FROM library_roots WHERE status != 'removed'")
                        .fetch_all(&state.pool)
                        .await
                {
                    for row in rows {
                        let id: String = row.get("id");
                        let _ = indexing::start(id, &state, handle.clone()).await;
                    }
                }
            });
            backups::start_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_roots,
            commands::add_library_root,
            commands::remove_library_root,
            commands::reconnect_library_root,
            commands::start_scan,
            commands::pause_scan,
            commands::get_scan_status,
            commands::list_folders,
            commands::list_models,
            commands::get_model,
            commands::toggle_favorite,
            commands::set_favorite,
            commands::save_notes,
            commands::list_tags,
            commands::save_tag,
            commands::delete_tag,
            commands::set_model_tags,
            commands::add_models_to_tag,
            commands::list_related_models,
            commands::get_duplicate_stats,
            commands::list_duplicate_groups,
            commands::list_collections,
            commands::create_collection,
            commands::update_collection,
            commands::delete_collection,
            commands::add_models_to_collection,
            commands::remove_models_from_collection,
            commands::list_materials,
            commands::save_material,
            commands::calculate_cost,
            commands::save_cost_estimate,
            commands::get_preview_payload,
            commands::get_3mf_plate_thumbnail,
            commands::get_viewer_mesh,
            commands::get_cached_thumbnail,
            commands::request_thumbnail,
            commands::get_preferences,
            commands::list_slicer_apps,
            commands::save_preference,
            commands::open_asset,
            commands::open_external_url,
            commands::reveal_asset,
            commands::export_metadata,
            commands::export_diagnostics,
            backups::list_backup_destinations,
            backups::save_backup_destination,
            backups::delete_backup_destination,
            backups::test_backup_destination,
            backups::run_backup,
            backups::list_backup_runs,
            backups::list_destination_archives,
            backups::prepare_restore_from_path,
            backups::prepare_restore_from_destination,
            backups::commit_prepared_restore,
            backups::cancel_prepared_restore,
            web_sources::preview_web_source,
            web_sources::list_web_sources,
            web_sources::save_web_source,
            web_sources::delete_web_source,
            web_sources::attach_web_source_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Volum");
}

async fn seed_defaults(state: &AppState) -> Result<(), String> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM materials")
        .fetch_one(&state.pool)
        .await
        .map_err(|error| error.to_string())?;
    if count == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO materials (id, name, material_type, color_name, color_hex, spool_price_minor, currency, spool_weight_g, density, created_at, updated_at) VALUES (?, 'Generic PLA', 'PLA', 'Natural', '#e9e2d3', 1599, 'EUR', 1000, 1.24, ?, ?)")
            .bind(id).bind(&now).bind(&now).execute(&state.pool).await.map_err(|error| error.to_string())?;
    }
    Ok(())
}
