mod backups;
mod commands;
mod companion;
mod domain;
mod geometry;
mod indexing;
mod menu_bar;
mod parsers;
mod previews;
mod project_docs;
mod project_packages;
mod project_storage;
mod remote;
mod state;
mod web_sources;

use sqlx::Row;
use state::AppState;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
use tauri::{Emitter, Manager};

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
            if let Err(error) = menu_bar::show_main_window(app) {
                log::warn!("Could not show the existing Volum window: {error}");
            }
        }))
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let menu_bar = window
                    .app_handle()
                    .try_state::<menu_bar::MenuBarState>();
                if menu_bar.is_some_and(|state| state.is_enabled()) {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        log::warn!("Could not hide Volum in the menu bar: {error}");
                    }
                }
            }
        })
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            backups::apply_pending_restore(&data_dir)?;
            let state = tauri::async_runtime::block_on(AppState::initialize(&data_dir))?;
            tauri::async_runtime::block_on(seed_defaults(&state))?;
            let companion = companion::CompanionController::new(
                &data_dir,
                state.clone(),
                Some(app.handle().clone()),
            )?;
            let remote = remote::RemoteController::new(&data_dir)?;
            let remote_sync = remote.clone();
            app.manage(state);
            app.manage(menu_bar::MenuBarState::default());
            app.manage(companion.clone());
            app.manage(remote);
            let menu_bar_requested = {
                let state = app.state::<AppState>();
                tauri::async_runtime::block_on(menu_bar::load_enabled(&state))?
            };
            if menu_bar_requested {
                if let Err(error) = menu_bar::enable_at_startup(app.handle()) {
                    log::warn!("Could not enable the Volum menu bar: {error}");
                    let state = app.state::<AppState>();
                    tauri::async_runtime::block_on(menu_bar::persist_enabled(&state, false))?;
                }
            }
            tauri::async_runtime::spawn(remote_sync.run_background_sync());
            tauri::async_runtime::spawn(async move {
                companion.start_if_enabled().await;
            });
            let maintenance_handle = app.handle().clone();
            let maintenance_data_dir = data_dir.clone();
            tauri::async_runtime::spawn(async move {
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    backups::cleanup_stale_work(&maintenance_data_dir);
                })
                .await;
                let state = maintenance_handle.state::<AppState>();
                if let Err(error) = backups::reconcile_backup_credentials(&state).await {
                    log::warn!("Could not reconcile backup credentials: {error}");
                }
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let has_managed_root: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM library_roots WHERE root_kind = 'managed' AND status != 'removed')")
                    .fetch_one(&state.pool)
                    .await
                    .unwrap_or(false);
                if has_managed_root {
                    match project_storage::import_managed_inbox_impl(&state).await {
                        Ok(result) if !result.project_ids.is_empty() => {
                            let _ = handle.emit("models-changed", ());
                        }
                        Ok(_) => {}
                        Err(error) => log::warn!("Could not import the managed Inbox: {error}"),
                    }
                }
                if let Ok(rows) =
                    sqlx::query("SELECT id FROM library_roots WHERE status != 'removed'")
                        .fetch_all(&state.pool)
                        .await
                {
                    for row in rows {
                        let id: String = row.get("id");
                        let _ = indexing::start(id, &state, Some(handle.clone())).await;
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
            commands::list_saved_searches,
            commands::save_saved_search,
            commands::delete_saved_search,
            commands::list_tags,
            commands::save_tag,
            commands::delete_tag,
            commands::set_model_tags,
            commands::add_models_to_tag,
            commands::list_related_models,
            commands::get_duplicate_stats,
            commands::list_duplicate_groups,
            commands::merge_projects,
            commands::split_project,
            commands::set_project_primary_asset,
            commands::dismiss_duplicate_match,
            commands::cleanup_duplicate_group,
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
            commands::cancel_thumbnail_request,
            commands::get_preferences,
            commands::list_slicer_apps,
            commands::save_preference,
            menu_bar::set_menu_bar_enabled,
            commands::open_asset,
            commands::open_external_url,
            commands::reveal_asset,
            commands::export_metadata,
            commands::export_diagnostics,
            project_docs::get_project_documentation,
            project_docs::add_project_photo,
            project_docs::update_project_photo,
            project_docs::delete_project_photo,
            project_docs::get_project_photo,
            project_docs::list_custom_field_definitions,
            project_docs::save_custom_field_definition,
            project_docs::delete_custom_field_definition,
            project_docs::set_project_custom_value,
            project_docs::save_project_bom_item,
            project_docs::delete_project_bom_item,
            project_docs::save_assembly_step,
            project_docs::delete_assembly_step,
            project_packages::inspect_project_package,
            project_packages::export_project_package,
            project_packages::import_project_package,
            project_storage::get_managed_library_status,
            project_storage::configure_managed_library,
            project_storage::list_library_assets,
            project_storage::import_managed_inbox,
            project_storage::create_managed_project,
            project_storage::add_files_to_project,
            project_storage::rename_project,
            project_storage::rename_project_file,
            project_storage::preview_project_consolidation,
            project_storage::convert_project_to_managed,
            project_storage::trash_project,
            project_storage::restore_project_from_trash,
            project_storage::delete_project_permanently,
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
            companion::get_companion_status,
            companion::set_companion_enabled,
            companion::set_companion_organization_writes,
            companion::create_companion_pairing,
            companion::revoke_companion_device,
            remote::get_backend_connection,
            remote::connect_remote_server,
            remote::use_local_backend,
            remote::use_remote_backend,
            remote::forget_remote_server,
            remote::remote_json_request,
            remote::remote_binary_request,
            remote::remote_upload_file,
            remote::remote_upload_photo,
            remote::configure_remote_mirror,
            remote::sync_remote_mirror,
            remote::open_remote_asset,
            remote::reveal_remote_asset,
            remote::list_mirrored_assets,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Volum")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Err(error) = menu_bar::show_main_window(app) {
                    log::warn!("Could not reopen Volum: {error}");
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}

/// Runs Volum's canonical library service without a desktop window.
///
/// The headless process uses the same database, migrations, indexer, managed-library
/// implementation, authentication, and API router as Desktop host mode.
pub async fn run_headless() -> Result<(), String> {
    let data_dir = std::env::var_os("VOLUM_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/config"));
    let library_dir = std::env::var_os("VOLUM_LIBRARY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/library"));
    let bind = std::env::var("VOLUM_BIND").unwrap_or_else(|_| "0.0.0.0:43821".into());
    let bind: SocketAddr = bind.parse().map_err(|_| {
        "VOLUM_BIND must be an IP address and port, for example 0.0.0.0:43821".to_string()
    })?;
    let public_url = std::env::var("VOLUM_PUBLIC_URL").unwrap_or_else(|_| {
        let ip = match bind.ip() {
            IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            ip => ip,
        };
        match ip {
            IpAddr::V4(ip) => format!("http://{ip}:{}", bind.port()),
            IpAddr::V6(ip) => format!("http://[{ip}]:{}", bind.port()),
        }
    });
    let organization_writes = env_flag("VOLUM_ORGANIZATION_WRITES", true)?;

    backups::apply_pending_restore(&data_dir)?;
    let state = AppState::initialize(&data_dir).await?;
    seed_defaults(&state).await?;
    let has_managed_root: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM library_roots WHERE root_kind = 'managed' AND status != 'removed')",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|error| error.to_string())?;
    if !has_managed_root {
        project_storage::configure_managed_library_impl(
            library_dir.to_string_lossy().to_string(),
            &state,
            None,
        )
        .await?;
    }

    let controller = companion::CompanionController::new(&data_dir, state.clone(), None)?;
    controller
        .start_headless(companion::HeadlessServerOptions {
            bind,
            public_url,
            organization_writes,
        })
        .await?;

    if let Err(error) = project_storage::import_managed_inbox_impl(&state).await {
        log::warn!("Could not import the managed Inbox: {error}");
    }
    let roots = sqlx::query("SELECT id FROM library_roots WHERE status != 'removed'")
        .fetch_all(&state.pool)
        .await
        .map_err(|error| error.to_string())?;
    for row in roots {
        let id: String = row.get("id");
        if let Err(error) = indexing::start(id, &state, None).await {
            log::warn!("Could not start a library watcher: {error}");
        }
    }

    let status = controller.status().await;
    let print_pairing = env_flag("VOLUM_PRINT_PAIRING", status.devices.is_empty())?;
    println!(
        "volumd listening at {}",
        status.base_url.as_deref().unwrap_or("unknown URL")
    );
    if print_pairing {
        let pairing = controller.create_pairing().await?;
        println!("Pair a client within 5 minutes using this payload:");
        println!("{}", pairing.qr_payload);
    }
    wait_for_shutdown_signal().await?;
    controller.stop().await;
    state.pool.close().await;
    Ok(())
}

fn env_flag(name: &str, default: bool) -> Result<bool, String> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(default);
    };
    match value.to_string_lossy().trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("{name} must be true or false")),
    }
}

async fn wait_for_shutdown_signal() -> Result<(), String> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|error| format!("Could not install the shutdown handler: {error}"))?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(|error| error.to_string()),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|error| error.to_string())
    }
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
