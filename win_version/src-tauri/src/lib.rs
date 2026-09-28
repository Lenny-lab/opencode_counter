//! Application wiring: Tauri commands, the two windows, and the tray.
//!
//! The two windows mirror the macOS build's two surfaces. The *popover* is the
//! 360x520 glanceable panel opened from the notification area; the *dashboard*
//! is the full 1180x800 window with the charts. Both are created hidden at
//! startup and are hidden rather than destroyed when closed, so reopening one
//! is instant and never re-runs the frontend bootstrap.

pub mod cache;
pub mod db;
pub mod stats;
mod tray_icon;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent,
};

use crate::db::DatabaseInfo;
use crate::stats::{Snapshot, TodayStat};

/// Event pushed to both windows when the database file changes underneath us.
const CHANGED_EVENT: &str = "stats://changed";
/// Tray menu item ids.
const MENU_DASHBOARD: &str = "dashboard";
const MENU_REFRESH: &str = "refresh";
const MENU_QUIT: &str = "quit";

/// How often the watcher compares database timestamps.
const WATCH_INTERVAL: Duration = Duration::from_secs(5);

/// State that outlives a single command.
#[derive(Default)]
pub struct AppState {
    /// The last pair of figures written to the tray, so a refresh that changed
    /// nothing does not churn the icon and tooltip.
    tray: Mutex<Option<(i64, f64)>>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Where the database is, and whether it is there at all.
///
/// Probing without opening keeps the "OpenCode is not installed here" case a
/// clean renderable state instead of an exception on every call.
#[tauri::command]
fn database_info() -> DatabaseInfo {
    db::probe()
}

/// Every aggregate for the selected window. `days: null` means all time.
#[tauri::command]
fn get_snapshot(days: Option<i64>) -> Result<Snapshot, String> {
    let (db, _info) = db::open_primary()?;
    stats::build_snapshot(&db, days)
}

/// Just today's figures, for the fast refresh path.
#[tauri::command]
fn get_today() -> Result<TodayStat, String> {
    let (db, _info) = db::open_primary()?;
    stats::build_today(&db)
}

#[tauri::command]
fn open_dashboard(app: AppHandle) -> Result<(), String> {
    reveal(&app, "dashboard")
}

#[tauri::command]
fn hide_popover(app: AppHandle) -> Result<(), String> {
    window(&app, "popover")?.hide().map_err(|e| e.to_string())
}

/// Hide the dashboard. Both windows are configured with `visible: false` and
/// are hidden rather than destroyed on close, so this is the only way the
/// frontend needs to dismiss one.
#[tauri::command]
fn hide_dashboard(app: AppHandle) -> Result<(), String> {
    window(&app, "dashboard")?.hide().map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window {label} is not available"))
}

/// Show a window and focus it, undoing the hidden state it may be in.
fn reveal(app: &AppHandle, label: &str) -> Result<(), String> {
    let target = window(app, label)?;
    if label == "popover" {
        anchor_to_tray(&target)?;
    }
    target.show().map_err(|e| e.to_string())?;
    target.set_focus().map_err(|e| e.to_string())
}

/// Park the popover in the bottom-right of the work area.
///
/// Windows puts the notification area in a corner but exposes no way to ask
/// where, so the primary monitor's bottom-right is the closest stable answer.
fn anchor_to_tray(target: &WebviewWindow) -> Result<(), String> {
    let size = target.outer_size().map_err(|e| e.to_string())?;
    let monitor = target
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| target.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let x = (work.position.x + work.size.width as i32 - size.width as i32 - 16).max(0);
        let y = (work.position.y + work.size.height as i32 - size.height as i32 - 16).max(0);
        target
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    } else {
        // No monitor information: fall back to centring rather than leaving
        // the panel wherever the config put it.
        let _ = target.center();
    }
    Ok(())
}

fn toggle_popover(app: &AppHandle) {
    let Ok(target) = window(app, "popover") else {
        return;
    };
    match target.is_visible() {
        Ok(true) => {
            let _ = target.hide();
        }
        _ => {
            let _ = reveal(app, "popover");
        }
    }
}

// ---------------------------------------------------------------------------
// Tray
// ---------------------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let dashboard = MenuItem::with_id(app, MENU_DASHBOARD, "Open dashboard", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, MENU_REFRESH, "Refresh", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&dashboard, &refresh, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        // Left click is the popover, matching the macOS menu bar item; the
        // menu belongs on right click.
        .show_menu_on_left_click(false)
        .tooltip("OpenCode Stats")
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_DASHBOARD => {
                let _ = reveal(app, "dashboard");
            }
            MENU_REFRESH => {
                refresh_tray(app);
                let _ = app.emit(CHANGED_EVENT, ());
            }
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

/// Recompute today's figures and repaint the tray, skipping the update when
/// nothing moved.
fn refresh_tray(app: &AppHandle) {
    let today = match db::open_primary() {
        Ok((db, _)) => stats::build_today(&db),
        Err(_) => Ok(TodayStat::default()),
    };
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let today = match today {
        Ok(today) => today,
        Err(err) => {
            eprintln!("[stats] tray refresh failed: {err}");
            TodayStat::default()
        }
    };
    let key = (today.tokens.total, today.cost);
    {
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        let mut slot = state.tray.lock().unwrap_or_else(|e| e.into_inner());
        if *slot == Some(key) {
            return;
        }
        *slot = Some(key);
    }
    let _ = tray.set_icon(Some(tray_icon::render(&tray_icon::token_label(today.tokens.total))));
    let _ = tray.set_tooltip(Some(tray_icon::tooltip(today.tokens.total, today.cost)));
}

// ---------------------------------------------------------------------------
// Database watcher
// ---------------------------------------------------------------------------

/// A cheap fingerprint of "the database is the same file at the same instant".
///
/// OpenCode writes through a WAL, so the interesting signal is on
/// `opencode.db-wal` as much as on the main file. Polling the modification
/// time keeps this dependency-free and behaves identically on every platform,
/// which matters more here than sub-second latency on a panel that is
/// refreshed on hover anyway.
fn fingerprint() -> Option<(PathBuf, SystemTime)> {
    let paths = db::discover_databases().ok()?;
    let primary = paths.first()?.clone();
    Some((primary.clone(), db::activity(&primary)))
}

fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = fingerprint();
        loop {
            std::thread::sleep(WATCH_INTERVAL);
            let current = fingerprint();
            if current == last {
                continue;
            }
            last = current;
            refresh_tray(&app);
            // Both windows re-run their own commands; neither caches across
            // events, so this is all that is needed to stay current.
            if let Err(err) = app.emit(CHANGED_EVENT, ()) {
                eprintln!("[stats] could not notify windows: {err}");
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            database_info,
            get_snapshot,
            get_today,
            open_dashboard,
            hide_popover,
            hide_dashboard,
            quit
        ])
        .on_window_event(|window, event| match event {
            // Neither window is ever destroyed. Closing the dashboard leaves it
            // hidden so the tray can bring it straight back; the popover
            // additionally disappears when it loses focus, the way a real
            // popover does.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) if window.label() == "popover" => {
                let _ = window.hide();
            }
            _ => {}
        })
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;
            refresh_tray(&handle);
            spawn_watcher(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start OpenCode Stats");
}
