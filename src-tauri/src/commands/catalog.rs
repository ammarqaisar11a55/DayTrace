//! Applications, categories and classification rules.

use std::fs;
use std::path::Path;

use base64::Engine;
use daytrace_core::classification::{self, Rule, RuleInput, RulePreview};
use daytrace_core::database::{applications, categories, rules};
use daytrace_core::models::{Application, ApplicationPatch, ApplicationUsage, Category, CategoryInput};
use daytrace_core::time::local_range_str;
use daytrace_core::tracker::runtime::Message;
use tauri::{AppHandle, State};

use super::notify_data_changed;
use crate::error::{to_message, CommandResult};
use crate::state::AppState;

/// Rules and defaults changed, so the tracker must classify with the new ones.
fn refresh(app: &AppHandle, state: &AppState) {
    state.send(Message::RulesChanged);
    notify_data_changed(app);
}

#[tauri::command(async)]
pub fn list_applications(state: State<'_, AppState>) -> CommandResult<Vec<Application>> {
    state.database.read(applications::list).map_err(to_message)
}

#[tauri::command(async)]
pub fn list_application_usage(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> CommandResult<Vec<ApplicationUsage>> {
    let (start, end) = local_range_str(&from, &to).map_err(to_message)?;
    state.database.read(|connection| applications::list_usage(connection, start, end)).map_err(to_message)
}

#[tauri::command(async)]
pub fn update_application(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: ApplicationPatch,
) -> CommandResult<()> {
    state.database.write(|connection| applications::update(connection, id, &patch)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(())
}

#[tauri::command(async)]
pub fn delete_application(app: AppHandle, state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state
        .database
        .erase(|connection| applications::delete_with_history(connection, id))
        .map_err(to_message)?;
    state.with_tracker(|tracker| tracker.data_replaced());
    refresh(&app, &state);
    Ok(())
}

fn icon_data_url(path: &str) -> Option<String> {
    #[cfg(windows)]
    if path.to_lowercase().ends_with(".exe") {
        let png = daytrace_core::tracker::executable_icon_png(path)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        return Some(format!("data:image/png;base64,{encoded}"));
    }

    let mime = match Path::new(path).extension()?.to_str()?.to_lowercase().as_str() {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    let bytes = fs::read(path).ok()?;
    if bytes.len() > 2_000_000 {
        return None;
    }
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command(async)]
pub fn application_icon(state: State<'_, AppState>, id: i64) -> CommandResult<Option<String>> {
    let application =
        state.database.read(|connection| applications::get(connection, id)).map_err(to_message)?;
    Ok(application.icon_path.as_deref().and_then(icon_data_url))
}

#[tauri::command(async)]
pub fn list_categories(state: State<'_, AppState>) -> CommandResult<Vec<Category>> {
    state.database.read(categories::list).map_err(to_message)
}

#[tauri::command(async)]
pub fn create_category(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CategoryInput,
) -> CommandResult<Category> {
    let category =
        state.database.write(|connection| categories::create(connection, &input)).map_err(to_message)?;
    notify_data_changed(&app);
    Ok(category)
}

#[tauri::command(async)]
pub fn update_category(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    input: CategoryInput,
) -> CommandResult<Category> {
    let category =
        state.database.write(|connection| categories::update(connection, id, &input)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(category)
}

#[tauri::command(async)]
pub fn delete_category(app: AppHandle, state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.database.write(|connection| categories::delete(connection, id)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(())
}

#[tauri::command(async)]
pub fn list_rules(state: State<'_, AppState>) -> CommandResult<Vec<Rule>> {
    state.database.read(rules::list).map_err(to_message)
}

#[tauri::command(async)]
pub fn create_rule(app: AppHandle, state: State<'_, AppState>, input: RuleInput) -> CommandResult<Rule> {
    let rule = state.database.write(|connection| rules::create(connection, &input)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(rule)
}

#[tauri::command(async)]
pub fn update_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    input: RuleInput,
) -> CommandResult<Rule> {
    let rule =
        state.database.write(|connection| rules::update(connection, id, &input)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(rule)
}

#[tauri::command(async)]
pub fn delete_rule(app: AppHandle, state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.database.write(|connection| rules::delete(connection, id)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(())
}

#[tauri::command(async)]
pub fn reorder_rules(app: AppHandle, state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<()> {
    state.database.write(|connection| rules::reorder(connection, &ids)).map_err(to_message)?;
    refresh(&app, &state);
    Ok(())
}

#[tauri::command(async)]
pub fn preview_rules(state: State<'_, AppState>, from: String, to: String) -> CommandResult<RulePreview> {
    let (start, end) = local_range_str(&from, &to).map_err(to_message)?;
    state.database.read(|connection| classification::preview(connection, start, end)).map_err(to_message)
}

#[tauri::command(async)]
pub fn apply_rules(
    app: AppHandle,
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> CommandResult<RulePreview> {
    let (start, end) = local_range_str(&from, &to).map_err(to_message)?;
    let result = state
        .database
        .write(|connection| classification::apply(connection, start, end))
        .map_err(to_message)?;
    notify_data_changed(&app);
    Ok(result)
}
