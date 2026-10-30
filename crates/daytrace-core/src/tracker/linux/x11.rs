//! X11, through EWMH properties and the MIT-SCREEN-SAVER extension.

use std::thread;

use log::debug;
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as ScreenSaverExt;
use x11rb::protocol::xproto::{Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use super::desktop_entries::{friendly_name, DesktopEntries};
use super::procinfo;
use super::session::SessionMonitor;
use crate::tracker::{
    ActivityTracker, Capabilities, Capability, EventSink, Foreground, ForegroundApp, Sample, SampleOptions,
    SystemEvent,
};

struct Atoms {
    active_window: Atom,
    supported: Atom,
    wm_pid: Atom,
    wm_name: Atom,
    utf8_string: Atom,
}

fn intern(connection: &RustConnection, name: &str) -> Result<Atom, String> {
    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn watch_root(screen: usize, active_window: Atom, sink: EventSink) -> Result<(), String> {
    let (connection, _) = RustConnection::connect(None).map_err(|error| error.to_string())?;
    let root = connection.setup().roots[screen].root;
    let attributes = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    connection
        .change_window_attributes(root, &attributes)
        .map_err(|error| error.to_string())?
        .check()
        .map_err(|error| error.to_string())?;

    loop {
        match connection.wait_for_event() {
            Ok(Event::PropertyNotify(event)) if event.atom == active_window => {
                if !sink.send(SystemEvent::FocusChanged) {
                    return Ok(());
                }
            }
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

pub struct LinuxX11Tracker {
    connection: RustConnection,
    root: Window,
    atoms: Atoms,
    has_ewmh: bool,
    has_screen_saver: bool,
    session: SessionMonitor,
    entries: DesktopEntries,
    desktop_name: String,
}

impl LinuxX11Tracker {
    pub fn connect(sink: EventSink, desktop_name: &str) -> Result<Self, String> {
        let (connection, screen) = RustConnection::connect(None).map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        let atoms = Atoms {
            active_window: intern(&connection, "_NET_ACTIVE_WINDOW")?,
            supported: intern(&connection, "_NET_SUPPORTED")?,
            wm_pid: intern(&connection, "_NET_WM_PID")?,
            wm_name: intern(&connection, "_NET_WM_NAME")?,
            utf8_string: intern(&connection, "UTF8_STRING")?,
        };

        let mut tracker = Self {
            has_ewmh: false,
            has_screen_saver: false,
            session: SessionMonitor::start(sink.clone()),
            entries: DesktopEntries::load(),
            desktop_name: desktop_name.to_string(),
            connection,
            root,
            atoms,
        };
        tracker.has_ewmh = tracker.check_ewmh();
        tracker.has_screen_saver = tracker.read_idle().is_some();

        let active_window = tracker.atoms.active_window;
        let spawned = thread::Builder::new().name("daytrace-x11".into()).spawn(move || {
            if let Err(error) = watch_root(screen, active_window, sink) {
                debug!("X11 event listener stopped: {error}");
            }
        });
        if let Err(error) = spawned {
            debug!("could not start the X11 event listener: {error}");
        }
        Ok(tracker)
    }

    fn check_ewmh(&self) -> bool {
        let reply = self
            .connection
            .get_property(false, self.root, self.atoms.supported, AtomEnum::ATOM, 0, 1024)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        reply
            .and_then(|reply| {
                reply.value32().map(|mut atoms| atoms.any(|atom| atom == self.atoms.active_window))
            })
            .unwrap_or(false)
    }

    fn active_window(&self) -> Option<Window> {
        let reply = self
            .connection
            .get_property(false, self.root, self.atoms.active_window, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let window = reply.value32()?.next();
        window.filter(|window| *window != x11rb::NONE)
    }

    fn pid(&self, window: Window) -> Option<u32> {
        let reply = self
            .connection
            .get_property(false, window, self.atoms.wm_pid, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let pid = reply.value32()?.next();
        pid.filter(|pid| *pid != 0)
    }

    /// The instance and class parts of `WM_CLASS`.
    fn wm_class(&self, window: Window) -> Option<(String, String)> {
        let reply = self
            .connection
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
            .ok()?
            .reply()
            .ok()?;
        parse_wm_class(&reply.value)
    }

    fn title(&self, window: Window) -> Option<String> {
        let reply = self
            .connection
            .get_property(false, window, self.atoms.wm_name, self.atoms.utf8_string, 0, 1024)
            .ok()?
            .reply()
            .ok()?;
        let title = String::from_utf8_lossy(&reply.value).trim().to_string();
        (!title.is_empty()).then_some(title)
    }

    fn read_idle(&self) -> Option<i64> {
        let reply = self.connection.screensaver_query_info(self.root).ok()?.reply().ok()?;
        Some(reply.ms_since_user_input as i64)
    }

    fn foreground(&self, include_title: bool) -> Foreground {
        if !self.has_ewmh {
            return Foreground::Unknown;
        }
        let Some(window) = self.active_window() else { return Foreground::NoFocus };

        let pid = self.pid(window);
        let class = self.wm_class(window);
        let identifier =
            class.as_ref().map(|(_, class)| class.clone()).or_else(|| pid.and_then(procinfo::process_name));
        let Some(identifier) = identifier else { return Foreground::Unknown };

        let app = ForegroundApp {
            name: friendly_name(&identifier),
            identifier: identifier.to_lowercase(),
            executable_path: pid.and_then(procinfo::executable_path),
            icon_path: None,
            window_title: if include_title { self.title(window) } else { None },
        };
        Foreground::App(self.entries.enrich(app))
    }
}

fn parse_wm_class(value: &[u8]) -> Option<(String, String)> {
    let mut parts = value.split(|byte| *byte == 0).filter(|part| !part.is_empty());
    let instance = String::from_utf8_lossy(parts.next()?).to_string();
    let class =
        parts.next().map_or_else(|| instance.clone(), |part| String::from_utf8_lossy(part).to_string());
    Some((instance, class))
}

impl ActivityTracker for LinuxX11Tracker {
    fn capabilities(&mut self) -> Capabilities {
        let foreground_app = if self.has_ewmh {
            Capability::available()
        } else {
            Capability::unavailable("This window manager does not publish the active window.")
        };
        Capabilities {
            platform: "linux".to_string(),
            display_server: "x11".to_string(),
            desktop: self.desktop_name.clone(),
            backend: "x11-ewmh".to_string(),
            foreground_app: foreground_app.clone(),
            executable_path: if self.has_ewmh {
                Capability::limited("Available for windows that publish their process ID.")
            } else {
                foreground_app.clone()
            },
            window_title: foreground_app,
            idle: if self.has_screen_saver {
                Capability::available()
            } else {
                Capability::unavailable("The X server does not offer the screen saver extension.")
            },
            lock: self.session.lock_capability(),
            suspend: self.session.suspend_capability(),
            integration: None,
        }
    }

    fn sample(&mut self, options: SampleOptions) -> Sample {
        Sample {
            foreground: self.foreground(options.include_window_title),
            idle_ms: self.read_idle().or_else(|| self.session.idle_ms()),
            locked: self.session.is_locked(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wm_class_has_instance_and_class() {
        assert_eq!(parse_wm_class(b"code\0Code\0"), Some(("code".to_string(), "Code".to_string())));
    }

    #[test]
    fn wm_class_with_one_part_uses_it_for_both() {
        assert_eq!(parse_wm_class(b"xterm\0"), Some(("xterm".to_string(), "xterm".to_string())));
        assert_eq!(parse_wm_class(b""), None);
    }
}
