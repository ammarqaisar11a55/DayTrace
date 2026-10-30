//! GNOME Shell on Wayland.
//!
//! GNOME refuses `Introspect.GetWindows` and `Eval` to ordinary applications, so
//! the only supported way to learn the focused window is code running inside
//! the Shell. DayTrace ships a small extension for that, installed only when the
//! user asks. The extension exposes the focused application on D-Bus and nothing
//! else. Idle time comes from Mutter's public idle monitor.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use log::debug;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::MatchRule;

use super::desktop_entries::friendly_name;
use super::{procinfo, FocusSource, IdleSource};
use crate::error::{invalid, Result};
use crate::tracker::{Capability, EventSink, Foreground, ForegroundApp, IntegrationState, SystemEvent};

pub const EXTENSION_UUID: &str = "daytrace@ammarqaisar11a55.github.io";
const SHELL: &str = "org.gnome.Shell";
const FOCUS_PATH: &str = "/io/github/ammarqaisar11a55/DayTrace";
const FOCUS_INTERFACE: &str = "io.github.ammarqaisar11a55.DayTrace.Focus";

const METADATA: &str = include_str!("../../../../../extensions/gnome-shell/metadata.json");
const EXTENSION_MODERN: &str = include_str!("../../../../../extensions/gnome-shell/extension.js");
const EXTENSION_LEGACY: &str = include_str!("../../../../../extensions/gnome-shell/legacy/extension.js");

/// The application focused in the Shell, as the extension reports it.
type FocusReply = (bool, String, String, String, u32, String);

fn extension_directory() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    Some(base.join("gnome-shell/extensions").join(EXTENSION_UUID))
}

fn shell_major_version(connection: &Connection) -> Option<u32> {
    let proxy = Proxy::new(connection, SHELL, "/org/gnome/Shell", "org.gnome.Shell").ok()?;
    let version: String = proxy.get_property("ShellVersion").ok()?;
    version.split('.').next()?.parse().ok()
}

fn set_enabled(enabled: bool) -> Result<()> {
    let read = Command::new("gsettings").args(["get", "org.gnome.shell", "enabled-extensions"]).output()?;
    if !read.status.success() {
        return invalid("Could not read the list of enabled GNOME extensions");
    }
    let current = String::from_utf8_lossy(&read.stdout);
    let current = current.trim().trim_start_matches("@as").trim();
    let mut extensions: Vec<String> = current
        .trim_matches(['[', ']'])
        .split(',')
        .map(|item| item.trim().trim_matches('\'').to_string())
        .filter(|item| !item.is_empty() && item != EXTENSION_UUID)
        .collect();
    if enabled {
        extensions.push(EXTENSION_UUID.to_string());
    }
    let value =
        format!("[{}]", extensions.iter().map(|item| format!("'{item}'")).collect::<Vec<_>>().join(", "));
    let write =
        Command::new("gsettings").args(["set", "org.gnome.shell", "enabled-extensions", &value]).status()?;
    if !write.success() {
        return invalid("Could not update the list of enabled GNOME extensions");
    }
    Ok(())
}

/// Writes the extension into the user's data directory and enables it. The
/// Shell only discovers new extensions at login, so it activates after the user
/// logs out and back in.
pub fn install_extension() -> Result<()> {
    let Some(directory) = extension_directory() else {
        return invalid("Could not find the home directory");
    };
    let connection = Connection::session().map_err(|error| crate::CoreError::Invalid(error.to_string()))?;
    let source = match shell_major_version(&connection) {
        Some(version) if version < 45 => EXTENSION_LEGACY,
        _ => EXTENSION_MODERN,
    };
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("metadata.json"), METADATA)?;
    fs::write(directory.join("extension.js"), source)?;
    set_enabled(true)
}

pub fn remove_extension() -> Result<()> {
    set_enabled(false)?;
    if let Some(directory) = extension_directory() {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
    }
    Ok(())
}

fn watch_focus(connection: Connection, sink: EventSink) -> zbus::Result<()> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(FOCUS_INTERFACE)?
        .member("FocusChanged")?
        .build();
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        message?;
        if !sink.send(SystemEvent::FocusChanged) {
            break;
        }
    }
    Ok(())
}

pub struct GnomeFocus {
    connection: Option<Connection>,
}

impl GnomeFocus {
    pub fn new(sink: EventSink) -> Self {
        let connection = Connection::session().ok();
        if let Some(connection) = connection.clone() {
            let spawned = thread::Builder::new().name("daytrace-gnome-focus".into()).spawn(move || {
                if let Err(error) = watch_focus(connection, sink) {
                    debug!("GNOME focus listener stopped: {error}");
                }
            });
            if let Err(error) = spawned {
                debug!("could not start the GNOME focus listener: {error}");
            }
        }
        Self { connection }
    }

    fn query(&self, include_title: bool) -> Option<FocusReply> {
        let connection = self.connection.as_ref()?;
        let proxy = Proxy::new(connection, SHELL, FOCUS_PATH, FOCUS_INTERFACE).ok()?;
        proxy.call("GetFocus", &(include_title,)).ok()
    }

    fn is_active(&self) -> bool {
        self.query(false).is_some()
    }
}

impl FocusSource for GnomeFocus {
    fn focused(&mut self, include_title: bool) -> Foreground {
        let Some((focused, app_id, app_name, wm_class, pid, title)) = self.query(include_title) else {
            return Foreground::Unknown;
        };
        if !focused {
            return Foreground::NoFocus;
        }

        let raw_identifier = if app_id.is_empty() { &wm_class } else { &app_id };
        if raw_identifier.is_empty() {
            return Foreground::Unknown;
        }
        let identifier = raw_identifier.trim_end_matches(".desktop").to_lowercase();
        let name = if app_name.is_empty() {
            procinfo::process_name(pid).unwrap_or_else(|| friendly_name(raw_identifier))
        } else {
            app_name
        };

        Foreground::App(ForegroundApp {
            identifier,
            name,
            executable_path: (pid != 0).then(|| procinfo::executable_path(pid)).flatten(),
            icon_path: None,
            window_title: (include_title && !title.is_empty()).then_some(title),
        })
    }

    fn capability(&mut self) -> Capability {
        if self.is_active() {
            Capability::available()
        } else {
            Capability::unavailable(
                "GNOME does not let applications see the focused window. Install the DayTrace Shell integration to enable it.",
            )
        }
    }

    fn executable_path(&self) -> Capability {
        if self.is_active() {
            Capability::available()
        } else {
            Capability::unavailable("Needs the DayTrace Shell integration.")
        }
    }

    fn window_title(&self) -> Capability {
        if self.is_active() {
            Capability::available()
        } else {
            Capability::unavailable("Needs the DayTrace Shell integration.")
        }
    }

    fn integration(&mut self) -> Option<IntegrationState> {
        if self.is_active() {
            return Some(IntegrationState::Active);
        }
        let installed =
            extension_directory().is_some_and(|directory| directory.join("metadata.json").exists());
        Some(if installed { IntegrationState::PendingRestart } else { IntegrationState::NotInstalled })
    }

    fn backend(&self) -> &'static str {
        "gnome-shell-extension"
    }
}

pub struct MutterIdle {
    connection: Connection,
}

impl MutterIdle {
    const SERVICE: &str = "org.gnome.Mutter.IdleMonitor";
    const PATH: &str = "/org/gnome/Mutter/IdleMonitor/Core";

    pub fn connect() -> Option<Self> {
        let connection = Connection::session().ok()?;
        let mut idle = Self { connection };
        idle.idle_ms().is_some().then_some(idle)
    }
}

impl IdleSource for MutterIdle {
    fn idle_ms(&mut self) -> Option<i64> {
        let proxy = Proxy::new(&self.connection, Self::SERVICE, Self::PATH, Self::SERVICE).ok()?;
        let idle: u64 = proxy.call("GetIdletime", &()).ok()?;
        Some(idle as i64)
    }

    fn capability(&mut self) -> Capability {
        if self.idle_ms().is_some() {
            Capability::available()
        } else {
            Capability::unavailable("Mutter's idle monitor is not reachable.")
        }
    }
}
