//! Runs the X11 backend against a real X server. It needs a disposable display
//! such as Xvfb, because it writes EWMH properties to the root window:
//!
//!   DAYTRACE_TEST_X11=1 DISPLAY=:94 XDG_SESSION_TYPE=x11 cargo test --test x11_tracker

#![cfg(target_os = "linux")]

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use daytrace_core::tracker::runtime::Message;
use daytrace_core::tracker::{create_tracker, EventSink, Foreground, SampleOptions, Support, SystemEvent};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperExt;

fn atom(connection: &RustConnection, name: &str) -> u32 {
    connection.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom
}

fn window(connection: &RustConnection, root: u32, class: &[u8], title: &str, pid: u32) -> u32 {
    let id = connection.generate_id().unwrap();
    connection
        .create_window(0, id, root, 0, 0, 100, 100, 0, WindowClass::INPUT_OUTPUT, 0, &CreateWindowAux::new())
        .unwrap();
    connection.change_property8(PropMode::REPLACE, id, AtomEnum::WM_CLASS, AtomEnum::STRING, class).unwrap();
    connection
        .change_property8(PropMode::REPLACE, id, atom(connection, "_NET_WM_NAME"), atom(connection, "UTF8_STRING"), title.as_bytes())
        .unwrap();
    connection.change_property32(PropMode::REPLACE, id, atom(connection, "_NET_WM_PID"), AtomEnum::CARDINAL, &[pid]).unwrap();
    connection.flush().unwrap();
    id
}

fn activate(connection: &RustConnection, root: u32, window: u32) {
    connection
        .change_property32(PropMode::REPLACE, root, atom(connection, "_NET_ACTIVE_WINDOW"), AtomEnum::WINDOW, &[window])
        .unwrap();
    // A round trip guarantees the server has applied the change.
    connection.get_input_focus().unwrap().reply().unwrap();
}

#[test]
fn reports_the_active_window_and_reacts_to_changes() {
    if std::env::var("DAYTRACE_TEST_X11").is_err() {
        eprintln!("skipped: set DAYTRACE_TEST_X11=1 on a disposable display");
        return;
    }

    let (connection, screen) = RustConnection::connect(None).unwrap();
    let root = connection.setup().roots[screen].root;
    let pid = std::process::id();
    let editor = window(&connection, root, b"code\0Code\0", "secret.txt - Code", pid);
    let browser = window(&connection, root, b"Navigator\0firefox\0", "Bank login", pid);

    connection
        .change_property32(
            PropMode::REPLACE,
            root,
            atom(&connection, "_NET_SUPPORTED"),
            AtomEnum::ATOM,
            &[atom(&connection, "_NET_ACTIVE_WINDOW")],
        )
        .unwrap();
    activate(&connection, root, editor);

    let (sender, receiver) = channel();
    let mut tracker = create_tracker(EventSink::new(sender));
    let capabilities = tracker.capabilities();
    assert_eq!(capabilities.display_server, "x11");
    assert_eq!(capabilities.backend, "x11-ewmh");
    assert_eq!(capabilities.foreground_app.support, Support::Available);
    assert_eq!(capabilities.idle.support, Support::Available);

    let sample = tracker.sample(SampleOptions { include_window_title: false });
    let Foreground::App(app) = sample.foreground else { panic!("expected an application") };
    assert_eq!(app.identifier, "code");
    assert_eq!(app.window_title, None, "titles must not be read unless asked for");
    assert!(app.executable_path.is_some(), "the process ID leads to the executable");
    assert!(sample.idle_ms.is_some_and(|idle| idle >= 0));

    std::thread::sleep(Duration::from_millis(300));
    activate(&connection, root, browser);
    let woken = loop {
        match receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(Message::System(SystemEvent::FocusChanged)) => break true,
            Ok(_) => continue,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break false,
        }
    };
    assert!(woken, "a window change must wake the runtime without waiting for a poll");

    let sample = tracker.sample(SampleOptions { include_window_title: true });
    let Foreground::App(app) = sample.foreground else { panic!("expected an application") };
    assert_eq!(app.identifier, "firefox");
    assert_eq!(app.window_title.as_deref(), Some("Bank login"));

    activate(&connection, root, 0);
    let sample = tracker.sample(SampleOptions::default());
    assert_eq!(sample.foreground, Foreground::NoFocus);
}
