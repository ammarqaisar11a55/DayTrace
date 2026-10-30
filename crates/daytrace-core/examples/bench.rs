//! Times the queries the interface depends on against a year of history.
//!
//! cargo run --release -p daytrace-core --example bench

use std::time::{Duration, Instant};

use chrono::{Days, Local};
use daytrace_core::analytics;
use daytrace_core::database::sessions::{self, TrackedSession};
use daytrace_core::database::{applications, Database};
use daytrace_core::models::{SessionFilter, SessionType};
use daytrace_core::time::{local_range, now_ms};

const DAYS: i64 = 365;
const SESSIONS_PER_DAY: i64 = 600;

fn measure<T>(label: &str, runs: u32, mut action: impl FnMut() -> T) {
    let mut best = Duration::MAX;
    for _ in 0..runs {
        let started = Instant::now();
        std::hint::black_box(action());
        best = best.min(started.elapsed());
    }
    println!("{label:<34} {:>9.2} ms", best.as_secs_f64() * 1_000.0);
}

fn main() {
    let directory = std::env::temp_dir().join(format!("daytrace-bench-{}", std::process::id()));
    let database = Database::open(directory.join("bench.db")).expect("open database");

    let apps: Vec<i64> = database
        .write(|c| {
            (0..40)
                .map(|i| applications::upsert(c, &format!("app{i}"), &format!("App {i}"), None, None))
                .collect()
        })
        .expect("insert applications");

    let started = Instant::now();
    let end = now_ms() - 60_000;
    let step = 86_400_000 / SESSIONS_PER_DAY;
    let total = DAYS * SESSIONS_PER_DAY;
    for chunk in 0..DAYS {
        database
            .write(|c| {
                for index in 0..SESSIONS_PER_DAY {
                    let position = total - (chunk * SESSIONS_PER_DAY + index);
                    let start_time = end - position * step;
                    sessions::insert_tracked(
                        c,
                        &TrackedSession {
                            application_id: Some(apps[(index % 40) as usize]),
                            project_id: None,
                            category_id: Some(index % 9 + 1),
                            start_time,
                            end_time: start_time + step,
                            session_type: if index % 11 == 0 {
                                SessionType::Idle
                            } else {
                                SessionType::Active
                            },
                            window_title: None,
                            label: None,
                            manual_override: false,
                            is_open: false,
                        },
                    )?;
                }
                Ok(())
            })
            .expect("insert sessions");
    }
    println!("{:<34} {:>9.2} s", format!("insert {total} sessions"), started.elapsed().as_secs_f64());

    let size = std::fs::metadata(database.path()).expect("database file").len();
    println!("{:<34} {:>9.1} MB", "database size", size as f64 / 1_048_576.0);

    let today = Local::now().date_naive();
    let report = |days: u64| {
        let from = today - Days::new(days - 1);
        database.read(|c| analytics::report(c, &Local, from, today, now_ms(), None)).expect("report")
    };
    measure("report, today", 20, || report(1));
    measure("report, last 7 days", 20, || report(7));
    measure("report, last 30 days", 10, || report(30));
    measure("report, last 365 days", 3, || report(365));

    let (from, to) = local_range(&Local, today, today);
    let filter = SessionFilter { from, to, ..Default::default() };
    measure("timeline, one day", 20, || database.read(|c| sessions::list(c, &filter)).expect("list"));
    measure("application usage, 30 days", 10, || {
        database.read(|c| applications::list_usage(c, from - 30 * 86_400_000, to)).expect("usage")
    });

    drop(database);
    let _ = std::fs::remove_dir_all(directory);
}
