//! Prints what the tracker can see on this computer, for diagnosing platform
//! support. Reports stay in this terminal and nothing is stored.
//!
//! cargo run -p daytrace-core --example probe -- 30

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use daytrace_core::tracker::{create_tracker, EventSink, Foreground, SampleOptions};

fn main() {
    let seconds: u64 = std::env::args().nth(1).and_then(|value| value.parse().ok()).unwrap_or(10);
    let include_window_title = std::env::args().any(|argument| argument == "--titles");

    let (sender, receiver) = channel();
    let mut tracker = create_tracker(EventSink::new(sender));
    thread::spawn(move || {
        while let Ok(message) = receiver.recv() {
            println!("event: {message:?}");
        }
    });
    thread::sleep(Duration::from_millis(500));

    let capabilities = tracker.capabilities();
    println!("{}", serde_json::to_string_pretty(&capabilities).expect("capabilities serialize"));

    let mut last = None;
    for _ in 0..seconds {
        let sample = tracker.sample(SampleOptions { include_window_title });
        let line = match &sample.foreground {
            Foreground::App(app) => format!(
                "app id={} name={:?} exe={:?} title={:?}",
                app.identifier, app.name, app.executable_path, app.window_title
            ),
            other => format!("{other:?}"),
        };
        let line = format!("{line} locked={}", sample.locked);
        if last.as_ref() != Some(&line) {
            println!("{line} idle_ms={:?}", sample.idle_ms);
            last = Some(line);
        }
        thread::sleep(Duration::from_secs(1));
    }
}
