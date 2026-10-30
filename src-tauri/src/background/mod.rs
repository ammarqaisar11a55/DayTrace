//! Everything that keeps running when the window is closed.

mod tray;

use std::thread;
use std::time::Duration;

use daytrace_core::database::{sessions, settings};
use daytrace_core::time::{now_ms, HOUR_MS};
use log::{info, warn};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::updates;
use crate::state::AppState;

pub use tray::{create as create_tray, update_status as update_tray};

const MAIN_WINDOW: &str = "main";
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(6 * 3_600);

/// Shows the window, creating it if needed. Closing the window destroys its
/// webview, which is what keeps background memory use low.
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::default())
        .title("DayTrace")
        .inner_size(1180.0, 780.0)
        .min_inner_size(860.0, 560.0)
        .build();
    if let Err(error) = built {
        warn!("could not open the window: {error}");
    }
}

/// Defaults to staying alive if settings cannot be read, so that a database
/// problem never silently stops tracking.
pub fn runs_in_background(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .database
        .read(settings::load)
        .map_or(true, |stored| stored.run_in_background)
}

pub fn quit(app: &AppHandle) {
    app.state::<AppState>().shutdown_tracker();
    app.exit(0);
}

fn apply_retention(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(stored) = state.database.read(settings::load) else { return };
    if stored.retention_days == 0 {
        return;
    }
    let cutoff = now_ms() - stored.retention_days as i64 * 24 * HOUR_MS;
    match state.database.write(|connection| sessions::delete_before(connection, cutoff)) {
        Ok(0) => {}
        Ok(count) => {
            info!("retention removed {count} sessions");
            let _ = app.emit("data-changed", ());
        }
        Err(error) => warn!("retention failed: {error}"),
    }
}

pub fn start_maintenance(app: AppHandle) {
    let spawned = thread::Builder::new().name("daytrace-maintenance".into()).spawn(move || loop {
        apply_retention(&app);
        thread::sleep(MAINTENANCE_INTERVAL);
    });
    if let Err(error) = spawned {
        warn!("could not start maintenance: {error}");
    }
}

const MIN_CHECK_SPACING_MS: i64 = HOUR_MS;

/// Whether a check is due. Startup checks are skipped if one ran recently, so
/// repeated restarts cannot pile up requests against GitHub.
fn update_check_due(app: &AppHandle, at_startup: bool) -> bool {
    let Ok(stored) = app.state::<AppState>().database.read(settings::load) else { return false };
    if !stored.update_check_on_startup {
        return false;
    }
    let spacing = if at_startup {
        MIN_CHECK_SPACING_MS
    } else {
        stored.update_check_interval_hours as i64 * HOUR_MS
    };
    stored.last_update_check.is_none_or(|last| now_ms() - last >= spacing)
}

/// Checks for updates at startup and then on the configured interval.
pub fn start_update_checks(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        let mut at_startup = true;
        loop {
            if update_check_due(&app, at_startup) && !cfg!(debug_assertions) {
                match updates::check(&app).await {
                    Ok(Some(update)) => {
                        let _ = app.emit("update-available", update.version);
                    }
                    Ok(None) => {}
                    Err(error) => info!("update check failed: {error}"),
                }
            }
            at_startup = false;
            tokio::time::sleep(Duration::from_secs(3_600)).await;
        }
    });
}

#[cfg(unix)]
pub fn handle_termination(app: AppHandle) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let Ok(mut signals) = Signals::new([SIGTERM, SIGINT, SIGHUP]) else { return };
    let _ = thread::Builder::new().name("daytrace-signals".into()).spawn(move || {
        if signals.forever().next().is_some() {
            quit(&app);
        }
    });
}

#[cfg(not(unix))]
pub fn handle_termination(_app: AppHandle) {}

pub fn database_path(app: &AppHandle) -> Result<std::path::PathBuf, tauri::Error> {
    Ok(app.path().app_data_dir()?.join("daytrace.db"))
}
