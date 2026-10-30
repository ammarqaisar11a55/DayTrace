use daytrace_core::time::{now_ms, MINUTE_MS};
use daytrace_core::tracker::engine::ManualActivity;
use daytrace_core::tracker::runtime::{Message, TrackerStatus};
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::error::CommandResult;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualTimerInput {
    label: Option<String>,
    project_id: Option<i64>,
    category_id: Option<i64>,
}

#[tauri::command(async)]
pub fn tracker_status(state: State<'_, AppState>) -> Option<TrackerStatus> {
    state.with_tracker(|tracker| tracker.status()).flatten()
}

#[tauri::command(async)]
pub fn pause_tracking(state: State<'_, AppState>, minutes: Option<i64>) {
    let until = minutes.map(|minutes| now_ms() + minutes * MINUTE_MS);
    state.send(Message::Pause { until });
}

#[tauri::command(async)]
pub fn resume_tracking(state: State<'_, AppState>) {
    state.send(Message::Resume);
}

#[tauri::command(async)]
pub fn start_manual_timer(state: State<'_, AppState>, input: ManualTimerInput) {
    state.send(Message::StartManual(ManualActivity {
        label: input.label,
        project_id: input.project_id,
        category_id: input.category_id,
    }));
}

#[tauri::command(async)]
pub fn stop_manual_timer(state: State<'_, AppState>) {
    state.send(Message::StopManual);
}

#[tauri::command(async)]
pub fn install_shell_integration(_app: AppHandle) -> CommandResult<()> {
    #[cfg(target_os = "linux")]
    {
        daytrace_core::tracker::linux::gnome::install_extension().map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("This integration is only for GNOME".to_string())
    }
}

#[tauri::command(async)]
pub fn remove_shell_integration(_app: AppHandle) -> CommandResult<()> {
    #[cfg(target_os = "linux")]
    {
        daytrace_core::tracker::linux::gnome::remove_extension().map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("This integration is only for GNOME".to_string())
    }
}
