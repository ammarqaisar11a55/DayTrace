use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use chrono::Local;
use daytrace_core::database::{exclusions, sessions, transfer};
use daytrace_core::models::{Exclusion, ExclusionKind};
use daytrace_core::tracker::runtime::Message;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::notify_data_changed;
use crate::error::{to_message, CommandResult};
use crate::state::AppState;

fn choose_save_path(app: &AppHandle, name: &str, extension: &str) -> Option<PathBuf> {
    app.dialog()
        .file()
        .set_file_name(name)
        .add_filter("DayTrace data", &[extension])
        .blocking_save_file()
        .and_then(|path| path.into_path().ok())
}

fn stamped(prefix: &str, extension: &str) -> String {
    format!("{prefix}-{}.{extension}", Local::now().format("%Y-%m-%d"))
}

#[tauri::command(async)]
pub fn list_exclusions(state: State<'_, AppState>) -> CommandResult<Vec<Exclusion>> {
    state.database.read(exclusions::list).map_err(to_message)
}

#[tauri::command(async)]
pub fn add_exclusion(
    state: State<'_, AppState>,
    kind: ExclusionKind,
    value: String,
) -> CommandResult<()> {
    state.database.write(|connection| exclusions::add(connection, kind, &value)).map_err(to_message)?;
    state.send(Message::RulesChanged);
    Ok(())
}

#[tauri::command(async)]
pub fn remove_exclusion(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.database.write(|connection| exclusions::remove(connection, id)).map_err(to_message)?;
    state.send(Message::RulesChanged);
    Ok(())
}

#[tauri::command(async)]
pub fn export_data(
    app: AppHandle,
    state: State<'_, AppState>,
    format: String,
) -> CommandResult<Option<String>> {
    let extension = if format == "csv" { "csv" } else { "json" };
    let Some(path) = choose_save_path(&app, &stamped("daytrace-export", extension), extension) else {
        return Ok(None);
    };

    state.with_tracker(|tracker| tracker.flush());
    let file = File::create(&path).map_err(|error| error.to_string())?;
    let mut output = BufWriter::new(file);
    if extension == "csv" {
        state.database.read(|connection| transfer::write_csv(connection, &mut output)).map_err(to_message)?;
    } else {
        let data = state.database.read(transfer::collect).map_err(to_message)?;
        serde_json::to_writer_pretty(&mut output, &data).map_err(|error| error.to_string())?;
    }
    output.flush().map_err(|error| error.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command(async)]
pub fn backup_database(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<String>> {
    let Some(path) = choose_save_path(&app, &stamped("daytrace-backup", "sqlite"), "sqlite") else {
        return Ok(None);
    };
    state.with_tracker(|tracker| tracker.flush());
    state.database.backup_to(&path).map_err(to_message)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command(async)]
pub fn restore_database(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<String>> {
    let chosen = app
        .dialog()
        .file()
        .add_filter("DayTrace backup", &["sqlite", "db"])
        .blocking_pick_file()
        .and_then(|path| path.into_path().ok());
    let Some(path) = chosen else { return Ok(None) };

    state.with_tracker(|tracker| tracker.flush());
    let safety_copy = state.database.restore_from(&path).map_err(to_message)?;
    state.with_tracker(|tracker| tracker.data_replaced());
    state.send(Message::RulesChanged);
    notify_data_changed(&app);
    Ok(Some(safety_copy.display().to_string()))
}

#[tauri::command(async)]
pub fn delete_all_activity(app: AppHandle, state: State<'_, AppState>) -> CommandResult<usize> {
    state.with_tracker(|tracker| tracker.flush());
    let count = state.database.write(sessions::delete_all).map_err(to_message)?;
    state.with_tracker(|tracker| tracker.data_replaced());
    notify_data_changed(&app);
    Ok(count)
}
