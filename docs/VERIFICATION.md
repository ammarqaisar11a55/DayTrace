# Verification status

This records what has actually been run. A platform is not called supported because its code
compiles. Last updated 2026-10-30.

## Verified by running it

| Area | How |
| --- | --- |
| Session engine | 113 unit tests, plus 40 randomized simulations of up to 200,000 steps |
| Timezones, daylight saving, midnight | Unit tests across New York, Havana and Kolkata |
| Database, migrations, backup, restore | Unit tests |
| Classification, rule precedence, preview | Unit tests |
| Application UI | 23 end-to-end checks on the real binary through WebDriver |
| Autostart entry on Linux | Written and removed, checked on disk by the end-to-end suite |
| GNOME 50.1 on Wayland | Real headless GNOME Shell: extension, focus, idle, ordering, titles opt-in, crash recovery |
| Background tracking | Survives closing the window, and quits on close when that setting is off |
| Single instance | A second launch reopens the first instance's window |
| X11 backend | Real X server (Xvfb) with EWMH properties set by the test |
| wlroots backend | Real headless sway: focus, events, `ext-idle-notify` |
| Update signatures | Real AppImage and `.deb` verified against the shipped key; tampered and wrongly signed files rejected |
| AppImage | Launched in real GNOME, window opened, database created |
| `.deb`, `.rpm` | Built and signed. `.deb` contents inspected. Neither has been installed |

## Compiled only, not run

| Area | Status |
| --- | --- |
| Windows backend and application | Type-checks against `x86_64-pc-windows-gnu`. `core` tests run on `windows-latest` in CI once pushed |
| KDE Plasma backend | Compiles. Written from KWin's scripting documentation and never run against KWin |
| GNOME extension on Shell 42 to 44 | The legacy variant has never been loaded. Shell 45 to 49 use the modern variant, tested only on 50 |

## Not verified

| Area | Why |
| --- | --- |
| Windows installation, updates, lock, suspend, startup | Needs a real Windows 10 and 11 machine |
| Installing `.deb` and `.rpm`, tray behaviour | Needs root, and a real desktop session for the tray |
| Snap build and install | Needs LXD or Multipass. `snapcraft.yaml` has never been built |
| Applying an update end to end | Needs a published release. Only signature checks and version rules are tested |
| Real suspend, resume and screen lock | Handled by tests in simulated time only; real `logind` signals were not triggered |
| GitHub Actions workflows | Have not run, because nothing has been pushed |
| Fedora, Debian, Linux Mint | Not tested. Only Ubuntu 26.04 was available |

## Running the suites

```bash
cargo test -p daytrace-core
npm test
DAYTRACE_BINARY=$PWD/target/debug/daytrace tests/e2e/run.sh
tests/gnome/session.sh python3 -I tests/gnome/tracking.py target/debug/daytrace
tests/wlroots/run.sh target/debug/examples/probe
```
