# Verification status

This records what has actually been run. A platform is not called supported because its code
compiles. Last updated 2026-11-13, after version 0.2.0.

## Verified by running it

| Area | How |
| --- | --- |
| Session engine | 132 unit tests, plus 40 randomized simulations of up to 200,000 steps |
| Timezones, daylight saving, midnight | Unit tests across New York, Havana and Kolkata |
| Database, migrations, backup, restore | Unit tests, including upgrading and restoring a real version 1 database |
| Classification, rule precedence, preview | Unit tests |
| Deletion | Raw database files are searched after each kind of delete and hold none of the deleted text. Each fix was disabled to confirm its test fails |
| Application UI | 26 end-to-end checks on the real binary through WebDriver, including that no page is taller than the window |
| Custom title bar on GNOME 50 Wayland | Real headless GNOME Shell with a virtual pointer: the three buttons, double-click, dragging, resizing from edges and corners, the minimum size, and closing to the background |
| Lock and unlock order on Linux | Replayed against a fake `logind` and GNOME screen saver on private buses. The old code stayed locked after the unlock and the fix does not |
| Autostart entry on Linux | Written and removed, checked on disk by the end-to-end suite |
| GNOME 50.1 on Wayland | Real headless GNOME Shell: extension, focus, idle, ordering, titles opt-in, crash recovery |
| Background tracking | Survives closing the window, and quits on close when that setting is off |
| Single instance | A second launch reopens the first instance's window |
| X11 backend | Real X server (Xvfb) with EWMH properties set by the test |
| wlroots backend | Real headless sway: focus, events, `ext-idle-notify` |
| Update signatures | Real AppImage and `.deb` verified against the shipped key; tampered and wrongly signed files rejected |
| AppImage | Launched in real GNOME, window opened, database created |
| `.deb`, `.rpm` | Built and signed. `.deb` contents inspected. Neither has been installed |
| Windows core logic | 110 tests pass on `windows-latest` in CI, including live Win32 calls for process inspection and idle time |
| CI on Ubuntu 24.04 | Clippy, the X11 test and all 25 end-to-end checks pass on GitHub Actions |
| Release workflow | v0.1.0 built and published every package. Both update manifests are served with all six platform entries, and the published `.deb` and Windows installer verify against the shipped key |
| Snap from the Store | Revision 1 installed from the edge channel on Ubuntu 26.04 with GNOME 50. It launched, and recorded active time as unidentified, as designed under strict confinement |

## Compiled only, not run

| Area | Status |
| --- | --- |
| Windows focus detection and application | The application and installer build for Windows but have never been launched. CI has no interactive desktop, so foreground detection and the event hooks are untested |
| KDE Plasma backend | Compiles. Written from KWin's scripting documentation and never run against KWin |
| GNOME extension on Shell 42 to 44 | The legacy variant has never been loaded. Shell 45 to 49 use the modern variant, tested only on 50 |

## Not verified

| Area | Why |
| --- | --- |
| Windows installation, foreground tracking, updates, lock, suspend, startup | Needs a real Windows 10 and 11 desktop |
| Custom title bar on Windows | The buttons, dragging and edge resizing need a real Windows desktop |
| Installing `.deb` and `.rpm`, tray behaviour | Needs root, and a real desktop session for the tray |
| Snap beyond first launch | Only the first launch on one machine has been seen. Autostart, tray and exports inside confinement are untested |
| Applying an update end to end | Needs a second published release to update to. Only signature checks and version rules are tested |
| Real suspend, resume and screen lock | Tested in simulated time and against a fake `logind`; a real desktop was not locked |
| Fedora, Debian, Linux Mint | Not tested. Only Ubuntu 26.04 was available |

## Running the suites

```bash
cargo test -p daytrace-core
npm test
DAYTRACE_BINARY=$PWD/target/debug/daytrace tests/e2e/run.sh
tests/gnome/session.sh python3 -I tests/gnome/tracking.py target/debug/daytrace
tests/gnome/session.sh --no-extension python3 -I tests/gnome/window.py target/debug/daytrace
tests/wlroots/run.sh target/debug/examples/probe
```
