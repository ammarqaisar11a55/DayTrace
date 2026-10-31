use daytrace_core::analytics::{self, Report};
use daytrace_core::database::sessions;
use daytrace_core::models::{ManualSessionInput, Session, SessionFilter, SessionPatch};
use daytrace_core::time::local_range_str;
use serde::Deserialize;
use tauri::{AppHandle, State};

use super::notify_data_changed;
use crate::error::{to_message, CommandResult};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionQuery {
    from: String,
    to: String,
    application_id: Option<i64>,
    category_id: Option<i64>,
    limit: Option<i64>,
    newest_first: Option<bool>,
}

#[tauri::command(async)]
pub fn get_report(
    state: State<'_, AppState>,
    from: String,
    to: String,
    focus_categories: Option<Vec<i64>>,
) -> CommandResult<Report> {
    state
        .database
        .read(|connection| analytics::local_report(connection, &from, &to, focus_categories.as_deref()))
        .map_err(to_message)
}

#[tauri::command(async)]
pub fn list_sessions(state: State<'_, AppState>, query: SessionQuery) -> CommandResult<Vec<Session>> {
    let (from, to) = local_range_str(&query.from, &query.to).map_err(to_message)?;
    let filter = SessionFilter {
        from,
        to,
        application_id: query.application_id,
        category_id: query.category_id,
        limit: query.limit,
        newest_first: query.newest_first,
    };
    // Include what the tracker has not yet written, so "now" is really now.
    state.with_tracker(|tracker| tracker.flush());
    state.database.read(|connection| sessions::list(connection, &filter)).map_err(to_message)
}

#[tauri::command(async)]
pub fn update_session(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: SessionPatch,
) -> CommandResult<Session> {
    let session =
        state.database.write(|connection| sessions::update(connection, id, &patch)).map_err(to_message)?;
    notify_data_changed(&app);
    Ok(session)
}

#[tauri::command(async)]
pub fn delete_session(app: AppHandle, state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.database.write(|connection| sessions::delete(connection, id)).map_err(to_message)?;
    notify_data_changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn create_manual_session(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ManualSessionInput,
) -> CommandResult<Session> {
    let session =
        state.database.write(|connection| sessions::insert_manual(connection, &input)).map_err(to_message)?;
    notify_data_changed(&app);
    Ok(session)
}
