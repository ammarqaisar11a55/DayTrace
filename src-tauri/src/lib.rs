mod background;
mod commands;
mod error;
mod state;
mod updater;

use std::sync::Arc;

use daytrace_core::database::{settings, Database};
use daytrace_core::tracker::{create_tracker, runtime};
use log::LevelFilter;
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

use commands::{activity, catalog, privacy, settings as settings_commands, tracking, updates};
use state::AppState;

const BACKGROUND_FLAG: &str = "--background";

fn start(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let database = Arc::new(Database::open(background::database_path(&handle)?)?);
    let stored = database.read(settings::load)?;
    app.manage(AppState::new(database.clone()));

    background::create_tray(&handle);
    let status_handle = handle.clone();
    let tracker = runtime::spawn(
        database,
        create_tracker,
        Box::new(move |status| {
            let _ = status_handle.emit("tracker-status", status);
            background::update_tray(&status_handle, status);
        }),
    );
    app.state::<AppState>().set_tracker(tracker);

    background::handle_termination(handle.clone());
    background::start_maintenance(handle.clone());
    background::start_update_checks(handle.clone());

    let launched_in_background = std::env::args().any(|argument| argument == BACKGROUND_FLAG);
    if !(launched_in_background && stored.start_hidden && stored.run_in_background) {
        background::show_window(&handle);
    }
    Ok(())
}

pub fn run() {
    let level = if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info };
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _arguments, _directory| {
            background::show_window(app);
        }))
        .plugin(tauri_plugin_log::Builder::new().level(level).build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![BACKGROUND_FLAG])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            settings_commands::get_settings,
            settings_commands::update_settings,
            settings_commands::complete_onboarding,
            tracking::tracker_status,
            tracking::pause_tracking,
            tracking::resume_tracking,
            tracking::start_manual_timer,
            tracking::stop_manual_timer,
            tracking::install_shell_integration,
            tracking::remove_shell_integration,
            activity::get_report,
            activity::list_sessions,
            activity::update_session,
            activity::delete_session,
            activity::create_manual_session,
            catalog::list_applications,
            catalog::list_application_usage,
            catalog::update_application,
            catalog::delete_application,
            catalog::application_icon,
            catalog::list_categories,
            catalog::create_category,
            catalog::update_category,
            catalog::delete_category,
            catalog::list_rules,
            catalog::create_rule,
            catalog::update_rule,
            catalog::delete_rule,
            catalog::reorder_rules,
            catalog::preview_rules,
            catalog::apply_rules,
            privacy::list_exclusions,
            privacy::add_exclusion,
            privacy::remove_exclusion,
            privacy::export_data,
            privacy::backup_database,
            privacy::restore_database,
            privacy::delete_all_activity,
            updates::update_info,
            updates::check_for_update,
            updates::known_update,
            updates::download_update,
            updates::install_update,
            updates::restart_app,
            updates::quit_app,
        ])
        .setup(|app| start(app))
        .build(tauri::generate_context!())
        .expect("failed to start DayTrace")
        .run(|app, event| match event {
            // Closing the last window keeps tracking unless the user turned that
            // off. An explicit quit passes an exit code and always ends the process.
            RunEvent::ExitRequested { api, code: None, .. } => {
                if background::runs_in_background(app) {
                    api.prevent_exit();
                }
            }
            RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.shutdown_tracker();
                }
            }
            _ => {}
        });
}
