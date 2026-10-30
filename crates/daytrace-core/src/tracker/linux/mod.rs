//! Linux backends, chosen by what the running session actually offers.
//!
//! Wayland deliberately hides other applications' windows. Each desktop has its
//! own supported integration, and when none applies the tracker says so rather
//! than guessing. X11 is never used as a fallback under Wayland, because
//! Xwayland only knows about a fraction of the windows and would misreport the
//! rest.

pub mod desktop_entries;
pub mod gnome;
mod kde;
mod procinfo;
mod session;
mod wayland;
mod x11;

use std::env;

use super::{
    ActivityTracker, Capabilities, Capability, EventSink, Foreground, IntegrationState, Sample,
    SampleOptions, UnsupportedTracker,
};
use desktop_entries::DesktopEntries;
use session::SessionMonitor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayServer {
    X11,
    Wayland,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Other,
}

pub fn detect_display_server(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    x11_display: Option<&str>,
) -> DisplayServer {
    match session_type {
        Some("wayland") => DisplayServer::Wayland,
        Some("x11") => DisplayServer::X11,
        _ if wayland_display.is_some() => DisplayServer::Wayland,
        _ if x11_display.is_some() => DisplayServer::X11,
        _ => DisplayServer::None,
    }
}

pub fn detect_desktop(current_desktop: &str) -> Desktop {
    let names: Vec<String> = current_desktop.split(':').map(str::to_lowercase).collect();
    if names.iter().any(|name| name == "gnome" || name == "unity") {
        Desktop::Gnome
    } else if names.iter().any(|name| name == "kde") {
        Desktop::Kde
    } else {
        Desktop::Other
    }
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

pub fn create(sink: EventSink) -> Box<dyn ActivityTracker> {
    let display = detect_display_server(
        env_value("XDG_SESSION_TYPE").as_deref(),
        env_value("WAYLAND_DISPLAY").as_deref(),
        env_value("DISPLAY").as_deref(),
    );
    let desktop_name = env_value("XDG_CURRENT_DESKTOP").unwrap_or_else(|| "unknown".to_string());
    let desktop = detect_desktop(&desktop_name);

    match display {
        DisplayServer::X11 => match x11::LinuxX11Tracker::connect(sink.clone(), &desktop_name) {
            Ok(tracker) => Box::new(tracker),
            Err(error) => Box::new(UnsupportedTracker::new(format!("Could not reach the X server: {error}"))),
        },
        DisplayServer::Wayland => {
            Box::new(LinuxWaylandTracker::new(sink, desktop, desktop_name))
        }
        DisplayServer::None => Box::new(UnsupportedTracker::new(
            "No graphical session was found. DayTrace needs X11 or Wayland.",
        )),
    }
}

/// Reports the focused window on a Wayland desktop.
pub(super) trait FocusSource: Send {
    fn focused(&mut self, include_title: bool) -> Foreground;
    fn capability(&mut self) -> Capability;
    fn executable_path(&self) -> Capability;
    fn window_title(&self) -> Capability;
    fn integration(&mut self) -> Option<IntegrationState> {
        None
    }
    fn backend(&self) -> &'static str;
}

pub(super) trait IdleSource: Send {
    fn idle_ms(&mut self) -> Option<i64>;
    fn capability(&mut self) -> Capability;
}

struct NoFocusSource {
    reason: String,
}

impl FocusSource for NoFocusSource {
    fn focused(&mut self, _include_title: bool) -> Foreground {
        Foreground::Unknown
    }

    fn capability(&mut self) -> Capability {
        Capability::unavailable(self.reason.clone())
    }

    fn executable_path(&self) -> Capability {
        Capability::unavailable(self.reason.clone())
    }

    fn window_title(&self) -> Capability {
        Capability::unavailable(self.reason.clone())
    }

    fn backend(&self) -> &'static str {
        "wayland-no-focus"
    }
}

/// Uses `logind`'s idle flag, which only says whether the session is idle.
struct LogindIdle {
    session: SessionMonitor,
}

impl IdleSource for LogindIdle {
    fn idle_ms(&mut self) -> Option<i64> {
        self.session.idle_ms()
    }

    fn capability(&mut self) -> Capability {
        if self.session.idle_ms().is_some() {
            Capability::limited("Uses the session's idle flag, so the desktop's own idle delay applies.")
        } else {
            Capability::unavailable("This desktop exposes no idle information.")
        }
    }
}

pub struct LinuxWaylandTracker {
    focus: Box<dyn FocusSource>,
    idle: Box<dyn IdleSource>,
    session: SessionMonitor,
    entries: DesktopEntries,
    desktop_name: String,
}

impl LinuxWaylandTracker {
    fn new(sink: EventSink, desktop: Desktop, desktop_name: String) -> Self {
        let session = SessionMonitor::start(sink.clone());
        let client = wayland::WaylandClient::start(sink.clone());

        let focus: Box<dyn FocusSource> = match desktop {
            Desktop::Gnome => Box::new(gnome::GnomeFocus::new(sink)),
            Desktop::Kde => match kde::KdeFocus::start(sink) {
                Ok(focus) => Box::new(focus),
                Err(reason) => Box::new(NoFocusSource { reason }),
            },
            Desktop::Other => match client.as_ref().filter(|client| client.has_foreign_toplevel()) {
                Some(client) => Box::new(wayland::WlrFocus::new(client.clone())),
                None => Box::new(NoFocusSource {
                    reason: "This compositor does not offer a supported way to see the focused application."
                        .to_string(),
                }),
            },
        };

        let idle: Box<dyn IdleSource> = match (desktop, client) {
            (Desktop::Gnome, _) => match gnome::MutterIdle::connect() {
                Some(idle) => Box::new(idle),
                None => Box::new(LogindIdle { session: session.clone() }),
            },
            (_, Some(client)) if client.has_idle_notify() => Box::new(wayland::NotifyIdle::new(client)),
            _ => Box::new(LogindIdle { session: session.clone() }),
        };

        Self { focus, idle, session, entries: DesktopEntries::load(), desktop_name }
    }
}

impl ActivityTracker for LinuxWaylandTracker {
    fn capabilities(&mut self) -> Capabilities {
        Capabilities {
            platform: "linux".to_string(),
            display_server: "wayland".to_string(),
            desktop: self.desktop_name.clone(),
            backend: self.focus.backend().to_string(),
            foreground_app: self.focus.capability(),
            executable_path: self.focus.executable_path(),
            window_title: self.focus.window_title(),
            idle: self.idle.capability(),
            lock: self.session.lock_capability(),
            suspend: self.session.suspend_capability(),
            integration: self.focus.integration(),
        }
    }

    fn sample(&mut self, options: SampleOptions) -> Sample {
        let foreground = match self.focus.focused(options.include_window_title) {
            Foreground::App(app) => Foreground::App(self.entries.enrich(app)),
            other => other,
        };
        Sample { foreground, idle_ms: self.idle.idle_ms(), locked: self.session.is_locked() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_type_wins_over_display_variables() {
        assert_eq!(detect_display_server(Some("x11"), Some("wayland-0"), Some(":0")), DisplayServer::X11);
        assert_eq!(detect_display_server(Some("wayland"), None, Some(":0")), DisplayServer::Wayland);
    }

    #[test]
    fn display_variables_are_the_fallback() {
        assert_eq!(detect_display_server(None, Some("wayland-0"), Some(":0")), DisplayServer::Wayland);
        assert_eq!(detect_display_server(Some("tty"), None, Some(":0")), DisplayServer::X11);
        assert_eq!(detect_display_server(None, None, None), DisplayServer::None);
    }

    #[test]
    fn desktops_are_recognized_from_colon_lists() {
        assert_eq!(detect_desktop("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(detect_desktop("KDE"), Desktop::Kde);
        assert_eq!(detect_desktop("sway"), Desktop::Other);
        assert_eq!(detect_desktop("Hyprland"), Desktop::Other);
    }
}
