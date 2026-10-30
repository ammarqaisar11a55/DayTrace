use daytrace_core::config::Settings;
use daytrace_core::database::settings;
use daytrace_core::time::now_ms;
use daytrace_core::tracker::runtime::Message;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::error::{to_message, CommandResult};
use crate::state::AppState;

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|error| format!("Could not change the login setting: {error}"))
}

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    state.database.read(settings::load).map_err(to_message)
}

#[tauri::command(async)]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> CommandResult<Settings> {
    let previous = state.database.read(settings::load).map_err(to_message)?;
    let mut next = settings;
    if next.tracking_enabled && next.tracking_consent_at.is_none() {
        next.tracking_consent_at = Some(now_ms());
    }
    if next.start_on_login != previous.start_on_login {
        apply_autostart(&app, next.start_on_login)?;
    }
    state.database.write(|connection| settings::save(connection, &next)).map_err(to_message)?;
    state.send(Message::SettingsChanged);
    Ok(next)
}

/// Records the user's explicit choice at the end of onboarding.
#[tauri::command(async)]
pub fn complete_onboarding(state: State<'_, AppState>, enable_tracking: bool) -> CommandResult<Settings> {
    let mut next = state.database.read(settings::load).map_err(to_message)?;
    next.onboarding_completed = true;
    next.tracking_enabled = enable_tracking;
    if enable_tracking {
        next.tracking_consent_at = Some(now_ms());
    }
    state.database.write(|connection| settings::save(connection, &next)).map_err(to_message)?;
    state.send(Message::SettingsChanged);
    Ok(next)
}
