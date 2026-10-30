# Development

## Setup

You need Rust (stable), Node.js 22, and on Linux:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev patchelf
```

Then:

```bash
npm install
npm run tauri dev
```

## Layout

| Path | Contents |
| --- | --- |
| `crates/daytrace-core` | Engine, trackers, database and analytics. No Tauri dependency |
| `src-tauri` | Tauri commands, tray, windows and updater |
| `src` | React interface |
| `extensions/gnome-shell` | The Shell extension, embedded in the binary |
| `tests` | End-to-end and platform tests |

`crates/daytrace-core` builds and tests without GTK or WebKit, so most work needs no desktop libraries.

## Tests

```bash
cargo test -p daytrace-core
npm run typecheck
npm test
```

### End-to-end

Needs `xvfb`, `webkit2gtk-driver`, and `cargo install tauri-driver --locked`. The suite uses a
disposable home directory, D-Bus session and display.

```bash
cargo build -p daytrace -F tauri/custom-protocol
DAYTRACE_BINARY=$PWD/target/debug/daytrace tests/e2e/run.sh
```

### Platform backends

Each of these runs in a disposable session and leaves your desktop alone.

```bash
tests/gnome/session.sh python3 -I tests/gnome/tracking.py target/debug/daytrace
cargo build -p daytrace-core --example probe
tests/wlroots/run.sh target/debug/examples/probe
```

The X11 test writes to the root window, so run it on a virtual display:

```bash
Xvfb :94 &
DAYTRACE_TEST_X11=1 DISPLAY=:94 XDG_SESSION_TYPE=x11 cargo test -p daytrace-core --test x11_tracker
```

### Diagnosing a platform

This prints what the tracker can see, and stores nothing.

```bash
cargo run -p daytrace-core --example probe -- 30
```

## Database changes

Add a new numbered file to `crates/daytrace-core/migrations`, and register it in
`database/migrations.rs`. Never edit a released migration.

## Releasing

The workflow needs these repository secrets:

| Secret | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Signs update artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password, which may be empty |
| `SNAPCRAFT_STORE_CREDENTIALS` | Optional. Publishes to the Snap Store |

To release:

1. Update the version in `package.json`, `src-tauri/tauri.conf.json` and the root `Cargo.toml`.
2. Update `CHANGELOG.md`.
3. Push a tag such as `v1.2.0`, or `v1.2.0-beta.1` for the beta channel.
