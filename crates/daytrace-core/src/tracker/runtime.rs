//! The background thread that turns platform samples into stored sessions.
//!
//! Writes are coalesced: the engine reports every change, and the latest state
//! of each segment is flushed in one transaction. Passing through a window for
//! a moment never reaches the disk, and a crash loses at most one flush
//! interval, because open sessions are closed at their last checkpoint on the
//! next start.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use super::engine::{Activity, AppActivity, Effect, Engine, EngineConfig, Focus, ManualActivity, Segment, Tick};
use super::{ActivityTracker, Capabilities, EventSink, Foreground, ForegroundApp, Sample, SampleOptions, SystemEvent};
use crate::config::Settings;
use crate::database::sessions::{self, TrackedSession};
use crate::database::{applications, categories, exclusions, rules, settings, Database};
use crate::error::Result;
use crate::projects::{Classifier, ExclusionSet, Subject};
use crate::time::now_ms;

const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
const CAPABILITY_REFRESH: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub enum Message {
    System(SystemEvent),
    SettingsChanged,
    RulesChanged,
    Pause { until: Option<i64> },
    Resume,
    StartManual(ManualActivity),
    StopManual,
    /// Writes everything now and answers on the given channel.
    Flush(Sender<()>),
    /// Stored data was deleted or restored, so remembered rows are invalid.
    DataReplaced(Sender<()>),
    Shutdown(Sender<()>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackingState {
    /// Automatic tracking is turned off.
    Disabled,
    Paused,
    Tracking,
    Idle,
    Locked,
    /// A manual timer is running.
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerStatus {
    pub state: TrackingState,
    pub current_application: Option<String>,
    pub paused_until: Option<i64>,
    pub manual_label: Option<String>,
    pub manual_started_at: Option<i64>,
    pub capabilities: Capabilities,
}

/// Handle to the running tracker. Dropping it does not stop the thread: call
/// [`TrackerHandle::shutdown`] so the final state reaches the database.
pub struct TrackerHandle {
    sender: Sender<Message>,
    status: Arc<RwLock<Option<TrackerStatus>>>,
    thread: Option<JoinHandle<()>>,
}

impl TrackerHandle {
    pub fn send(&self, message: Message) {
        if self.sender.send(message).is_err() {
            warn!("tracking runtime is no longer running");
        }
    }

    pub fn status(&self) -> Option<TrackerStatus> {
        self.status.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub fn flush(&self) {
        let (reply, done) = channel();
        self.send(Message::Flush(reply));
        let _ = done.recv_timeout(Duration::from_secs(5));
    }

    pub fn data_replaced(&self) {
        let (reply, done) = channel();
        self.send(Message::DataReplaced(reply));
        let _ = done.recv_timeout(Duration::from_secs(5));
    }

    pub fn shutdown(&mut self) {
        let (reply, done) = channel();
        self.send(Message::Shutdown(reply));
        let _ = done.recv_timeout(Duration::from_secs(5));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub type StatusCallback = Box<dyn Fn(&TrackerStatus) + Send>;

/// Starts tracking on its own thread. `create` receives the event sink and
/// builds the platform backend on that thread.
pub fn spawn(
    database: Arc<Database>,
    create: impl FnOnce(EventSink) -> Box<dyn ActivityTracker> + Send + 'static,
    on_status: StatusCallback,
) -> TrackerHandle {
    let (sender, receiver) = channel();
    let status = Arc::new(RwLock::new(None));
    let sink = EventSink::new(sender.clone());
    let shared_status = status.clone();

    let thread = thread::Builder::new()
        .name("daytrace-tracker".into())
        .spawn(move || {
            let tracker = create(sink);
            match Runtime::new(database, tracker, receiver, shared_status, on_status) {
                Ok(mut runtime) => runtime.run(),
                Err(error) => error!("tracking runtime failed to start: {error}"),
            }
        })
        .expect("failed to start the tracking thread");

    TrackerHandle { sender, status, thread: Some(thread) }
}

#[derive(Default)]
struct KnownApps {
    ids: HashMap<String, i64>,
}

/// Remembers the last classification so that a sample with the same
/// application and title costs no rule evaluation or database access.
#[derive(Clone)]
struct CachedFocus {
    app: ForegroundApp,
    focus: Focus,
}

struct PendingWrite {
    segment: Segment,
    open: bool,
}

struct Runtime {
    database: Arc<Database>,
    tracker: Box<dyn ActivityTracker>,
    receiver: Receiver<Message>,
    shared_status: Arc<RwLock<Option<TrackerStatus>>>,
    on_status: StatusCallback,

    settings: Settings,
    classifier: Classifier,
    exclusions: ExclusionSet,
    engine: Engine,
    capabilities: Capabilities,

    known_apps: KnownApps,
    cached_focus: Option<CachedFocus>,
    current_app_name: Option<String>,
    manual_started_at: Option<i64>,
    paused_until: Option<i64>,
    paused: bool,
    locked: bool,

    pending: BTreeMap<u64, Option<PendingWrite>>,
    rows: HashMap<u64, i64>,
    last_flush: Instant,
    last_capability_check: Instant,
    last_status: Option<TrackerStatus>,
}

impl Runtime {
    fn new(
        database: Arc<Database>,
        mut tracker: Box<dyn ActivityTracker>,
        receiver: Receiver<Message>,
        shared_status: Arc<RwLock<Option<TrackerStatus>>>,
        on_status: StatusCallback,
    ) -> Result<Self> {
        let recovered = database.write(sessions::recover_open)?;
        if recovered > 0 {
            info!("closed {recovered} sessions left open by an unclean exit");
        }

        let settings = database.read(settings::load)?;
        let engine = Engine::new(engine_config(&settings));
        let capabilities = tracker.capabilities();
        let mut runtime = Self {
            database,
            tracker,
            receiver,
            shared_status,
            on_status,
            settings,
            classifier: Classifier::default(),
            exclusions: ExclusionSet::default(),
            engine,
            capabilities,
            known_apps: KnownApps::default(),
            cached_focus: None,
            current_app_name: None,
            manual_started_at: None,
            paused_until: None,
            paused: false,
            locked: false,
            pending: BTreeMap::new(),
            rows: HashMap::new(),
            last_flush: Instant::now(),
            last_capability_check: Instant::now(),
            last_status: None,
        };
        runtime.reload_rules()?;
        Ok(runtime)
    }

    fn run(&mut self) {
        self.publish_status();
        let mut next_sample = Instant::now();
        loop {
            let timeout = next_sample.saturating_duration_since(Instant::now());
            match self.receiver.recv_timeout(timeout) {
                Ok(message) => {
                    if !self.handle(message) {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.step();
                    next_sample = Instant::now() + self.poll_interval();
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.finish();
                    return;
                }
            }

            if self.last_flush.elapsed() >= FLUSH_INTERVAL {
                self.checkpoint();
            }
            if self.last_capability_check.elapsed() >= CAPABILITY_REFRESH {
                self.last_capability_check = Instant::now();
                self.capabilities = self.tracker.capabilities();
                self.publish_status();
            }
        }
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.settings.poll_interval_secs as u64)
    }

    fn handle(&mut self, message: Message) -> bool {
        match message {
            Message::System(event) => self.handle_system(event),
            Message::SettingsChanged => self.reload_settings(),
            Message::RulesChanged => {
                if let Err(error) = self.reload_rules() {
                    error!("failed to reload classification rules: {error}");
                }
                self.step();
            }
            Message::Pause { until } => {
                self.paused = true;
                self.paused_until = until;
                self.stop_tracking();
                self.publish_status();
            }
            Message::Resume => {
                self.resume();
            }
            Message::StartManual(manual) => {
                self.manual_started_at = Some(now_ms());
                let effects = self.engine.start_manual(now_ms(), manual);
                self.record(effects);
                self.write_pending();
                self.publish_status();
            }
            Message::StopManual => {
                self.manual_started_at = None;
                let effects = self.engine.stop_manual(now_ms());
                self.record(effects);
                self.write_pending();
                self.step();
            }
            Message::Flush(reply) => {
                self.checkpoint();
                let _ = reply.send(());
            }
            Message::DataReplaced(reply) => {
                self.stop_tracking();
                self.rows.clear();
                self.known_apps = KnownApps::default();
                self.cached_focus = None;
                self.reload_settings();
                let _ = reply.send(());
            }
            Message::Shutdown(reply) => {
                self.finish();
                let _ = reply.send(());
                return false;
            }
        }
        true
    }

    fn handle_system(&mut self, event: SystemEvent) {
        match event {
            SystemEvent::FocusChanged => self.step(),
            SystemEvent::Lock => {
                self.locked = true;
                self.step();
            }
            SystemEvent::Unlock => {
                self.locked = false;
                self.step();
            }
            SystemEvent::Suspend | SystemEvent::Shutdown => {
                self.manual_started_at = None;
                self.stop_tracking();
                self.publish_status();
            }
            SystemEvent::Resume => {
                self.locked = false;
                self.step();
            }
        }
    }

    fn resume(&mut self) {
        self.paused = false;
        self.paused_until = None;
        self.step();
    }

    /// Takes one sample and advances the engine.
    fn step(&mut self) {
        if self.paused_until.is_some_and(|until| now_ms() >= until) {
            self.paused = false;
            self.paused_until = None;
        }

        let now = now_ms();
        if self.engine.manual().is_some() {
            let tick = Tick { now, focus: Focus::Excluded, idle_ms: None, locked: false };
            let effects = self.engine.tick(tick);
            self.record(effects);
        } else if self.is_tracking() {
            let options = SampleOptions { include_window_title: self.settings.collect_window_titles };
            let sample = self.tracker.sample(options);
            self.apply_sample(now, sample);
        }
        self.publish_status();
    }

    fn is_tracking(&self) -> bool {
        self.settings.tracking_enabled && !self.paused
    }

    fn apply_sample(&mut self, now: i64, sample: Sample) {
        let has_idle = sample.idle_ms.is_some();
        let focus = match sample.foreground {
            Foreground::App(app) => self.resolve(app),
            Foreground::NoFocus => Focus::Transient,
            // Without idle detection there is no evidence the user is present,
            // so claiming "active but unidentified" would be a guess.
            Foreground::Unknown if has_idle => {
                self.current_app_name = None;
                Focus::Unknown
            }
            Foreground::Unknown => {
                self.current_app_name = None;
                Focus::Excluded
            }
        };
        let locked = sample.locked || self.locked;
        let effects = self.engine.tick(Tick { now, focus, idle_ms: sample.idle_ms, locked });
        self.record(effects);
    }

    /// Applies privacy exclusions and classification. Excluded applications
    /// return before the database is touched, so no trace of them is stored.
    fn resolve(&mut self, app: ForegroundApp) -> Focus {
        if let Some(cached) = &self.cached_focus {
            if cached.app == app {
                return cached.focus.clone();
            }
        }

        let focus = self.classify(&app);
        self.current_app_name = match focus {
            Focus::Excluded => None,
            _ => Some(app.name.clone()),
        };
        self.cached_focus = Some(CachedFocus { app, focus: focus.clone() });
        focus
    }

    fn classify(&mut self, app: &ForegroundApp) -> Focus {
        if self.exclusions.excludes_application(&app.identifier, &app.name, app.executable_path.as_deref()) {
            return Focus::Excluded;
        }

        let application_id = match self.known_apps.ids.get(&app.identifier) {
            Some(id) => *id,
            None => {
                let stored = self.database.write(|connection| {
                    applications::upsert(
                        connection,
                        &app.identifier,
                        &app.name,
                        app.executable_path.as_deref(),
                        app.icon_path.as_deref(),
                    )
                });
                match stored {
                    Ok(id) => {
                        self.known_apps.ids.insert(app.identifier.clone(), id);
                        id
                    }
                    Err(error) => {
                        error!("failed to store application: {error}");
                        return Focus::Unknown;
                    }
                }
            }
        };

        let subject = Subject {
            identifier: &app.identifier,
            name: &app.name,
            display_name: None,
            executable_path: app.executable_path.as_deref(),
            window_title: app.window_title.as_deref(),
            domain: None,
        };
        let classification = self.classifier.classify(Some(application_id), &subject);
        if self.exclusions.excludes_project(classification.project_id) {
            return Focus::Excluded;
        }

        Focus::Activity(AppActivity {
            application_id,
            project_id: classification.project_id,
            category_id: classification.category_id,
            window_title: app.window_title.clone(),
        })
    }

    fn stop_tracking(&mut self) {
        let effects = self.engine.stop(now_ms());
        self.record(effects);
        self.current_app_name = None;
        self.write_pending();
    }

    fn finish(&mut self) {
        self.stop_tracking();
    }

    fn record(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Save { segment, open } => {
                    self.pending.insert(segment.id, Some(PendingWrite { segment, open }));
                }
                Effect::Remove { id } => {
                    self.pending.insert(id, None);
                }
            }
        }
    }

    fn checkpoint(&mut self) {
        self.write_pending();
        self.last_flush = Instant::now();
    }

    fn write_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let mut rows = std::mem::take(&mut self.rows);

        let result = self.database.write(|connection| {
            for (id, write) in &pending {
                match write {
                    Some(write) => {
                        let session = tracked_session(&write.segment, write.open);
                        match rows.get(id) {
                            Some(row) => sessions::update_tracked(connection, *row, &session)?,
                            None => {
                                rows.insert(*id, sessions::insert_tracked(connection, &session)?);
                            }
                        }
                    }
                    None => {
                        if let Some(row) = rows.remove(id) {
                            sessions::remove_tracked(connection, row)?;
                        }
                    }
                }
            }
            Ok(())
        });

        match result {
            Ok(()) => {
                // Only the two newest segments can still change.
                let mut ids: Vec<u64> = rows.keys().copied().collect();
                ids.sort_unstable();
                let keep_from = ids.iter().rev().nth(1).copied().unwrap_or(0);
                rows.retain(|id, _| *id >= keep_from);
                self.rows = rows;
            }
            Err(error) => {
                error!("failed to write sessions: {error}");
                self.pending = pending;
                self.rows = rows;
            }
        }
    }

    fn reload_settings(&mut self) {
        match self.database.read(settings::load) {
            Ok(settings) => {
                let tracking_turned_off = self.settings.tracking_enabled && !settings.tracking_enabled;
                let titles_changed = self.settings.collect_window_titles != settings.collect_window_titles;
                self.engine.configure(engine_config(&settings));
                self.settings = settings;
                if titles_changed {
                    self.cached_focus = None;
                }
                if tracking_turned_off {
                    self.stop_tracking();
                }
                self.step();
            }
            Err(error) => error!("failed to reload settings: {error}"),
        }
    }

    fn reload_rules(&mut self) -> Result<()> {
        let (classifier, exclusions) = self.database.read(|connection| {
            let classifier = Classifier::new(
                rules::list(connection)?,
                &applications::list(connection)?,
                &categories::list(connection)?,
            );
            Ok((classifier, ExclusionSet::new(&exclusions::list(connection)?)))
        })?;
        self.classifier = classifier;
        self.exclusions = exclusions;
        self.cached_focus = None;
        Ok(())
    }

    fn state(&self) -> TrackingState {
        if self.engine.manual().is_some() {
            return TrackingState::Manual;
        }
        if !self.settings.tracking_enabled {
            return TrackingState::Disabled;
        }
        if self.paused {
            return TrackingState::Paused;
        }
        match self.engine.current().map(|segment| &segment.activity) {
            Some(Activity::Idle) => TrackingState::Idle,
            Some(Activity::Locked) => TrackingState::Locked,
            _ => TrackingState::Tracking,
        }
    }

    fn publish_status(&mut self) {
        let state = self.state();
        let manual = self.engine.manual();
        let status = TrackerStatus {
            state,
            current_application: match state {
                TrackingState::Tracking => self.current_app_name.clone(),
                _ => None,
            },
            paused_until: self.paused_until,
            manual_label: manual.and_then(|manual| manual.label.clone()),
            manual_started_at: self.manual_started_at,
            capabilities: self.capabilities.clone(),
        };
        if self.last_status.as_ref() == Some(&status) {
            return;
        }
        *self.shared_status.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(status.clone());
        (self.on_status)(&status);
        self.last_status = Some(status);
    }
}

fn engine_config(settings: &Settings) -> EngineConfig {
    EngineConfig::new(
        settings.idle_threshold_secs as i64 * 1_000,
        settings.poll_interval_secs as i64 * 1_000,
    )
}

fn tracked_session(segment: &Segment, open: bool) -> TrackedSession {
    let mut session = TrackedSession {
        application_id: None,
        project_id: None,
        category_id: None,
        start_time: segment.start,
        end_time: segment.end,
        session_type: segment.activity.session_type(),
        window_title: None,
        label: None,
        manual_override: false,
        is_open: open,
    };
    match &segment.activity {
        Activity::App(app) => {
            session.application_id = Some(app.application_id);
            session.project_id = app.project_id;
            session.category_id = app.category_id;
            session.window_title = app.window_title.clone();
        }
        Activity::Manual(manual) => {
            session.project_id = manual.project_id;
            session.category_id = manual.category_id;
            session.label = manual.label.clone();
            session.manual_override = true;
        }
        Activity::Unknown | Activity::Idle | Activity::Locked => {}
    }
    session
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::sessions::list;
    use crate::database::test_support::database as open_database;
    use crate::models::{ExclusionKind, Session, SessionFilter, SessionType};
    use crate::tracker::{Capability, EventSink};
    use std::sync::Mutex;

    /// A backend whose answers the test scripts.
    #[derive(Clone)]
    struct Scripted(Arc<Mutex<Sample>>);

    impl Scripted {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Sample { foreground: Foreground::NoFocus, idle_ms: Some(0), locked: false })))
        }

        fn focus(&self, name: &str) {
            self.0.lock().unwrap().foreground = Foreground::App(ForegroundApp {
                identifier: name.to_lowercase(),
                name: name.into(),
                ..Default::default()
            });
        }

        fn set_unknown(&self) {
            self.0.lock().unwrap().foreground = Foreground::Unknown;
        }

        fn set_idle(&self, idle_ms: Option<i64>) {
            self.0.lock().unwrap().idle_ms = idle_ms;
        }
    }

    impl ActivityTracker for Scripted {
        fn capabilities(&mut self) -> Capabilities {
            let available = Capability::available;
            Capabilities {
                platform: "test".into(),
                display_server: "test".into(),
                desktop: "test".into(),
                backend: "scripted".into(),
                foreground_app: available(),
                executable_path: available(),
                window_title: available(),
                idle: available(),
                lock: available(),
                suspend: available(),
                integration: None,
            }
        }

        fn sample(&mut self, _options: SampleOptions) -> Sample {
            self.0.lock().unwrap().clone()
        }
    }

    fn enable_tracking(database: &Database) {
        let settings = Settings { tracking_enabled: true, ..Settings::default() };
        database.write(|c| settings::save(c, &settings)).unwrap();
    }

    fn start(database: &Arc<Database>, backend: &Scripted) -> TrackerHandle {
        let backend = backend.clone();
        spawn(database.clone(), move |_: EventSink| Box::new(backend), Box::new(|_| {}))
    }

    fn sessions_of(database: &Database) -> Vec<Session> {
        let filter = SessionFilter { from: 0, to: i64::MAX, ..Default::default() };
        database.read(|c| list(c, &filter)).unwrap()
    }

    fn wake(handle: &TrackerHandle) {
        handle.send(Message::System(SystemEvent::FocusChanged));
    }

    #[test]
    fn nothing_is_recorded_without_consent() {
        let (_dir, database) = open_database();
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.focus("Firefox");

        let mut handle = start(&database, &backend);
        wake(&handle);
        handle.flush();
        handle.shutdown();

        assert!(sessions_of(&database).is_empty());
        assert!(database.read(applications::list).unwrap().is_empty());
    }

    #[test]
    fn samples_become_closed_sessions_on_shutdown() {
        let (_dir, database) = open_database();
        enable_tracking(&database);
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.focus("Firefox");

        let mut handle = start(&database, &backend);
        wake(&handle);
        thread::sleep(Duration::from_millis(30));
        wake(&handle);
        handle.flush();
        assert!(sessions_of(&database)[0].is_open);
        handle.shutdown();

        let stored = sessions_of(&database);
        assert_eq!(stored.len(), 1);
        assert!(!stored[0].is_open);
        assert_eq!(stored[0].application_name.as_deref(), Some("Firefox"));
    }

    #[test]
    fn excluded_applications_are_never_stored() {
        let (_dir, database) = open_database();
        enable_tracking(&database);
        database.write(|c| exclusions::add(c, ExclusionKind::Application, "KeePassXC")).unwrap();
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.focus("KeePassXC");

        let mut handle = start(&database, &backend);
        wake(&handle);
        handle.flush();
        handle.shutdown();

        assert!(sessions_of(&database).is_empty());
        assert!(database.read(applications::list).unwrap().is_empty());
    }

    #[test]
    fn unknown_focus_with_idle_detection_records_unidentified_activity() {
        let (_dir, database) = open_database();
        enable_tracking(&database);
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.set_unknown();

        let mut handle = start(&database, &backend);
        wake(&handle);
        thread::sleep(Duration::from_millis(20));
        wake(&handle);
        handle.shutdown();

        let stored = sessions_of(&database);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].session_type, SessionType::Unknown);
        assert_eq!(stored[0].application_id, None);
    }

    #[test]
    fn no_evidence_of_presence_records_nothing() {
        let (_dir, database) = open_database();
        enable_tracking(&database);
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.set_unknown();
        backend.set_idle(None);

        let mut handle = start(&database, &backend);
        wake(&handle);
        handle.shutdown();

        assert!(sessions_of(&database).is_empty());
    }

    #[test]
    fn manual_timers_work_without_automatic_tracking() {
        let (_dir, database) = open_database();
        let database = Arc::new(database);
        let backend = Scripted::new();

        let mut handle = start(&database, &backend);
        handle.send(Message::StartManual(ManualActivity {
            label: Some("Reading".into()),
            project_id: None,
            category_id: None,
        }));
        thread::sleep(Duration::from_millis(20));
        handle.send(Message::StopManual);
        handle.shutdown();

        let stored = sessions_of(&database);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].session_type, SessionType::Manual);
        assert_eq!(stored[0].label.as_deref(), Some("Reading"));
        assert!(stored[0].manual_override);
    }

    #[test]
    fn pausing_stops_recording() {
        let (_dir, database) = open_database();
        enable_tracking(&database);
        let database = Arc::new(database);
        let backend = Scripted::new();
        backend.focus("Firefox");

        let mut handle = start(&database, &backend);
        wake(&handle);
        thread::sleep(Duration::from_millis(20));
        handle.send(Message::Pause { until: None });
        handle.flush();
        let before = sessions_of(&database);
        wake(&handle);
        thread::sleep(Duration::from_millis(20));
        wake(&handle);
        handle.shutdown();

        assert_eq!(sessions_of(&database).len(), before.len());
        assert_eq!(handle.status().unwrap().state, TrackingState::Paused);
    }

    #[test]
    fn open_sessions_are_recovered_after_a_crash() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.db");
        {
            let database = Database::open(&path).unwrap();
            enable_tracking(&database);
            let database = Arc::new(database);
            let backend = Scripted::new();
            backend.focus("Firefox");
            let handle = start(&database, &backend);
            wake(&handle);
            thread::sleep(Duration::from_millis(20));
            wake(&handle);
            handle.flush();
            // The handle is dropped without shutdown, as in a crash.
            drop(handle);
        }

        let database = Arc::new(Database::open(&path).unwrap());
        let backend = Scripted::new();
        let mut handle = start(&database, &backend);
        handle.shutdown();
        let stored = sessions_of(&database);
        assert_eq!(stored.len(), 1);
        assert!(!stored[0].is_open);
    }

    #[test]
    fn events_from_backends_arrive_through_the_sink() {
        let (sender, receiver) = channel();
        let sink = EventSink::new(sender);
        assert!(sink.send(SystemEvent::Suspend));
        assert!(matches!(receiver.recv().unwrap(), Message::System(SystemEvent::Suspend)));
        drop(receiver);
        assert!(!sink.send(SystemEvent::Resume));
    }
}
