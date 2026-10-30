//! Activity tracking: the shared session engine and platform backends.
//!
//! Backends only observe. Each one reports a [`Sample`] of the current state,
//! and all session logic lives in [`engine`], so every platform behaves alike.

pub mod engine;
pub mod runtime;
mod unsupported;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(windows)]
mod windows;

use std::sync::mpsc::Sender;

use serde::{Deserialize, Serialize};

use runtime::Message;
pub use unsupported::UnsupportedTracker;
#[cfg(windows)]
pub use windows::executable_icon_png;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForegroundApp {
    /// Stable, lowercase identity of the application on this platform.
    pub identifier: String,
    pub name: String,
    pub executable_path: Option<String>,
    pub icon_path: Option<String>,
    pub window_title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Foreground {
    App(ForegroundApp),
    /// The platform cannot say which application has focus.
    Unknown,
    /// Nothing has focus at this moment, for example during a window switch.
    NoFocus,
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub foreground: Foreground,
    /// Time since the last input. `None` when the platform cannot report it.
    pub idle_ms: Option<i64>,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SampleOptions {
    pub include_window_title: bool,
}

/// Interrupts a backend can raise instead of waiting for the next poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemEvent {
    FocusChanged,
    Lock,
    Unlock,
    Suspend,
    Resume,
    Shutdown,
}

/// Lets a backend deliver events to the tracking runtime from its own threads.
#[derive(Clone, Debug)]
pub struct EventSink(Sender<Message>);

impl EventSink {
    pub fn new(sender: Sender<Message>) -> Self {
        Self(sender)
    }

    /// Returns false once the runtime has stopped, so threads can exit.
    pub fn send(&self, event: SystemEvent) -> bool {
        self.0.send(Message::System(event)).is_ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    Available,
    Limited,
    Unavailable,
}

/// One honestly reported capability, with the reason when it is reduced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub support: Support,
    pub detail: Option<String>,
}

impl Capability {
    pub fn available() -> Self {
        Self { support: Support::Available, detail: None }
    }

    pub fn limited(detail: impl Into<String>) -> Self {
        Self { support: Support::Limited, detail: Some(detail.into()) }
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self { support: Support::Unavailable, detail: Some(detail.into()) }
    }

    pub fn is_available(&self) -> bool {
        self.support != Support::Unavailable
    }
}

/// An optional desktop integration the user can choose to install.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationState {
    NotInstalled,
    /// Installed, but the desktop has not loaded it yet.
    PendingRestart,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub platform: String,
    pub display_server: String,
    pub desktop: String,
    pub backend: String,
    pub foreground_app: Capability,
    pub executable_path: Capability,
    pub window_title: Capability,
    pub idle: Capability,
    pub lock: Capability,
    pub suspend: Capability,
    pub integration: Option<IntegrationState>,
}

pub trait ActivityTracker: Send {
    fn capabilities(&mut self) -> Capabilities;
    fn sample(&mut self, options: SampleOptions) -> Sample;
}

/// Picks the backend for the running platform and session.
pub fn create_tracker(sink: EventSink) -> Box<dyn ActivityTracker> {
    #[cfg(target_os = "linux")]
    {
        linux::create(sink)
    }
    #[cfg(windows)]
    {
        Box::new(windows::WindowsTracker::new(sink))
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = sink;
        Box::new(UnsupportedTracker::new("This operating system is not supported"))
    }
}
