pub mod activity;
pub mod catalog;
pub mod privacy;
pub mod settings;
pub mod tracking;
pub mod updates;

use tauri::{AppHandle, Emitter};

fn notify_data_changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
}
