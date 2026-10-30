//! Standard Wayland protocols.
//!
//! `wlr-foreign-toplevel-management` reports the focused application on wlroots
//! compositors and several others. It carries no process ID, so the executable
//! path is unavailable there. `ext-idle-notify` reports inactivity on any
//! compositor that offers it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use log::debug;
use wayland_client::backend::ObjectId;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{ext_idle_notification_v1, ext_idle_notifier_v1};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as toplevel_handle, zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
};

use super::desktop_entries::friendly_name;
use super::{FocusSource, IdleSource};
use crate::tracker::{Capability, EventSink, Foreground, ForegroundApp, SystemEvent};

/// Shorter than the smallest idle threshold, so that it fits any setting without
/// recreating the notification. The real threshold is applied by the engine.
const IDLE_NOTIFY_MS: u32 = 10_000;

#[derive(Default)]
struct Shared {
    focused: Mutex<Option<Toplevel>>,
    idle_since: Mutex<Option<Instant>>,
    has_foreign_toplevel: AtomicBool,
    has_idle_notify: AtomicBool,
    running: AtomicBool,
}

#[derive(Clone, Debug, Default)]
struct Toplevel {
    app_id: String,
    title: String,
    activated: bool,
}

#[derive(Clone)]
pub struct WaylandClient {
    shared: Arc<Shared>,
}

struct State {
    shared: Arc<Shared>,
    sink: EventSink,
    toplevels: HashMap<ObjectId, Toplevel>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_idle_notifier_v1::ExtIdleNotifierV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ext_idle_notifier_v1::ExtIdleNotifierV1,
        _: ext_idle_notifier_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut idle_since = lock(&state.shared.idle_since);
        match event {
            ext_idle_notification_v1::Event::Idled => {
                *idle_since = Instant::now().checked_sub(Duration::from_millis(IDLE_NOTIFY_MS as u64));
            }
            ext_idle_notification_v1::Event::Resumed => *idle_since = None,
            _ => {}
        }
    }
}

impl Dispatch<toplevel_manager::ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &toplevel_manager::ZwlrForeignToplevelManagerV1,
        _: toplevel_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, toplevel_manager::ZwlrForeignToplevelManagerV1, [
        toplevel_manager::EVT_TOPLEVEL_OPCODE => (toplevel_handle::ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<toplevel_handle::ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &toplevel_handle::ZwlrForeignToplevelHandleV1,
        event: toplevel_handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        match event {
            toplevel_handle::Event::AppId { app_id } => {
                state.toplevels.entry(id).or_default().app_id = app_id;
            }
            toplevel_handle::Event::Title { title } => {
                state.toplevels.entry(id).or_default().title = title;
            }
            toplevel_handle::Event::State { state: states } => {
                let activated = states
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|bytes| u32::from_ne_bytes(*bytes) == toplevel_handle::State::Activated as u32);
                state.toplevels.entry(id).or_default().activated = activated;
            }
            toplevel_handle::Event::Done => state.publish(),
            toplevel_handle::Event::Closed => {
                state.toplevels.remove(&id);
                state.publish();
                handle.destroy();
            }
            _ => {}
        }
    }
}

impl State {
    fn publish(&self) {
        let focused = self.toplevels.values().find(|toplevel| toplevel.activated).cloned();
        *lock(&self.shared.focused) = focused;
        self.sink.send(SystemEvent::FocusChanged);
    }
}

impl WaylandClient {
    /// Connects and starts the event thread. Returns `None` when there is no
    /// reachable compositor or it offers neither protocol.
    pub fn start(sink: EventSink) -> Option<Self> {
        let connection = Connection::connect_to_env().ok()?;
        let (globals, mut queue) = registry_queue_init::<State>(&connection).ok()?;
        let handle = queue.handle();

        let shared = Arc::new(Shared::default());
        let manager: Option<toplevel_manager::ZwlrForeignToplevelManagerV1> =
            globals.bind(&handle, 1..=3, ()).ok();
        let seat: Option<wl_seat::WlSeat> = globals.bind(&handle, 1..=1, ()).ok();
        let notifier: Option<ext_idle_notifier_v1::ExtIdleNotifierV1> = globals.bind(&handle, 1..=1, ()).ok();

        let notification = match (&notifier, &seat) {
            (Some(notifier), Some(seat)) => {
                Some(notifier.get_idle_notification(IDLE_NOTIFY_MS, seat, &handle, ()))
            }
            _ => None,
        };

        shared.has_foreign_toplevel.store(manager.is_some(), Ordering::SeqCst);
        shared.has_idle_notify.store(notification.is_some(), Ordering::SeqCst);
        if manager.is_none() && notification.is_none() {
            return None;
        }
        shared.running.store(true, Ordering::SeqCst);

        let thread_shared = shared.clone();
        let spawned = thread::Builder::new().name("daytrace-wayland".into()).spawn(move || {
            let mut state = State { shared: thread_shared.clone(), sink, toplevels: HashMap::new() };
            // These objects must stay alive for their events to arrive.
            let _bound = (manager, seat, notifier, notification);
            while thread_shared.running.load(Ordering::SeqCst) {
                if let Err(error) = queue.blocking_dispatch(&mut state) {
                    debug!("Wayland connection closed: {error}");
                    break;
                }
            }
            thread_shared.running.store(false, Ordering::SeqCst);
        });
        spawned.ok()?;

        Some(Self { shared })
    }

    pub fn has_foreign_toplevel(&self) -> bool {
        self.shared.has_foreign_toplevel.load(Ordering::SeqCst)
    }

    pub fn has_idle_notify(&self) -> bool {
        self.shared.has_idle_notify.load(Ordering::SeqCst)
    }

    fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }
}

pub struct WlrFocus {
    client: WaylandClient,
}

impl WlrFocus {
    pub fn new(client: WaylandClient) -> Self {
        Self { client }
    }
}

impl FocusSource for WlrFocus {
    fn focused(&mut self, include_title: bool) -> Foreground {
        if !self.client.is_running() {
            return Foreground::Unknown;
        }
        let focused = lock(&self.client.shared.focused).clone();
        let Some(toplevel) = focused else { return Foreground::NoFocus };
        if toplevel.app_id.is_empty() {
            return Foreground::Unknown;
        }
        Foreground::App(ForegroundApp {
            identifier: toplevel.app_id.to_lowercase(),
            name: friendly_name(&toplevel.app_id),
            executable_path: None,
            icon_path: None,
            window_title: (include_title && !toplevel.title.is_empty()).then_some(toplevel.title),
        })
    }

    fn capability(&mut self) -> Capability {
        if self.client.is_running() {
            Capability::available()
        } else {
            Capability::unavailable("The connection to the compositor was lost.")
        }
    }

    fn executable_path(&self) -> Capability {
        Capability::unavailable("This Wayland protocol does not expose which process owns a window.")
    }

    fn window_title(&self) -> Capability {
        Capability::available()
    }

    fn backend(&self) -> &'static str {
        "wlr-foreign-toplevel"
    }
}

pub struct NotifyIdle {
    client: WaylandClient,
}

impl NotifyIdle {
    pub fn new(client: WaylandClient) -> Self {
        Self { client }
    }
}

impl IdleSource for NotifyIdle {
    fn idle_ms(&mut self) -> Option<i64> {
        if !self.client.is_running() {
            return None;
        }
        let idle_since = lock(&self.client.shared.idle_since);
        Some(idle_since.map_or(0, |since| since.elapsed().as_millis() as i64))
    }

    fn capability(&mut self) -> Capability {
        if self.client.is_running() {
            Capability::available()
        } else {
            Capability::unavailable("The connection to the compositor was lost.")
        }
    }
}
