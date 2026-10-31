# DayTrace Technical Implementation Plan

This is the original plan. Projects were later removed from the product, and references to them
below are historical. ARCHITECTURE.md describes the current design.

This plan was written after inspecting the repository (empty, no commits) and probing a real
GNOME 50 Wayland session on Ubuntu 26.04. Statements about platform behaviour are marked
**verified** when they were observed directly, and **documented** when they rest on upstream
documentation and could not be run on the development machine.

## 1. Repository decisions

| Area | Decision | Reason |
| --- | --- | --- |
| Layout | Cargo workspace: `crates/daytrace-core` (no Tauri dependency) and `src-tauri` (shell) | Business logic, persistence and trackers build and test without GTK/WebKit, and cross-check for Windows quickly |
| Licence | MIT | Suitable for public open-source distribution |
| Identifier | `io.github.ammarqaisar11a55.DayTrace` | Reverse-DNS of the GitHub owner |
| Tauri | 2.x, pinned below the 3.0 alpha line | Specified stack |
| Database | `rusqlite` with bundled SQLite, owned by Rust | Same SQLite everywhere, no SQL in the webview |
| Timestamps | UTC epoch milliseconds stored as `INTEGER` | Indexable, no string parsing, no timezone ambiguity |
| Cached summaries | No `daily_summaries` table | Queries over indexed sessions are fast enough (see `docs/BENCHMARKS.md`), and a cache would go stale on every edit, rule application or timezone change |

## 2. Architecture

```text
React UI ── typed invoke() ──> Tauri commands ──> daytrace-core
                                                    ├─ database (migrations, repositories)
                                                    ├─ projects (rule classifier)
                                                    ├─ analytics (timezone-aware bucketing)
                                                    ├─ config (typed settings)
                                                    └─ tracker
                                                        ├─ engine   pure state machine
                                                        ├─ runtime  background thread, batched writes
                                                        └─ platform backends
                                                            ├─ windows
                                                            ├─ linux/x11
                                                            ├─ linux/gnome, linux/kde, linux/wlr
                                                            └─ unsupported
```

The tracking engine is a pure function of its inputs, so session rules (idle back-dating, blip
absorption, merging, clock jumps, suspend gaps) are tested with simulated time. Platform
backends only produce a `Sample`. The runtime turns samples into engine inputs and writes the
results in batched transactions.

### Session lifecycle

- The open session is checkpointed every 30 seconds with `is_open = 1`.
- A crash loses at most one checkpoint interval. On startup, open rows are closed at their last
  checkpoint.
- A wall-clock jump backwards, or a tick gap larger than the suspend threshold, closes the
  session at the last good tick. Durations cannot go negative.
- Idle is back-dated to the last input, so idle time is never counted as application use.
- Activity shorter than the blip threshold is absorbed into its neighbours.

### Session types

| Type | Meaning | Counts as |
| --- | --- | --- |
| `active` | Foreground application positively identified | Active |
| `unknown` | User is active (idle detection works) but the application cannot be identified | Active, shown as "Unidentified" |
| `manual` | User-started timer or user-entered period | Active |
| `idle` | No input past the idle threshold | Idle |
| `locked` | Session locked | Idle |

Time with no record at all (machine off, suspended, paused, excluded application) is
**untracked**. It is never estimated.

### Classification precedence

1. A manual assignment on a session always wins and is never overwritten.
2. Enabled rules run in ascending `priority` (ties by `id`). First match wins, independently for
   project and category.
3. The application's default project and category.
4. The category's default project.

## 3. Platform feasibility

### Windows 10/11 (documented, compile-checked only on this machine)

`GetForegroundWindow`, `GetWindowThreadProcessId`, `QueryFullProcessImageNameW`,
`GetLastInputInfo`, and `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` for prompt switch detection.
A hidden top-level window receives `WM_WTSSESSION_CHANGE`, `WM_POWERBROADCAST` and
`WM_ENDSESSION`. It must not be a message-only window, because those do not receive broadcasts.
UWP applications are hosted by `ApplicationFrameHost.exe`, so the real child process is resolved.

**Risk:** the backend cannot be run here. It is cross-checked against the MinGW target and
exercised on `windows-latest` in CI where a desktop session allows it. Manual verification on
Windows 10 and 11 is required before calling Windows supported.

### Linux X11 (verifiable here through Xvfb and the session's Xwayland)

`x11rb` (pure Rust, no libX11 link) reads `_NET_ACTIVE_WINDOW`, `_NET_WM_PID`, `WM_CLASS` and
`_NET_WM_NAME`, subscribes to `PropertyNotify` on the root window, and uses `MIT-SCREEN-SAVER`
for idle time.

### Linux Wayland

X11 fallback under Wayland is **rejected**: `_NET_ACTIVE_WINDOW` under Xwayland only knows about
Xwayland clients, so it would report wrong applications as if they were accurate.

| Desktop | Foreground app | Idle | Status |
| --- | --- | --- | --- |
| GNOME | Bundled Shell extension exposing focus on D-Bus, installed only after explicit consent | `org.gnome.Mutter.IdleMonitor` **verified** | `org.gnome.Shell.Introspect.GetWindows` returns `AccessDenied` and `Shell.Eval` returns false: **verified**. No extension-free route exists |
| KDE Plasma | KWin script loaded through `org.kde.KWin /Scripting`, calling back into a DayTrace D-Bus object | `ext-idle-notify-v1` | **documented**, not runnable here |
| wlroots (Sway, Hyprland, Wayfire, labwc, river, niri) | `wlr-foreign-toplevel-management-v1` (no PID, so no executable path) | `ext-idle-notify-v1` | Testable with headless sway |
| Other | Unavailable, reported honestly | `ext-idle-notify-v1` where offered | Manual timer remains available |

`org.freedesktop.ScreenSaver.GetSessionIdleTime` returns `NotSupported` on GNOME (**verified**),
so it is only a last-resort probe elsewhere. `logind` `IdleHint`, `LockedHint` and
`PrepareForSleep` are available (**verified**).

The desktop-portal family has no API for the focused application, so no portal is used.

When no integration is available, tracking records `unknown` active time if idle detection
works, and nothing otherwise. The Settings capability panel states exactly what is available.

## 4. Updater strategy

| Install type | Detection | In-app install | Behaviour |
| --- | --- | --- | --- |
| Windows NSIS | Windows build | Yes | Tauri updater, signature verified, NSIS passive install |
| AppImage | `$APPIMAGE` | Yes | Tauri updater replaces the file and keeps it executable |
| `.deb` | Executable owned by dpkg | No | Notify, show the `apt`/download instructions |
| `.rpm` | Executable owned by rpm | No | Notify, show the `dnf`/download instructions |
| Snap | `$SNAP` | No | Show version only, `snapd` updates it |
| Development or unknown | Otherwise | No | Reports the install method as unsupported |

Channels are separate manifest URLs. Stable uses the `latest` GitHub release and beta uses a
rolling `channel-beta` release, so channel selection needs no GitHub API calls and no rate
limit. Every install path verifies a minisign signature against the public key compiled into
the binary. Only newer versions are accepted, which provides downgrade protection.

## 5. Snap confinement risk

Strict confinement limits what a tracker may do: AppArmor blocks reading `/proc/<pid>/exe` of
other processes, and access to Mutter's D-Bus API and the DayTrace Shell extension is not
granted by a standard interface. Classic confinement needs Snap Store approval.

Decision: ship strict confinement with `x11`, `wayland`, `desktop` and `login-session-observe`,
detect failures at runtime, and show the real capability state in Settings. The packaged Snap
cannot be built here (no LXD or Multipass), so GitHub Actions builds it from `snap/snapcraft.yaml`
and installation must be verified manually.

## 6. Verification available on the development machine

| Target | Method |
| --- | --- |
| Core logic | `cargo test -p daytrace-core`, randomized long-run simulation |
| Frontend | `tsc`, Vitest |
| Real app UI | `WebKitWebDriver` and `tauri-driver` under Xvfb |
| X11 tracker | Real windows under Xvfb and Xwayland |
| GNOME Wayland tracker | Disposable `gnome-shell --headless` session, so the user's session is untouched |
| wlroots tracker | Headless sway if it can be unpacked locally |
| Windows | `cargo check` against `x86_64-pc-windows-gnu` with locally unpacked MinGW |
| Packaging | `.deb`, `.rpm`, AppImage built locally; Snap and Windows installer in CI |

Anything that cannot be verified here is listed in `docs/VERIFICATION.md`, and is not claimed
as working.

## 7. Delivery order

Work follows the roadmap phases, committing after each module:

1. Foundation: scaffold, design system, navigation, SQLite, migrations.
2. Tracking engine, backends, runtime, crash recovery.
3. Dashboard, timeline, applications, categories, projects, rules.
4. Analytics.
5. Privacy, settings, tray, autostart, export, backup.
6. Updater, packaging, CI/CD.
7. Tests, profiling, accessibility, documentation.
