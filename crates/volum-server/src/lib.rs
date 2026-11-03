pub mod commands;
pub mod companion;
pub mod credentials;
pub mod domain;
pub mod events;
pub mod geometry;
pub mod indexing;
pub mod library_migration;
pub mod parsers;
pub mod previews;
pub mod project_docs;
pub mod project_packages;
pub mod project_storage;
pub mod recovery;
pub mod remote;
pub mod state;
pub mod web_admin;
pub mod web_sources;

use sqlx::Row;
use state::AppState;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

/// Runs Volum's canonical library service without a desktop window.
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
    let photo_uploads = env_flag("VOLUM_PHOTO_UPLOADS", true)?;
    let web_enabled = env_flag("VOLUM_WEB_ENABLED", true)?;

    recovery::apply_pending_restore(&data_dir)?;
    library_migration::apply_pending_activation(&data_dir, &library_dir)?;
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

    let loaded_web_admin = if web_enabled {
        Some(web_admin::WebAdminState::load(&data_dir)?)
    } else {
        None
    };
    let web_dir = if web_enabled {
        let directory = std::env::var_os("VOLUM_WEB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/share/volum/web"));
        if directory.join("index.html").is_file() {
            Some(directory)
        } else {
            log::warn!(
                "Volum web assets were not found at {}; the API will remain available",
                directory.display()
            );
            None
        }
    } else {
        None
    };

    let controller = companion::CompanionController::new(&data_dir, state.clone(), None)?;
    if loaded_web_admin
        .as_ref()
        .is_some_and(|loaded| loaded.password_changed)
    {
        controller.revoke_web_admin_devices().await?;
    }
    controller
        .start_headless(companion::HeadlessServerOptions {
            bind,
            public_url,
            organization_writes,
            photo_uploads,
            web_admin: loaded_web_admin.map(|loaded| loaded.state),
            web_dir,
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
    println!(
        "volumd listening at {}",
        status.base_url.as_deref().unwrap_or("unknown URL")
    );
    if env_flag("VOLUM_PRINT_PAIRING", false)? {
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

pub async fn seed_defaults(state: &AppState) -> Result<(), String> {
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
