//! The system tray menu. Not every desktop can show one, so failing to create
//! it is logged and the application carries on. Launching DayTrace again always
//! reopens the window, and Settings has a Quit button.

use std::sync::Mutex;

use daytrace_core::tracker::runtime::{Message, TrackerStatus, TrackingState};
use log::info;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use super::{quit, show_window};
use crate::state::{lock, AppState};

struct TrayItems {
    status: MenuItem<Wry>,
    pause: MenuItem<Wry>,
    resume: MenuItem<Wry>,
}

struct TrayState(Mutex<Option<TrayItems>>);

fn build(app: &AppHandle) -> tauri::Result<TrayItems> {
    let open = MenuItem::with_id(app, "open", "Open DayTrace", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Starting", false, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause tracking", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume tracking", false, None::<&str>)?;
    let exit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &status, &separator, &pause, &resume, &separator, &exit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("DayTrace")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "pause" => app.state::<AppState>().send(Message::Pause { until: None }),
            "resume" => app.state::<AppState>().send(Message::Resume),
            "quit" => quit(app),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(TrayItems { status, pause, resume })
}

pub fn create(app: &AppHandle) {
    let items = match build(app) {
        Ok(items) => Some(items),
        Err(error) => {
            info!("no system tray available: {error}");
            None
        }
    };
    app.manage(TrayState(Mutex::new(items)));
}

fn describe(status: &TrackerStatus) -> String {
    match status.state {
        TrackingState::Tracking => match &status.current_application {
            Some(application) => format!("Tracking {application}"),
            None => "Tracking".to_string(),
        },
        TrackingState::Idle => "Idle".to_string(),
        TrackingState::Locked => "Screen locked".to_string(),
        TrackingState::Paused => "Paused".to_string(),
        TrackingState::Manual => "Timer running".to_string(),
        TrackingState::Disabled => "Tracking is off".to_string(),
    }
}

pub fn update_status(app: &AppHandle, status: &TrackerStatus) {
    let Some(tray) = app.try_state::<TrayState>() else { return };
    let items = lock(&tray.0);
    let Some(items) = items.as_ref() else { return };
    let can_pause = !matches!(
        status.state,
        TrackingState::Disabled | TrackingState::Paused | TrackingState::Manual
    );
    let _ = items.status.set_text(describe(status));
    let _ = items.pause.set_enabled(can_pause);
    let _ = items.resume.set_enabled(status.state == TrackingState::Paused);
}
