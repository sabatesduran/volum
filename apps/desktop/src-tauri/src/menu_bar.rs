use chrono::{DateTime, Duration as ChronoDuration, Utc};
use sqlx::Row;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, State,
};
use uuid::Uuid;
use volum_server::{companion::CompanionStatus, indexing, state::AppState};

use crate::backups;

const PREFERENCE_KEY: &str = "menu_bar_enabled";
const TRAY_ID: &str = "volum-menu-bar";
const OPEN_ID: &str = "volum-menu-bar-open";
const SCAN_ID: &str = "volum-menu-bar-scan";
const BACKUP_ID: &str = "volum-menu-bar-backup";
const QUIT_ID: &str = "volum-menu-bar-quit";
const REFRESH_SECONDS: u64 = 15;

#[derive(Clone)]
struct MenuBarItems {
    menu: Menu<tauri::Wry>,
    projects: MenuItem<tauri::Wry>,
    library: MenuItem<tauri::Wry>,
    companion: MenuItem<tauri::Wry>,
    backup: MenuItem<tauri::Wry>,
    scan_action: MenuItem<tauri::Wry>,
    backup_action: MenuItem<tauri::Wry>,
    backup_visible: Arc<AtomicBool>,
}

pub struct MenuBarState {
    enabled: AtomicBool,
    generation: AtomicU64,
    items: Mutex<Option<MenuBarItems>>,
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            items: Mutex::new(None),
        }
    }
}

impl MenuBarState {
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn set_items(&self, items: Option<MenuBarItems>) {
        if let Ok(mut current) = self.items.lock() {
            *current = items;
        }
    }

    fn current_items(&self) -> Option<MenuBarItems> {
        self.items.lock().ok().and_then(|items| items.clone())
    }

    fn mark_scan_busy(&self) {
        if let Some(items) = self.current_items() {
            let _ = items.scan_action.set_text("Scanning libraries…");
            let _ = items.scan_action.set_enabled(false);
        }
    }

    fn mark_backup_busy(&self) {
        if let Some(items) = self.current_items() {
            let _ = items.backup.set_text("Backups: Running…");
            let _ = items.backup_action.set_text("Backing up…");
            let _ = items.backup_action.set_enabled(false);
        }
    }

    fn apply(&self, snapshot: &MenuBarSnapshot) {
        let Some(items) = self.current_items() else {
            return;
        };
        let _ = items.projects.set_text(&snapshot.projects);
        let _ = items.library.set_text(&snapshot.library);
        let _ = items.companion.set_text(&snapshot.companion);
        let _ = items.scan_action.set_text(&snapshot.scan_action);
        let _ = items.scan_action.set_enabled(snapshot.scan_enabled);
        let _ = items.backup.set_text(&snapshot.backup);
        let _ = items.backup_action.set_text(&snapshot.backup_action);
        let _ = items.backup_action.set_enabled(snapshot.backup_enabled);

        if snapshot.show_backup != items.backup_visible.load(Ordering::SeqCst) {
            let changed = if snapshot.show_backup {
                items
                    .menu
                    .insert(&items.backup, 3)
                    .and_then(|_| items.menu.insert(&items.backup_action, 7))
            } else {
                items
                    .menu
                    .remove(&items.backup)
                    .and_then(|_| items.menu.remove(&items.backup_action))
            };
            if let Err(error) = changed {
                log::warn!("Could not update backup items in the Volum menu bar: {error}");
            } else {
                items
                    .backup_visible
                    .store(snapshot.show_backup, Ordering::SeqCst);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuBarSnapshot {
    projects: String,
    library: String,
    companion: String,
    backup: String,
    scan_action: String,
    backup_action: String,
    scan_enabled: bool,
    backup_enabled: bool,
    show_backup: bool,
    tooltip: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BackupMenuStatus {
    total: i64,
    enabled: i64,
    errors: i64,
    running: bool,
    last_success_at: Option<String>,
}

pub async fn load_enabled(state: &AppState) -> Result<bool, String> {
    let stored: Option<String> = sqlx::query_scalar("SELECT value_json FROM user_overrides WHERE entity_type = 'app' AND entity_id = 'global' AND key = ?")
        .bind(PREFERENCE_KEY)
        .fetch_optional(&state.pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(stored
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .is_some_and(|value| value == "true"))
}

pub async fn persist_enabled(state: &AppState, enabled: bool) -> Result<(), String> {
    let value = enabled.to_string();
    sqlx::query("INSERT INTO user_overrides (id, entity_type, entity_id, key, value_json, updated_at) VALUES (?, 'app', 'global', ?, ?, ?) ON CONFLICT(entity_type, entity_id, key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(Uuid::new_v4().to_string())
        .bind(PREFERENCE_KEY)
        .bind(serde_json::to_string(&value).map_err(|error| error.to_string())?)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn install(app: &AppHandle) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_visible(true).map_err(|error| error.to_string())?;
        return Ok(());
    }

    let projects = MenuItem::with_id(
        app,
        "volum-menu-bar-projects",
        "Loading library…",
        false,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let library = MenuItem::with_id(
        app,
        "volum-menu-bar-library",
        "Library: Checking…",
        false,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let companion = MenuItem::with_id(
        app,
        "volum-menu-bar-companion",
        "Mobile access: Checking…",
        false,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let backup = MenuItem::with_id(
        app,
        "volum-menu-bar-backup-status",
        "Backups: Checking…",
        false,
        None::<&str>,
    )
    .map_err(|error| error.to_string())?;
    let info_separator = PredefinedMenuItem::separator(app).map_err(|error| error.to_string())?;
    let open = MenuItem::with_id(app, OPEN_ID, "Open Volum", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let scan_action = MenuItem::with_id(app, SCAN_ID, "Scan libraries now", false, None::<&str>)
        .map_err(|error| error.to_string())?;
    let backup_action = MenuItem::with_id(app, BACKUP_ID, "Back up now", false, None::<&str>)
        .map_err(|error| error.to_string())?;
    let action_separator = PredefinedMenuItem::separator(app).map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit Volum", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let menu = Menu::with_items(
        app,
        &[
            &projects,
            &library,
            &companion,
            &backup,
            &info_separator,
            &open,
            &scan_action,
            &backup_action,
            &action_separator,
            &quit,
        ],
    )
    .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../icons/tray-template.png").as_slice();
    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../../src/assets/volum-icon-transparent.png").as_slice();
    let icon = Image::from_bytes(icon_bytes).map_err(|error| error.to_string())?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Volum")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_ID => {
                if let Err(error) = show_main_window(app) {
                    log::warn!("Could not show Volum from the menu bar: {error}");
                }
            }
            SCAN_ID => scan_libraries(app),
            BACKUP_ID => run_backup(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)
        .map_err(|error| error.to_string())?;
    app.state::<MenuBarState>().set_items(Some(MenuBarItems {
        menu,
        projects,
        library,
        companion,
        backup,
        scan_action,
        backup_action,
        backup_visible: Arc::new(AtomicBool::new(true)),
    }));
    Ok(())
}

fn hide(app: &AppHandle) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_visible(false).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn enable_at_startup(app: &AppHandle) -> Result<(), String> {
    install(app)?;
    app.state::<MenuBarState>().set_enabled(true);
    start_refresh_loop(app);
    Ok(())
}

fn start_refresh_loop(app: &AppHandle) {
    let generation = app.state::<MenuBarState>().generation();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let should_refresh = {
                let current = app.state::<MenuBarState>();
                current.is_enabled() && current.generation() == generation
            };
            if !should_refresh {
                break;
            }
            if let Err(error) = refresh(&app).await {
                log::warn!("Could not refresh the Volum menu bar: {error}");
            }
            tokio::time::sleep(Duration::from_secs(REFRESH_SECONDS)).await;
        }
    });
}

async fn refresh(app: &AppHandle) -> Result<(), String> {
    let snapshot = load_snapshot(app).await?;
    app.state::<MenuBarState>().apply(&snapshot);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_tooltip(Some(&snapshot.tooltip))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn load_snapshot(app: &AppHandle) -> Result<MenuBarSnapshot, String> {
    let state = app.state::<AppState>();
    let companion = app.state::<crate::companion::CompanionController>();
    let project_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE trashed_at IS NULL")
            .fetch_one(&state.pool);
    let roots = sqlx::query("SELECT COUNT(*) total, COALESCE(SUM(CASE WHEN status IN ('offline', 'error') THEN 1 ELSE 0 END), 0) issues FROM library_roots WHERE status != 'removed'")
        .fetch_one(&state.pool);
    let backups = sqlx::query("SELECT COUNT(*) total, COALESCE(SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END), 0) enabled, COALESCE(SUM(CASE WHEN enabled = 1 AND last_error IS NOT NULL THEN 1 ELSE 0 END), 0) errors, MAX(CASE WHEN enabled = 1 THEN last_success_at END) last_success_at FROM backup_destinations")
        .fetch_one(&state.pool);
    let backup_running = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM backup_runs WHERE status = 'running')",
    )
    .fetch_one(&state.pool);
    let companion_status = companion.status();
    let (project_count, roots, backups, backup_running, companion_status) = tokio::join!(
        project_count,
        roots,
        backups,
        backup_running,
        companion_status
    );
    let project_count = project_count.map_err(|error| error.to_string())?;
    let roots = roots.map_err(|error| error.to_string())?;
    let backups = backups.map_err(|error| error.to_string())?;
    let backup_status = BackupMenuStatus {
        total: backups.get("total"),
        enabled: backups.get("enabled"),
        errors: backups.get("errors"),
        running: backup_running.map_err(|error| error.to_string())?,
        last_success_at: backups.get("last_success_at"),
    };
    let scans = state
        .scans
        .read()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let root_count = roots.get("total");
    let root_issues = roots.get("issues");
    let scanning = scans.iter().any(|status| status.state == "scanning");
    let library = format_library_status(&scans, root_count, root_issues);
    let companion_text = format_companion_status(&companion_status, Utc::now());
    let (backup, backup_action, backup_enabled) = format_backup_status(&backup_status, Utc::now());
    let projects = project_label(project_count);
    let mobile_tooltip = if companion_status.running {
        "mobile access online"
    } else {
        "mobile access offline"
    };
    Ok(MenuBarSnapshot {
        projects: projects.clone(),
        library,
        companion: companion_text,
        backup,
        scan_action: if scanning {
            "Scanning libraries…".into()
        } else {
            "Scan libraries now".into()
        },
        backup_action,
        scan_enabled: root_count > 0 && !scanning,
        backup_enabled,
        show_backup: backup_status.total > 0,
        tooltip: format!("Volum · {projects} · {mobile_tooltip}"),
    })
}

fn scan_libraries(app: &AppHandle) {
    app.state::<MenuBarState>().mark_scan_busy();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let state = app.state::<AppState>();
            let root_ids = sqlx::query_scalar::<_, String>(
                "SELECT id FROM library_roots WHERE status != 'removed' ORDER BY created_at",
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|error| error.to_string())?;
            for root_id in root_ids {
                indexing::start(root_id, &state, Some(crate::event_emitter(app.clone()))).await?;
            }
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = result {
            log::warn!("Could not scan libraries from the Volum menu bar: {error}");
        }
        if let Err(error) = refresh(&app).await {
            log::warn!("Could not refresh the Volum menu bar after scanning: {error}");
        }
    });
}

fn run_backup(app: &AppHandle) {
    app.state::<MenuBarState>().mark_backup_busy();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = backups::run_backup(None, state, app.clone()).await {
            log::warn!("Could not run a backup from the Volum menu bar: {error}");
        }
        if let Err(error) = refresh(&app).await {
            log::warn!("Could not refresh the Volum menu bar after backup: {error}");
        }
    });
}

fn project_label(count: i64) -> String {
    format!(
        "{count} {}",
        if count == 1 { "project" } else { "projects" }
    )
}

fn format_library_status(
    scans: &[volum_server::domain::ScanStatus],
    root_count: i64,
    root_issues: i64,
) -> String {
    let active = scans
        .iter()
        .filter(|status| status.state == "scanning")
        .collect::<Vec<_>>();
    if !active.is_empty() {
        let discovered = active.iter().map(|status| status.discovered).sum::<u64>();
        let processed = active.iter().map(|status| status.processed).sum::<u64>();
        let errors = active.iter().map(|status| status.errors).sum::<u64>();
        let progress = if discovered > 0 {
            format!("Library: Scanning {processed} of {discovered}")
        } else {
            "Library: Scanning…".into()
        };
        return if errors > 0 {
            format!(
                "{progress} · {errors} {}",
                if errors == 1 { "error" } else { "errors" }
            )
        } else {
            progress
        };
    }
    if scans.iter().any(|status| status.state == "paused") {
        return "Library: Scan paused".into();
    }
    if root_issues > 0 {
        return if root_issues == 1 {
            "Library: 1 folder needs attention".into()
        } else {
            format!("Library: {root_issues} folders need attention")
        };
    }
    if root_count == 0 {
        "Library: Not configured".into()
    } else {
        "Library: Up to date".into()
    }
}

fn format_companion_status(status: &CompanionStatus, now: DateTime<Utc>) -> String {
    if status.running {
        let active = status
            .devices
            .iter()
            .filter(|device| {
                device
                    .last_seen_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .is_some_and(|seen| {
                        now.signed_duration_since(seen.with_timezone(&Utc))
                            <= ChronoDuration::minutes(2)
                    })
            })
            .count();
        let paired = status.devices.len();
        return match (active, paired) {
            (0, 0) => "Mobile access: Online · No paired devices".into(),
            (0, paired) => format!("Mobile access: Online · {paired} paired"),
            (active, paired) => {
                format!("Mobile access: Online · {active} active / {paired} paired")
            }
        };
    }
    if !status.enabled {
        return "Mobile access: Off".into();
    }
    if status.last_error.is_none() {
        return "Mobile access: Starting…".into();
    }
    if status
        .last_error
        .as_deref()
        .is_some_and(|error| error.to_ascii_lowercase().contains("tailscale"))
    {
        "Mobile access: Tailscale unavailable".into()
    } else {
        "Mobile access: Needs attention".into()
    }
}

fn format_backup_status(status: &BackupMenuStatus, now: DateTime<Utc>) -> (String, String, bool) {
    if status.running {
        return ("Backups: Running…".into(), "Backing up…".into(), false);
    }
    if status.total == 0 {
        return (
            "Backups: Not configured".into(),
            "Back up now".into(),
            false,
        );
    }
    if status.enabled == 0 {
        return ("Backups: Disabled".into(), "Back up now".into(), false);
    }
    if status.errors > 0 {
        return (
            "Backups: Needs attention".into(),
            "Back up now".into(),
            status.enabled > 0,
        );
    }
    let summary = status
        .last_success_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| {
            format!(
                "Last backup: {}",
                relative_time(value.with_timezone(&Utc), now)
            )
        })
        .unwrap_or_else(|| "Backups: Never run".into());
    (summary, "Back up now".into(), true)
}

fn relative_time(value: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let elapsed = now.signed_duration_since(value);
    if elapsed < ChronoDuration::minutes(1) {
        "just now".into()
    } else if elapsed < ChronoDuration::hours(1) {
        format!("{}m ago", elapsed.num_minutes())
    } else if elapsed < ChronoDuration::days(1) {
        format!("{}h ago", elapsed.num_hours())
    } else if elapsed < ChronoDuration::days(2) {
        "yesterday".into()
    } else {
        format!("{}d ago", elapsed.num_days())
    }
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    app.show().map_err(|error| error.to_string())?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The main Volum window is unavailable".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_menu_bar_enabled(
    enabled: bool,
    app: AppHandle,
    library: State<'_, AppState>,
    menu_bar: State<'_, MenuBarState>,
) -> Result<bool, String> {
    if enabled {
        install(&app)?;
        if let Err(error) = persist_enabled(&library, true).await {
            if let Err(hide_error) = hide(&app) {
                log::error!(
                    "Could not hide the Volum menu bar after a preference error: {hide_error}"
                );
            }
            return Err(error);
        }
        menu_bar.set_enabled(true);
        start_refresh_loop(&app);
    } else {
        let was_enabled = menu_bar.is_enabled();
        menu_bar.set_enabled(false);
        if let Err(error) = hide(&app) {
            menu_bar.set_enabled(was_enabled);
            return Err(error);
        }
        if let Err(error) = persist_enabled(&library, false).await {
            if was_enabled {
                if let Err(restore_error) = install(&app) {
                    log::error!("Could not restore the Volum menu bar after a preference error: {restore_error}");
                } else {
                    menu_bar.set_enabled(true);
                    start_refresh_loop(&app);
                }
            }
            return Err(error);
        }
    }
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use volum_server::{companion::CompanionDevice, domain::ScanStatus};

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[tokio::test]
    async fn menu_bar_preference_defaults_off_and_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path()).await.unwrap();

        assert!(!load_enabled(&state).await.unwrap());
        persist_enabled(&state, true).await.unwrap();
        assert!(load_enabled(&state).await.unwrap());
        persist_enabled(&state, false).await.unwrap();
        assert!(!load_enabled(&state).await.unwrap());
    }

    #[test]
    fn menu_bar_summarizes_library_progress_and_problems() {
        let scan = ScanStatus {
            root_id: "root".into(),
            state: "scanning".into(),
            discovered: 100,
            processed: 35,
            errors: 1,
            current_path: None,
            message: None,
        };
        assert_eq!(
            format_library_status(&[scan], 1, 0),
            "Library: Scanning 35 of 100 · 1 error"
        );
        assert_eq!(
            format_library_status(&[], 2, 1),
            "Library: 1 folder needs attention"
        );
        assert_eq!(format_library_status(&[], 1, 0), "Library: Up to date");
    }

    #[test]
    fn menu_bar_distinguishes_active_and_paired_companions() {
        let now = timestamp("2026-10-29T12:00:00Z");
        let status = CompanionStatus {
            enabled: true,
            running: true,
            server_id: "server".into(),
            tailnet_ip: Some("100.64.0.1".into()),
            dns_name: None,
            base_url: Some("http://100.64.0.1:43821".into()),
            port: 43821,
            devices: vec![
                CompanionDevice {
                    id: "phone".into(),
                    name: "Phone".into(),
                    platform: "iOS".into(),
                    paired_at: "2026-10-20T12:00:00Z".into(),
                    last_seen_at: Some("2026-10-29T11:59:00Z".into()),
                    role: "client".into(),
                },
                CompanionDevice {
                    id: "tablet".into(),
                    name: "Tablet".into(),
                    platform: "iOS".into(),
                    paired_at: "2026-10-20T12:00:00Z".into(),
                    last_seen_at: Some("2026-10-29T10:00:00Z".into()),
                    role: "client".into(),
                },
            ],
            organization_writes: false,
            photo_uploads: false,
            network_mode: "automatic".into(),
            bind_address: None,
            public_url: None,
            transport: Some("tailscale-http".into()),
            last_error: None,
        };
        assert_eq!(
            format_companion_status(&status, now),
            "Mobile access: Online · 1 active / 2 paired"
        );

        let unavailable = CompanionStatus {
            running: false,
            last_error: Some("Tailscale is not connected".into()),
            devices: Vec::new(),
            ..status
        };
        assert_eq!(
            format_companion_status(&unavailable, now),
            "Mobile access: Tailscale unavailable"
        );

        let starting = CompanionStatus {
            last_error: None,
            ..unavailable
        };
        assert_eq!(
            format_companion_status(&starting, now),
            "Mobile access: Starting…"
        );
    }

    #[test]
    fn menu_bar_surfaces_backup_recency_and_failures() {
        let now = timestamp("2026-10-29T12:00:00Z");
        let healthy = BackupMenuStatus {
            total: 1,
            enabled: 1,
            errors: 0,
            running: false,
            last_success_at: Some("2026-10-29T10:00:00Z".into()),
        };
        assert_eq!(
            format_backup_status(&healthy, now),
            ("Last backup: 2h ago".into(), "Back up now".into(), true)
        );

        let failed = BackupMenuStatus {
            errors: 1,
            ..healthy
        };
        assert_eq!(
            format_backup_status(&failed, now),
            (
                "Backups: Needs attention".into(),
                "Back up now".into(),
                true
            )
        );
    }
}
