//! The session state machine. It holds no I/O and reads no clock: every input
//! carries its timestamp, so all session rules are testable in simulated time.

use crate::models::SessionType;

pub type Ts = i64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppActivity {
    pub application_id: i64,
    pub category_id: Option<i64>,
    pub window_title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualActivity {
    pub label: Option<String>,
    pub category_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activity {
    App(AppActivity),
    Unknown,
    Idle,
    Locked,
    Manual(ManualActivity),
}

impl Activity {
    pub fn session_type(&self) -> SessionType {
        match self {
            Self::App(_) => SessionType::Active,
            Self::Unknown => SessionType::Unknown,
            Self::Idle => SessionType::Idle,
            Self::Locked => SessionType::Locked,
            Self::Manual(_) => SessionType::Manual,
        }
    }

    /// Whether two activities belong in one session. The window title is
    /// deliberately ignored, so that browsing does not shatter into a session
    /// per page. A title change that alters the classification still splits.
    fn continues(&self, other: &Activity) -> bool {
        match (self, other) {
            (Self::App(a), Self::App(b)) => {
                (a.application_id, a.category_id) == (b.application_id, b.category_id)
            }
            _ => self == other,
        }
    }

    fn can_be_blip(&self) -> bool {
        matches!(self, Self::App(_) | Self::Unknown)
    }
}

/// What the platform says has focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Focus {
    Activity(AppActivity),
    /// The user is present but the application cannot be identified.
    Unknown,
    /// An excluded application has focus. Nothing may be recorded for it.
    Excluded,
    /// Nothing is focused right now, for example during a window switch.
    Transient,
}

#[derive(Clone, Debug)]
pub struct Tick {
    pub now: Ts,
    pub focus: Focus,
    /// Time since the last input, when the platform can report it.
    pub idle_ms: Option<i64>,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub idle_threshold_ms: i64,
    /// Activity shorter than this is merged away, such as passing through an
    /// application while switching windows.
    pub blip_ms: i64,
    /// A longer silence between ticks means the process did not run, for
    /// example because the machine slept without a suspend event.
    pub max_tick_gap_ms: i64,
}

impl EngineConfig {
    pub fn new(idle_threshold_ms: i64, poll_ms: i64) -> Self {
        Self { idle_threshold_ms, blip_ms: 2_000, max_tick_gap_ms: (poll_ms * 3).max(30_000) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub id: u64,
    pub activity: Activity,
    pub start: Ts,
    pub end: Ts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Save { segment: Segment, open: bool },
    Remove { id: u64 },
}

#[derive(Debug)]
pub struct Engine {
    config: EngineConfig,
    current: Option<Segment>,
    /// The last closed segment, kept while it can still absorb a blip.
    previous: Option<Segment>,
    manual: Option<ManualActivity>,
    last_focus: Option<Activity>,
    last_tick: Option<Ts>,
    next_id: u64,
    effects: Vec<Effect>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            current: None,
            previous: None,
            manual: None,
            last_focus: None,
            last_tick: None,
            next_id: 1,
            effects: Vec::new(),
        }
    }

    pub fn configure(&mut self, config: EngineConfig) {
        self.config = config;
    }

    pub fn current(&self) -> Option<&Segment> {
        self.current.as_ref()
    }

    pub fn manual(&self) -> Option<&ManualActivity> {
        self.manual.as_ref()
    }

    pub fn tick(&mut self, tick: Tick) -> Vec<Effect> {
        if let Some(last) = self.last_tick {
            if tick.now < last || tick.now - last > self.config.max_tick_gap_ms {
                self.finish(last);
            }
        }
        self.last_tick = Some(tick.now);

        let (next, at) = self.desired(&tick);
        self.transition(next, at, tick.now);
        std::mem::take(&mut self.effects)
    }

    /// Ends all activity at `at`, for suspend, shutdown or pausing. Nothing is
    /// recorded until the next tick.
    pub fn stop(&mut self, at: Ts) -> Vec<Effect> {
        self.finish(at);
        self.manual = None;
        self.last_tick = None;
        self.last_focus = None;
        std::mem::take(&mut self.effects)
    }

    pub fn start_manual(&mut self, now: Ts, manual: ManualActivity) -> Vec<Effect> {
        self.manual = Some(manual.clone());
        self.transition(Some(Activity::Manual(manual)), now, now);
        self.last_tick = Some(now);
        std::mem::take(&mut self.effects)
    }

    pub fn stop_manual(&mut self, now: Ts) -> Vec<Effect> {
        self.manual = None;
        self.transition(None, now, now);
        std::mem::take(&mut self.effects)
    }

    /// The state the user is in, and when that state began.
    fn desired(&mut self, tick: &Tick) -> (Option<Activity>, Ts) {
        if let Some(manual) = &self.manual {
            return (Some(Activity::Manual(manual.clone())), tick.now);
        }
        if tick.locked {
            return (Some(Activity::Locked), tick.now);
        }

        let idle_ms = tick.idle_ms.unwrap_or(0).max(0);
        if idle_ms >= self.config.idle_threshold_ms {
            return (Some(Activity::Idle), tick.now - idle_ms);
        }

        let focused = match &tick.focus {
            Focus::Activity(activity) => Some(Activity::App(activity.clone())),
            Focus::Unknown => Some(Activity::Unknown),
            Focus::Excluded => None,
            Focus::Transient => self.last_focus.clone(),
        };
        if !matches!(tick.focus, Focus::Transient) {
            self.last_focus = focused.clone();
        }

        let resumed = self.current.as_ref().is_some_and(|c| c.activity == Activity::Idle);
        (focused, if resumed { tick.now - idle_ms } else { tick.now })
    }

    fn transition(&mut self, next: Option<Activity>, at: Ts, now: Ts) {
        let Some(mut current) = self.current.take() else {
            if let Some(activity) = next {
                self.open(activity, at, now);
            }
            return;
        };

        if next.as_ref().is_some_and(|next| current.activity.continues(next)) {
            current.end = current.end.max(now);
            self.save(current.clone(), true);
            self.current = Some(current);
            return;
        }

        let boundary = at.clamp(current.start, now.max(current.start));
        current.end = boundary;

        match next {
            Some(next)
                if current.activity.can_be_blip() && boundary - current.start < self.config.blip_ms =>
            {
                self.effects.push(Effect::Remove { id: current.id });
                self.open(next, current.start, now);
            }
            next => {
                self.close(current);
                if let Some(activity) = next {
                    self.open(activity, boundary, now);
                }
            }
        }
    }

    /// Opens a segment, or continues the previous one when it is the same
    /// activity and directly adjacent.
    fn open(&mut self, activity: Activity, start: Ts, now: Ts) {
        if let Some(mut previous) = self.previous.take() {
            if previous.end == start && previous.activity.continues(&activity) {
                previous.end = previous.end.max(now);
                self.save(previous.clone(), true);
                self.current = Some(previous);
                return;
            }
            self.previous = Some(previous);
        }

        let floor = self.previous.as_ref().map_or(start, |previous| previous.end);
        let start = start.max(floor).min(now);
        let id = self.next_id;
        self.next_id += 1;
        let segment = Segment { id, activity, start, end: now };
        self.save(segment.clone(), true);
        self.current = Some(segment);
    }

    fn close(&mut self, segment: Segment) {
        if segment.end <= segment.start {
            self.effects.push(Effect::Remove { id: segment.id });
            return;
        }
        self.save(segment.clone(), false);
        self.previous = Some(segment);
    }

    fn finish(&mut self, at: Ts) {
        if let Some(mut current) = self.current.take() {
            current.end = current.end.min(at).max(current.start);
            self.close(current);
        }
        self.previous = None;
    }

    fn save(&mut self, segment: Segment, open: bool) {
        self.effects.push(Effect::Save { segment, open });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const SECOND: i64 = 1_000;
    const MINUTE: i64 = 60 * SECOND;

    fn engine() -> Engine {
        Engine::new(EngineConfig::new(5 * MINUTE, 5 * SECOND))
    }

    fn app(id: i64) -> Focus {
        Focus::Activity(AppActivity { application_id: id, category_id: None, window_title: None })
    }

    fn tick(now: Ts, focus: Focus, idle_ms: i64) -> Tick {
        Tick { now, focus, idle_ms: Some(idle_ms), locked: false }
    }

    /// Replays effects the way the runtime persists them.
    #[derive(Default)]
    struct Store(BTreeMap<u64, (Segment, bool)>);

    impl Store {
        fn apply(&mut self, effects: Vec<Effect>) {
            for effect in effects {
                match effect {
                    Effect::Save { segment, open } => {
                        self.0.insert(segment.id, (segment, open));
                    }
                    Effect::Remove { id } => {
                        self.0.remove(&id);
                    }
                }
            }
        }

        fn segments(&self) -> Vec<Segment> {
            let mut segments: Vec<_> = self.0.values().map(|(segment, _)| segment.clone()).collect();
            segments.sort_by_key(|segment| segment.start);
            segments
        }

        fn summary(&self) -> Vec<(SessionType, Ts, Ts)> {
            self.segments()
                .iter()
                .map(|segment| (segment.activity.session_type(), segment.start, segment.end))
                .collect()
        }

        fn assert_valid(&self) {
            let segments = self.segments();
            for segment in &segments {
                assert!(segment.end >= segment.start, "negative duration: {segment:?}");
            }
            for pair in segments.windows(2) {
                assert!(pair[0].end <= pair[1].start, "overlap: {pair:?}");
            }
            let open = self.0.values().filter(|(_, open)| *open).count();
            assert!(open <= 1, "more than one open segment");
        }
    }

    fn run(engine: &mut Engine, ticks: Vec<Tick>) -> Store {
        let mut store = Store::default();
        for tick in ticks {
            let effects = engine.tick(tick);
            store.apply(effects);
        }
        store.assert_valid();
        store
    }

    #[test]
    fn same_application_stays_one_session() {
        let mut engine = engine();
        let ticks = (0..10).map(|i| tick(i * 5 * SECOND, app(1), 0)).collect();
        let store = run(&mut engine, ticks);
        assert_eq!(store.summary(), vec![(SessionType::Active, 0, 45 * SECOND)]);
    }

    #[test]
    fn switching_applications_starts_a_new_session() {
        let mut engine = engine();
        let store = run(
            &mut engine,
            vec![
                tick(0, app(1), 0),
                tick(10 * SECOND, app(1), 0),
                tick(15 * SECOND, app(2), 0),
                tick(20 * SECOND, app(2), 0),
            ],
        );
        let segments = store.segments();
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start, segments[0].end), (0, 15 * SECOND));
        assert_eq!((segments[1].start, segments[1].end), (15 * SECOND, 20 * SECOND));
    }

    /// Ticks every 5 seconds, as the runtime does, with input stopping at
    /// `last_input` and optionally resuming at `resumed`.
    fn steady_ticks(until: Ts, last_input: Ts, resumed: Option<Ts>) -> Vec<Tick> {
        (0..=until / (5 * SECOND))
            .map(|step| {
                let now = step * 5 * SECOND;
                let idle = match resumed {
                    Some(resumed) if now >= resumed => now - resumed,
                    _ => (now - last_input).max(0),
                };
                tick(now, app(1), idle)
            })
            .collect()
    }

    #[test]
    fn idle_is_back_dated_to_the_last_input() {
        let mut engine = engine();
        let last_input = 10 * MINUTE - 5 * SECOND;
        let store = run(&mut engine, steady_ticks(15 * MINUTE + 5 * SECOND, last_input, None));
        assert_eq!(
            store.summary(),
            vec![
                (SessionType::Active, 0, last_input),
                (SessionType::Idle, last_input, 15 * MINUTE + 5 * SECOND)
            ]
        );
    }

    #[test]
    fn resuming_starts_active_time_at_the_first_input() {
        let mut engine = engine();
        let resumed = 10 * MINUTE - 2 * SECOND;
        let end = 10 * MINUTE + 5 * SECOND;
        let store = run(&mut engine, steady_ticks(end, 5 * SECOND, Some(resumed)));
        assert_eq!(
            store.summary(),
            vec![
                (SessionType::Active, 0, 5 * SECOND),
                (SessionType::Idle, 5 * SECOND, resumed),
                (SessionType::Active, resumed, end)
            ]
        );
    }

    #[test]
    fn idle_time_is_never_active_even_when_the_app_stays_focused() {
        let mut engine = engine();
        let store = run(&mut engine, steady_ticks(30 * MINUTE, 5 * SECOND, None));
        let active: i64 = store
            .summary()
            .iter()
            .filter(|(kind, _, _)| *kind == SessionType::Active)
            .map(|(_, start, end)| end - start)
            .sum();
        assert_eq!(active, 5 * SECOND);
    }

    #[test]
    fn short_visits_are_absorbed_into_the_surrounding_session() {
        let mut engine = engine();
        let store = run(
            &mut engine,
            vec![
                tick(0, app(1), 0),
                tick(10 * SECOND, app(2), 0),
                tick(11 * SECOND, app(1), 0),
                tick(20 * SECOND, app(1), 0),
            ],
        );
        let segments = store.segments();
        assert_eq!(segments.len(), 1, "{segments:?}");
        assert_eq!((segments[0].start, segments[0].end), (0, 20 * SECOND));
    }

    #[test]
    fn a_blip_between_different_apps_goes_to_the_next_app() {
        let mut engine = engine();
        let store = run(
            &mut engine,
            vec![
                tick(0, app(1), 0),
                tick(10 * SECOND, app(2), 0),
                tick(11 * SECOND, app(3), 0),
                tick(20 * SECOND, app(3), 0),
            ],
        );
        let segments = store.segments();
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[1].start, segments[1].end), (10 * SECOND, 20 * SECOND));
    }

    #[test]
    fn locking_records_locked_time() {
        let mut engine = engine();
        let mut locked = tick(10 * SECOND, app(1), 0);
        locked.locked = true;
        let store = run(&mut engine, vec![tick(0, app(1), 0), locked, tick(20 * SECOND, app(1), 0)]);
        let kinds: Vec<_> = store.summary().iter().map(|s| s.0).collect();
        assert_eq!(kinds, vec![SessionType::Active, SessionType::Locked, SessionType::Active]);
    }

    #[test]
    fn excluded_applications_leave_a_gap() {
        let mut engine = engine();
        let store = run(
            &mut engine,
            vec![
                tick(0, app(1), 0),
                tick(10 * SECOND, Focus::Excluded, 0),
                tick(40 * SECOND, app(1), 0),
                tick(50 * SECOND, app(1), 0),
            ],
        );
        assert_eq!(
            store.summary(),
            vec![(SessionType::Active, 0, 10 * SECOND), (SessionType::Active, 40 * SECOND, 50 * SECOND)]
        );
    }

    #[test]
    fn transient_focus_keeps_the_session() {
        let mut engine = engine();
        let store = run(
            &mut engine,
            vec![tick(0, app(1), 0), tick(5 * SECOND, Focus::Transient, 0), tick(10 * SECOND, app(1), 0)],
        );
        assert_eq!(store.summary(), vec![(SessionType::Active, 0, 10 * SECOND)]);
    }

    #[test]
    fn unknown_focus_is_not_reported_as_an_application() {
        let mut engine = engine();
        let store = run(&mut engine, vec![tick(0, Focus::Unknown, 0), tick(10 * SECOND, Focus::Unknown, 0)]);
        assert_eq!(store.summary(), vec![(SessionType::Unknown, 0, 10 * SECOND)]);
    }

    #[test]
    fn a_long_tick_gap_is_not_counted() {
        let mut engine = engine();
        let later = 2 * 60 * MINUTE;
        let store = run(
            &mut engine,
            vec![
                tick(0, app(1), 0),
                tick(5 * SECOND, app(1), 0),
                tick(later, app(1), 0),
                tick(later + 5 * SECOND, app(1), 0),
            ],
        );
        assert_eq!(
            store.summary(),
            vec![(SessionType::Active, 0, 5 * SECOND), (SessionType::Active, later, later + 5 * SECOND)]
        );
    }

    #[test]
    fn a_backwards_clock_jump_never_creates_negative_durations() {
        let mut engine = engine();
        let base = 100 * MINUTE;
        let store = run(
            &mut engine,
            vec![
                tick(base, app(1), 0),
                tick(base + 5 * SECOND, app(1), 0),
                tick(base - 30 * MINUTE, app(1), 0),
                tick(base - 30 * MINUTE + 5 * SECOND, app(1), 0),
            ],
        );
        assert_eq!(store.segments().len(), 2);
    }

    #[test]
    fn suspend_closes_the_session_at_the_suspend_time() {
        let mut engine = engine();
        let mut store = run(&mut engine, vec![tick(0, app(1), 0), tick(5 * SECOND, app(1), 0)]);
        store.apply(engine.stop(7 * SECOND));
        assert_eq!(store.summary(), vec![(SessionType::Active, 0, 5 * SECOND)]);
        assert!(store.0.values().all(|(_, open)| !open));
    }

    #[test]
    fn stop_with_nothing_running_is_harmless() {
        let mut engine = engine();
        assert!(engine.stop(1_000).is_empty());
    }

    #[test]
    fn manual_timer_overrides_tracking_and_idle() {
        let mut engine = engine();
        let manual = ManualActivity { label: Some("Reading".into()), category_id: None };
        let mut store = run(&mut engine, vec![tick(0, app(1), 0), tick(5 * SECOND, app(1), 0)]);
        store.apply(engine.start_manual(10 * SECOND, manual));
        store.apply(engine.tick(tick(30 * MINUTE, app(1), 25 * MINUTE)));
        store.apply(engine.stop_manual(40 * MINUTE));
        store.apply(engine.tick(tick(40 * MINUTE + 5 * SECOND, app(1), 0)));
        store.assert_valid();
        let kinds: Vec<_> = store.summary().iter().map(|s| s.0).collect();
        assert_eq!(kinds, vec![SessionType::Active, SessionType::Manual, SessionType::Active]);
        let segments = store.segments();
        assert_eq!(segments[1].end, 40 * MINUTE);
    }

    #[test]
    fn title_changes_do_not_split_a_session() {
        let mut engine = engine();
        let titled = |title: &str| {
            Focus::Activity(AppActivity {
                application_id: 1,
                category_id: None,
                window_title: Some(title.into()),
            })
        };
        let store = run(&mut engine, vec![tick(0, titled("one"), 0), tick(10 * SECOND, titled("two"), 0)]);
        assert_eq!(store.segments().len(), 1);
    }

    #[test]
    fn a_classification_change_splits_a_session() {
        let mut engine = engine();
        let classified = |category| {
            Focus::Activity(AppActivity { application_id: 1, category_id: category, window_title: None })
        };
        let store = run(
            &mut engine,
            vec![
                tick(0, classified(Some(1)), 0),
                tick(10 * SECOND, classified(Some(2)), 0),
                tick(20 * SECOND, classified(Some(2)), 0),
            ],
        );
        assert_eq!(store.segments().len(), 2);
    }

    /// Deterministic generator, so a failure reproduces from the seed.
    struct Random(u64);

    impl Random {
        fn next(&mut self, bound: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % bound
        }
    }

    #[test]
    fn simulated_months_of_activity_keep_every_invariant() {
        for seed in 1..=20u64 {
            let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut engine = engine();
            let mut store = Store::default();
            let mut now: Ts = 1_700_000_000_000;
            let mut idle: i64 = 0;
            let mut focus = app(1);

            for step in 0..200_000 {
                let roll = random.next(1_000);
                now += match roll {
                    0 => 3 * 60 * MINUTE,
                    1..=3 => -(random.next(120) as i64 * SECOND),
                    _ => 1_000 + random.next(9_000) as i64,
                };

                idle = if random.next(100) < 90 { 0 } else { idle + random.next(120_000) as i64 };
                if random.next(100) < 3 {
                    idle += 10 * MINUTE;
                }
                if random.next(100) < 8 {
                    focus = match random.next(10) {
                        0 => Focus::Unknown,
                        1 => Focus::Excluded,
                        2 => Focus::Transient,
                        id => app(id as i64),
                    };
                }

                let effects = match random.next(2_000) {
                    0 => engine.stop(now),
                    1 => engine.start_manual(now, ManualActivity { label: None, category_id: None }),
                    2 => engine.stop_manual(now),
                    _ => engine.tick(Tick {
                        now,
                        focus: focus.clone(),
                        idle_ms: Some(idle),
                        locked: random.next(500) == 0,
                    }),
                };

                // Overlap is not checked here: a backwards clock jump legitimately
                // revisits earlier wall-clock time, which real clocks can do too.
                for effect in &effects {
                    if let Effect::Save { segment, .. } = effect {
                        assert!(segment.end >= segment.start, "seed {seed} step {step}: {segment:?}");
                    }
                }
                store.apply(effects);
            }
            let open = store.0.values().filter(|(_, open)| *open).count();
            assert!(open <= 1, "seed {seed}: {open} open segments");
        }
    }

    #[test]
    fn with_a_forward_clock_no_session_ever_overlaps() {
        for seed in 1..=20u64 {
            let mut random = Random(seed.wrapping_mul(0xD6E8_FEB8_6659_FD93) | 1);
            let mut engine = engine();
            let mut store = Store::default();
            let mut now: Ts = 1_700_000_000_000;
            let mut idle: i64 = 0;
            let mut focus = app(1);

            for _ in 0..100_000 {
                now += 1_000 + random.next(9_000) as i64;
                idle = if random.next(100) < 85 { 0 } else { idle + 5_000 };
                if random.next(1_000) == 0 {
                    idle = 10 * MINUTE;
                }
                if random.next(100) < 10 {
                    focus = match random.next(8) {
                        0 => Focus::Unknown,
                        1 => Focus::Excluded,
                        2 => Focus::Transient,
                        id => app(id as i64),
                    };
                }
                let effects = if random.next(5_000) == 0 {
                    engine.stop(now)
                } else {
                    engine.tick(Tick { now, focus: focus.clone(), idle_ms: Some(idle), locked: false })
                };
                store.apply(effects);
            }
            store.assert_valid();
        }
    }
}
