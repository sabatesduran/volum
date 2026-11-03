use sqlx::Row;
use std::sync::Arc;
use tauri::{Emitter, Manager};
mod backups;
mod commands;
mod companion_commands;
mod credentials;
mod library_migration;
mod menu_bar;
mod project_docs_commands;
mod project_packages;
mod project_storage;
mod remote;
mod web_sources;
use volum_server::{
    companion, events::EventEmitter, indexing, project_storage as project_storage_core, recovery,
    remote as remote_core, state::AppState,
};

fn event_emitter(app: tauri::AppHandle) -> EventEmitter {
    EventEmitter::new(move |event, payload| {
        let _ = app.emit(event, payload);
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
            recovery::apply_pending_restore(&data_dir)?;
            let local_retired = remote_core::local_library_retired(&data_dir);
            let state = tauri::async_runtime::block_on(AppState::initialize(&data_dir))?;
            if !local_retired {
                tauri::async_runtime::block_on(volum_server::seed_defaults(&state))?;
            }
            let companion = companion::CompanionController::new(
                &data_dir,
                state.clone(),
                Some(event_emitter(app.handle().clone())),
            )?;
            let remote = remote_core::RemoteController::new(
                &data_dir,
                Arc::new(credentials::KeyringCredentialStore::new(
                    "dev.didac.volum.remote",
                )),
            )?;
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
            if !local_retired {
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
                        match project_storage_core::import_managed_inbox_impl(&state).await {
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
                            let _ = indexing::start(id, &state, Some(event_emitter(handle.clone()))).await;
                        }
                    }
                });
                backups::start_scheduler(app.handle().clone());
            }
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
            commands::clear_preview_cache,
            commands::get_preferences,
            commands::list_slicer_apps,
            commands::save_preference,
            menu_bar::set_menu_bar_enabled,
            commands::open_asset,
            commands::open_external_url,
            commands::reveal_asset,
            commands::export_metadata,
            commands::export_diagnostics,
             project_docs_commands::get_project_documentation,
             project_docs_commands::add_project_photo,
             project_docs_commands::update_project_photo,
             project_docs_commands::delete_project_photo,
             project_docs_commands::get_project_photo,
             project_docs_commands::list_custom_field_definitions,
             project_docs_commands::save_custom_field_definition,
             project_docs_commands::delete_custom_field_definition,
             project_docs_commands::set_project_custom_value,
             project_docs_commands::save_project_bom_item,
             project_docs_commands::delete_project_bom_item,
             project_docs_commands::save_assembly_step,
             project_docs_commands::delete_assembly_step,
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
            companion_commands::get_companion_status,
            companion_commands::set_companion_enabled,
            companion_commands::set_companion_organization_writes,
            companion_commands::set_companion_photo_uploads,
            companion_commands::set_companion_network,
            companion_commands::create_companion_pairing,
            companion_commands::revoke_companion_device,
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
            remote::get_remote_mirror_settings,
            remote::set_remote_mirror_mode,
            remote::set_remote_mirror_selection,
            remote::sync_remote_mirror,
            remote::open_remote_asset,
            remote::reveal_remote_asset,
            remote::list_mirrored_assets,
            remote::list_mirrored_projects,
            library_migration::inspect_library_migration,
            library_migration::get_pending_library_migration,
            library_migration::start_library_migration,
            library_migration::resume_library_migration,
            library_migration::abandon_library_migration,
            library_migration::rollback_library_migration,
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
