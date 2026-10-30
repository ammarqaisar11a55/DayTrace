use std::sync::{Arc, Mutex, MutexGuard};

use daytrace_core::database::Database;
use daytrace_core::tracker::runtime::{Message, TrackerHandle};
use tauri_plugin_updater::Update;

use crate::commands::updates::AvailableUpdate;

/// A verified update waiting for the user to install it.
pub struct DownloadedUpdate {
    pub update: Update,
    pub bytes: Vec<u8>,
}

#[derive(Default)]
pub struct UpdateState {
    pub checked: Option<Update>,
    pub downloaded: Option<DownloadedUpdate>,
    pub known: Option<AvailableUpdate>,
}

pub struct AppState {
    pub database: Arc<Database>,
    tracker: Mutex<Option<TrackerHandle>>,
    pub updates: Mutex<UpdateState>,
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AppState {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database, tracker: Mutex::new(None), updates: Mutex::new(UpdateState::default()) }
    }

    pub fn set_tracker(&self, tracker: TrackerHandle) {
        *lock(&self.tracker) = Some(tracker);
    }

    pub fn send(&self, message: Message) {
        if let Some(tracker) = lock(&self.tracker).as_ref() {
            tracker.send(message);
        }
    }

    pub fn with_tracker<T>(&self, action: impl FnOnce(&TrackerHandle) -> T) -> Option<T> {
        lock(&self.tracker).as_ref().map(action)
    }

    /// Stops tracking and writes the final state. Safe to call more than once.
    pub fn shutdown_tracker(&self) {
        if let Some(mut tracker) = lock(&self.tracker).take() {
            tracker.shutdown();
        }
    }
}
