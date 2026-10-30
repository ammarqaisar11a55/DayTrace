//! Lock, suspend, shutdown and idle state from `logind` and screen savers.
//!
//! Every source is optional. Confined packages may be refused access to the
//! system bus, in which case the capability report says so and suspend is still
//! caught by the engine's tick-gap detection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use log::{debug, warn};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::MatchRule;

use crate::time::now_ms;
use crate::tracker::{Capability, EventSink, SystemEvent};

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

#[derive(Clone)]
pub struct SessionMonitor {
    locked: Arc<AtomicBool>,
    logind: Arc<AtomicBool>,
    saver: Arc<AtomicBool>,
    system: Option<Connection>,
    session_path: Option<OwnedObjectPath>,
}

fn session_path(connection: &Connection) -> zbus::Result<OwnedObjectPath> {
    let manager = Proxy::new(connection, LOGIND, MANAGER_PATH, MANAGER)?;
    manager.call("GetSession", &("auto",))
}

fn spawn_listener(name: &str, listener: impl FnOnce() -> zbus::Result<()> + Send + 'static) {
    let label = name.to_string();
    let spawned = thread::Builder::new().name(format!("daytrace-{name}")).spawn(move || {
        if let Err(error) = listener() {
            debug!("{label} listener stopped: {error}");
        }
    });
    if let Err(error) = spawned {
        warn!("could not start the {name} listener: {error}");
    }
}

fn watch_manager(connection: Connection, sink: EventSink) -> zbus::Result<()> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(MANAGER)?
        .path(MANAGER_PATH)?
        .build();
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        let message = message?;
        let header = message.header();
        let Some(member) = header.member() else { continue };
        let Ok(starting) = message.body().deserialize::<bool>() else { continue };
        let event = match (member.as_str(), starting) {
            ("PrepareForSleep", true) => SystemEvent::Suspend,
            ("PrepareForSleep", false) => SystemEvent::Resume,
            ("PrepareForShutdown", true) => SystemEvent::Shutdown,
            _ => continue,
        };
        if !sink.send(event) {
            break;
        }
    }
    Ok(())
}

fn watch_locked_hint(
    connection: Connection,
    path: OwnedObjectPath,
    locked: Arc<AtomicBool>,
    sink: EventSink,
) -> zbus::Result<()> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path(path.as_ref())?
        .build();
    let proxy = Proxy::new(&connection, LOGIND, path.as_str().to_string(), SESSION)?;
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        message?;
        let Ok(value) = proxy.get_property::<bool>("LockedHint") else { continue };
        if locked.swap(value, Ordering::SeqCst) != value {
            let event = if value { SystemEvent::Lock } else { SystemEvent::Unlock };
            if !sink.send(event) {
                break;
            }
        }
    }
    Ok(())
}

fn watch_screen_saver(locked: Arc<AtomicBool>, sink: EventSink) -> zbus::Result<()> {
    let connection = Connection::session()?;
    let rule = MatchRule::builder().msg_type(zbus::message::Type::Signal).member("ActiveChanged")?.build();
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        let message = message?;
        let Ok(active) = message.body().deserialize::<bool>() else { continue };
        if locked.swap(active, Ordering::SeqCst) != active {
            let event = if active { SystemEvent::Lock } else { SystemEvent::Unlock };
            if !sink.send(event) {
                break;
            }
        }
    }
    Ok(())
}

impl SessionMonitor {
    pub fn start(sink: EventSink) -> Self {
        let locked = Arc::new(AtomicBool::new(false));
        let logind = Arc::new(AtomicBool::new(false));
        let saver = Arc::new(AtomicBool::new(false));

        let system = Connection::system().ok();
        let session_path = system.as_ref().and_then(|connection| session_path(connection).ok());

        if let Some(connection) = &system {
            let (connection, events) = (connection.clone(), sink.clone());
            logind.store(true, Ordering::SeqCst);
            let flag = logind.clone();
            spawn_listener("logind", move || {
                let result = watch_manager(connection, events);
                flag.store(false, Ordering::SeqCst);
                result
            });
        }

        if let (Some(connection), Some(path)) = (&system, &session_path) {
            if let Ok(proxy) = Proxy::new(connection, LOGIND, path.as_str().to_string(), SESSION) {
                if let Ok(value) = proxy.get_property::<bool>("LockedHint") {
                    locked.store(value, Ordering::SeqCst);
                }
            }
            let (connection, path, state, events) =
                (connection.clone(), path.clone(), locked.clone(), sink.clone());
            spawn_listener("session-lock", move || watch_locked_hint(connection, path, state, events));
        }

        saver.store(Connection::session().is_ok(), Ordering::SeqCst);
        let (state, events) = (locked.clone(), sink);
        spawn_listener("screen-saver", move || watch_screen_saver(state, events));

        Self { locked, logind, saver, system, session_path }
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    /// Time since the session went idle, from `logind`'s `IdleHint`.
    pub fn idle_ms(&self) -> Option<i64> {
        let connection = self.system.as_ref()?;
        let path = self.session_path.as_ref()?;
        let proxy = Proxy::new(connection, LOGIND, path.as_str().to_string(), SESSION).ok()?;
        if !proxy.get_property::<bool>("IdleHint").ok()? {
            return Some(0);
        }
        let since: OwnedValue = proxy.get_property("IdleSinceHint").ok()?;
        let since_us = u64::try_from(since).ok()?;
        Some((now_ms() - (since_us / 1_000) as i64).max(0))
    }

    pub fn lock_capability(&self) -> Capability {
        if self.session_path.is_some() {
            Capability::available()
        } else if self.saver.load(Ordering::SeqCst) {
            Capability::limited("Detected from the screen saver only.")
        } else {
            Capability::unavailable("No lock information is available in this session.")
        }
    }

    pub fn suspend_capability(&self) -> Capability {
        if self.logind.load(Ordering::SeqCst) {
            Capability::available()
        } else {
            Capability::limited("Sleep is detected from gaps in tracking after waking.")
        }
    }
}
