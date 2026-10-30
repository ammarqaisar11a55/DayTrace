use daytrace_core::database::settings;
use daytrace_core::time::now_ms;
use daytrace_core::updates;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{to_message, CommandResult};
use crate::state::{lock, AppState, DownloadedUpdate};
use crate::updater;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    channel: daytrace_core::config::UpdateChannel,
    install_method: updates::InstallMethod,
    can_install: bool,
    guidance: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command(async)]
pub fn update_info(app: AppHandle, state: State<'_, AppState>) -> CommandResult<UpdateInfo> {
    let channel = state.database.read(settings::load).map_err(to_message)?.update_channel;
    let method = updates::detect_install();
    Ok(UpdateInfo {
        current_version: app.package_info().version.to_string(),
        channel,
        install_method: method,
        can_install: method.can_self_update(),
        guidance: method.guidance().map(str::to_string),
    })
}

pub async fn check(app: &AppHandle) -> Result<Option<AvailableUpdate>, String> {
    let state = app.state::<AppState>();
    let found = updater::check(app).await?;

    let mut stored = state.database.read(settings::load).map_err(to_message)?;
    stored.last_update_check = Some(now_ms());
    state.database.write(|connection| settings::save(connection, &stored)).map_err(to_message)?;

    let available = found.as_ref().map(|update| AvailableUpdate {
        version: update.version.clone(),
        notes: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
    });
    let mut updates = lock(&state.updates);
    updates.checked = found;
    updates.known.clone_from(&available);
    Ok(available)
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> CommandResult<Option<AvailableUpdate>> {
    check(&app).await
}

#[tauri::command(async)]
pub fn known_update(state: State<'_, AppState>) -> Option<AvailableUpdate> {
    lock(&state.updates).known.clone()
}

/// Downloads the update and verifies its signature against the public key
/// compiled into this binary. The updater rejects anything that fails, so only
/// verified bytes ever reach the install step.
#[tauri::command]
pub async fn download_update(app: AppHandle) -> CommandResult<()> {
    if !updates::detect_install().can_self_update() {
        return Err("This installation is updated by its package manager".to_string());
    }
    let state = app.state::<AppState>();
    let update = lock(&state.updates).checked.take().ok_or("Check for updates first")?;

    let mut downloaded = 0u64;
    let progress = app.clone();
    let result = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await;

    match result {
        Ok(bytes) => {
            lock(&state.updates).downloaded = Some(DownloadedUpdate { update, bytes });
            Ok(())
        }
        Err(error) => {
            let message = updater::describe_error(&error);
            lock(&state.updates).checked = Some(update);
            Err(message)
        }
    }
}

#[tauri::command(async)]
pub fn install_update(state: State<'_, AppState>) -> CommandResult<()> {
    let downloaded = lock(&state.updates).downloaded.take().ok_or("Download the update first")?;
    // The Windows installer takes over and ends this process, so the final
    // session must be written first.
    state.shutdown_tracker();
    downloaded.update.install(&downloaded.bytes).map_err(|error| updater::describe_error(&error))
}

#[tauri::command(async)]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

#[tauri::command(async)]
pub fn quit_app(app: AppHandle) {
    crate::background::quit(&app);
}
