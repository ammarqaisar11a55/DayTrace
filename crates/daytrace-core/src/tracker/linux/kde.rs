//! KDE Plasma on Wayland, through KWin's scripting API.
//!
//! A small KWin script reports window activation to a D-Bus object owned by
//! DayTrace. The script is loaded at runtime and leaves nothing installed.

use std::fs;
use std::sync::{Arc, Mutex};

use zbus::blocking::{connection, Connection, Proxy};

use super::desktop_entries::friendly_name;
use super::{procinfo, FocusSource};
use crate::tracker::{Capability, EventSink, Foreground, ForegroundApp, SystemEvent};

const SERVICE: &str = "io.github.ammarqaisar11a55.DayTrace";
const PATH: &str = "/io/github/ammarqaisar11a55/DayTrace";
const PLUGIN: &str = "daytrace-focus";

const SCRIPT: &str = r#"
function report(window) {
    if (!window) {
        callDBus("io.github.ammarqaisar11a55.DayTrace", "/io/github/ammarqaisar11a55/DayTrace",
            "io.github.ammarqaisar11a55.DayTrace.KWin", "Cleared");
        return;
    }
    callDBus("io.github.ammarqaisar11a55.DayTrace", "/io/github/ammarqaisar11a55/DayTrace",
        "io.github.ammarqaisar11a55.DayTrace.KWin", "Focused",
        String(window.desktopFileName || ""), String(window.resourceClass || ""),
        String(window.caption || ""), Number(window.pid || 0));
}
if (workspace.windowActivated) {
    workspace.windowActivated.connect(report);
    report(workspace.activeWindow);
} else {
    workspace.clientActivated.connect(report);
    report(workspace.activeClient);
}
"#;

#[derive(Clone, Debug, Default)]
struct Focused {
    desktop_file: String,
    resource_class: String,
    caption: String,
    pid: u32,
}

#[derive(Default)]
struct Shared {
    focused: Option<Focused>,
    reported: bool,
}

struct Receiver {
    shared: Arc<Mutex<Shared>>,
    sink: EventSink,
}

#[zbus::interface(name = "io.github.ammarqaisar11a55.DayTrace.KWin")]
impl Receiver {
    // KWin scripts pass JavaScript numbers as signed integers.
    fn focused(&self, desktop_file: String, resource_class: String, caption: String, pid: i32) {
        let mut shared = self.shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        shared.reported = true;
        shared.focused = Some(Focused { desktop_file, resource_class, caption, pid: pid.max(0) as u32 });
        self.sink.send(SystemEvent::FocusChanged);
    }

    fn cleared(&self) {
        let mut shared = self.shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        shared.reported = true;
        shared.focused = None;
        self.sink.send(SystemEvent::FocusChanged);
    }
}

pub struct KdeFocus {
    shared: Arc<Mutex<Shared>>,
    _connection: Connection,
}

fn load_script(connection: &Connection) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("daytrace-kwin-{}.js", std::process::id()));
    fs::write(&path, SCRIPT).map_err(|error| error.to_string())?;

    let scripting = Proxy::new(connection, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting")
        .map_err(|error| error.to_string())?;
    let _: Result<bool, _> = scripting.call("unloadScript", &(PLUGIN,));
    let id: i32 = scripting
        .call("loadScript", &(path.to_string_lossy().as_ref(), PLUGIN))
        .map_err(|error| format!("KWin refused the tracking script: {error}"))?;
    if id < 0 {
        return Err("KWin refused the tracking script.".to_string());
    }
    let _: () = scripting.call("start", &()).map_err(|error| error.to_string())?;
    let _ = fs::remove_file(path);
    Ok(())
}

impl KdeFocus {
    pub fn start(sink: EventSink) -> Result<Self, String> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let receiver = Receiver { shared: shared.clone(), sink };
        let connection = connection::Builder::session()
            .and_then(|builder| builder.name(SERVICE))
            .and_then(|builder| builder.serve_at(PATH, receiver))
            .and_then(|builder| builder.build())
            .map_err(|error| format!("Could not start the KWin connection: {error}"))?;
        load_script(&connection)?;
        Ok(Self { shared, _connection: connection })
    }

    fn snapshot(&self) -> (Option<Focused>, bool) {
        let shared = self.shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        (shared.focused.clone(), shared.reported)
    }
}

impl FocusSource for KdeFocus {
    fn focused(&mut self, include_title: bool) -> Foreground {
        let (focused, reported) = self.snapshot();
        if !reported {
            return Foreground::Unknown;
        }
        let Some(focused) = focused else { return Foreground::NoFocus };

        let raw_identifier =
            if focused.desktop_file.is_empty() { &focused.resource_class } else { &focused.desktop_file };
        if raw_identifier.is_empty() {
            return Foreground::Unknown;
        }

        Foreground::App(ForegroundApp {
            identifier: raw_identifier.to_lowercase(),
            name: friendly_name(raw_identifier),
            executable_path: (focused.pid != 0).then(|| procinfo::executable_path(focused.pid)).flatten(),
            icon_path: None,
            window_title: (include_title && !focused.caption.is_empty()).then_some(focused.caption),
        })
    }

    fn capability(&mut self) -> Capability {
        if self.snapshot().1 {
            Capability::available()
        } else {
            Capability::limited("Waiting for KWin to report the first focus change.")
        }
    }

    fn executable_path(&self) -> Capability {
        Capability::available()
    }

    fn window_title(&self) -> Capability {
        Capability::available()
    }

    fn backend(&self) -> &'static str {
        "kwin-script"
    }
}
