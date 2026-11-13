//! Lock, suspend, shutdown and idle state from `logind` and screen savers.
//!
//! Every source is optional. Confined packages may be refused access to the
//! system bus, in which case the capability report says so and suspend is still
//! caught by the engine's tick-gap detection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use log::{debug, warn};
use zbus::blocking::{proxy, Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::MatchRule;

use crate::time::now_ms;
use crate::tracker::{Capability, EventSink, SystemEvent};

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
const SCREEN_SAVERS: [&str; 2] = ["org.gnome.ScreenSaver", "org.freedesktop.ScreenSaver"];

#[derive(Clone)]
pub struct SessionMonitor {
    locked: Arc<AtomicBool>,
    logind: Arc<AtomicBool>,
    saver: Arc<AtomicBool>,
    /// Whether `logind` reports this session's lock state. When it does, it
    /// is the only source; the screen saver is a fallback for sessions without it.
    locked_hint: Arc<AtomicBool>,
    system: Option<Connection>,
    session_path: Option<OwnedObjectPath>,
}

/// A proxy that asks `logind` every time. The default property cache is
/// updated separately from the signals this module reads, so a value read
/// right after a change could still be the old one.
fn session_proxy<'a>(connection: &Connection, path: &OwnedObjectPath) -> zbus::Result<Proxy<'a>> {
    proxy::Builder::new(connection)
        .destination(LOGIND)?
        .path(path.as_str().to_string())?
        .interface(SESSION)?
        .cache_properties(CacheProperties::No)
        .build()
}

fn read_locked_hint(connection: &Connection, path: &OwnedObjectPath) -> Option<bool> {
    session_proxy(connection, path).ok()?.get_property("LockedHint").ok()
}

/// What a `PropertiesChanged` signal says about `LockedHint`: its new value,
/// that it changed without a value, or nothing at all.
#[derive(Debug, PartialEq, Eq)]
enum HintChange {
    Value(bool),
    Invalidated,
    Unrelated,
}

fn locked_hint_change(interface: &str, changed: &HashMap<String, OwnedValue>, invalidated: &[String]) -> HintChange {
    if interface != SESSION {
        return HintChange::Unrelated;
    }
    if let Some(value) = changed.get("LockedHint") {
        return match bool::try_from(value) {
            Ok(value) => HintChange::Value(value),
            Err(_) => HintChange::Invalidated,
        };
    }
    if invalidated.iter().any(|name| name == "LockedHint") {
        HintChange::Invalidated
    } else {
        HintChange::Unrelated
    }
}

/// Records a new lock state and reports a change, if there was one.
fn publish(locked: &AtomicBool, value: bool, sink: &EventSink) -> bool {
    if locked.swap(value, Ordering::SeqCst) == value {
        return true;
    }
    sink.send(if value { SystemEvent::Lock } else { SystemEvent::Unlock })
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
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        let message = message?;
        let Ok((interface, changed, invalidated)) =
            message.body().deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
        else {
            continue;
        };
        let value = match locked_hint_change(&interface, &changed, &invalidated) {
            HintChange::Value(value) => value,
            HintChange::Invalidated => match read_locked_hint(&connection, &path) {
                Some(value) => value,
                None => continue,
            },
            HintChange::Unrelated => continue,
        };
        if !publish(&locked, value, &sink) {
            break;
        }
    }
    Ok(())
}

fn watch_screen_saver(locked: Arc<AtomicBool>, locked_hint: Arc<AtomicBool>, sink: EventSink) -> zbus::Result<()> {
    let connection = Connection::session()?;
    let rule = MatchRule::builder().msg_type(zbus::message::Type::Signal).member("ActiveChanged")?.build();
    for message in MessageIterator::for_match_rule(rule, &connection, None)? {
        let message = message?;
        let header = message.header();
        let from_screen_saver = header.interface().is_some_and(|name| SCREEN_SAVERS.contains(&name.as_str()));
        // Two sources writing one flag could undo each other's unlock.
        if !from_screen_saver || locked_hint.load(Ordering::SeqCst) {
            continue;
        }
        let Ok(active) = message.body().deserialize::<bool>() else { continue };
        if !publish(&locked, active, &sink) {
            break;
        }
    }
    Ok(())
}

impl SessionMonitor {
    pub fn start(sink: EventSink) -> Self {
        let locked = Arc::new(AtomicBool::new(false));
        let logind = Arc::new(AtomicBool::new(false));
        let saver = Arc::new(AtomicBool::new(false));
        let locked_hint = Arc::new(AtomicBool::new(false));

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
            if let Some(value) = read_locked_hint(connection, path) {
                locked.store(value, Ordering::SeqCst);
                locked_hint.store(true, Ordering::SeqCst);
            }
            let (connection, path, state, events) =
                (connection.clone(), path.clone(), locked.clone(), sink.clone());
            spawn_listener("session-lock", move || watch_locked_hint(connection, path, state, events));
        }

        saver.store(Connection::session().is_ok(), Ordering::SeqCst);
        let (state, hint, events) = (locked.clone(), locked_hint.clone(), sink);
        spawn_listener("screen-saver", move || watch_screen_saver(state, hint, events));

        Self { locked, logind, saver, locked_hint, system, session_path }
    }

    /// Asks `logind` on every sample when it can, so that a missed signal
    /// corrects itself at the next poll instead of lasting until the next lock.
    pub fn is_locked(&self) -> bool {
        if self.locked_hint.load(Ordering::SeqCst) {
            if let (Some(connection), Some(path)) = (&self.system, &self.session_path) {
                if let Some(value) = read_locked_hint(connection, path) {
                    self.locked.store(value, Ordering::SeqCst);
                    return value;
                }
            }
        }
        self.locked.load(Ordering::SeqCst)
    }

    /// Time since the session went idle, from `logind`'s `IdleHint`.
    pub fn idle_ms(&self) -> Option<i64> {
        let connection = self.system.as_ref()?;
        let path = self.session_path.as_ref()?;
        let proxy = session_proxy(connection, path).ok()?;
        if !proxy.get_property::<bool>("IdleHint").ok()? {
            return Some(0);
        }
        let since: OwnedValue = proxy.get_property("IdleSinceHint").ok()?;
        let since_us = u64::try_from(since).ok()?;
        Some((now_ms() - (since_us / 1_000) as i64).max(0))
    }

    pub fn lock_capability(&self) -> Capability {
        if self.locked_hint.load(Ordering::SeqCst) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn changed(entries: &[(&str, Value<'_>)]) -> HashMap<String, OwnedValue> {
        entries.iter().map(|(name, value)| (name.to_string(), value.try_to_owned().unwrap())).collect()
    }

    #[test]
    fn the_new_value_is_read_from_the_signal() {
        let unlocked = changed(&[("LockedHint", Value::from(false))]);
        assert_eq!(locked_hint_change(SESSION, &unlocked, &[]), HintChange::Value(false));
        let locked = changed(&[("LockedHint", Value::from(true)), ("IdleHint", Value::from(true))]);
        assert_eq!(locked_hint_change(SESSION, &locked, &[]), HintChange::Value(true));
    }

    #[test]
    fn other_properties_leave_the_lock_state_alone() {
        let idle = changed(&[("IdleHint", Value::from(true))]);
        assert_eq!(locked_hint_change(SESSION, &idle, &[]), HintChange::Unrelated);
        let other = changed(&[("LockedHint", Value::from(true))]);
        assert_eq!(locked_hint_change("org.freedesktop.login1.User", &other, &[]), HintChange::Unrelated);
    }

    #[test]
    fn an_invalidated_hint_is_read_again() {
        let invalidated = vec!["LockedHint".to_string()];
        assert_eq!(locked_hint_change(SESSION, &HashMap::new(), &invalidated), HintChange::Invalidated);
    }
}
