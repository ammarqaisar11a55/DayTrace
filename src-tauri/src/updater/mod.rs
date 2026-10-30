use daytrace_core::database::settings;
use daytrace_core::updates;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Error, Update, UpdaterExt};

use crate::state::AppState;

pub async fn check(app: &AppHandle) -> Result<Option<Update>, String> {
    let channel = app
        .state::<AppState>()
        .database
        .read(settings::load)
        .map_err(|error| error.to_string())?
        .update_channel;

    let endpoint = updates::manifest_url(channel).parse().map_err(|_| "Invalid update address".to_string())?;
    let updater = app
        .updater_builder()
        .version_comparator(move |current, remote| {
            updates::should_update(&current.to_string(), &remote.version.to_string(), channel)
        })
        .endpoints(vec![endpoint])
        .map_err(|error| describe_error(&error))?
        .build()
        .map_err(|error| describe_error(&error))?;

    updater.check().await.map_err(|error| describe_error(&error))
}

/// Turns updater failures into something a user can act on.
pub fn describe_error(error: &Error) -> String {
    let detail = error.to_string();
    let lowered = detail.to_lowercase();
    match error {
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) => {
            "The update's signature could not be verified, so it was not installed.".to_string()
        }
        Error::Reqwest(_) if lowered.contains("connect") || lowered.contains("dns") => {
            "No internet connection. Check your connection and try again.".to_string()
        }
        Error::Reqwest(_) => format!("The download failed: {detail}"),
        Error::Io(io) if io.raw_os_error() == Some(28) || io.raw_os_error() == Some(112) => {
            "There is not enough disk space to install the update.".to_string()
        }
        Error::UnsupportedOs | Error::UnsupportedArch | Error::TargetNotFound(_) => {
            "No update is published for this system.".to_string()
        }
        _ => format!("The update failed: {detail}"),
    }
}
