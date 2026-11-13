# Architecture

DayTrace is a Tauri 2 application with a Rust backend and a React frontend. All data is local.

```text
React UI ── typed invoke() ──> Tauri commands ──> daytrace-core
                                                    ├─ database    migrations and repositories
                                                    ├─ classification   rules and exclusions
                                                    ├─ analytics   timezone-aware reports
                                                    ├─ updates     channels and install methods
                                                    └─ tracker
                                                        ├─ engine   pure session state machine
                                                        ├─ runtime  background thread, batched writes
                                                        └─ backends Windows, X11, GNOME, KDE, wlroots
```

Logic lives in `crates/daytrace-core`, which has no Tauri dependency. `src-tauri` is a thin shell.

## Tracking

Backends only observe. Each implements `ActivityTracker` and reports a `Sample`: what has focus, how
long since the last input, and whether the session is locked. They can also raise `SystemEvent`s, so
a window switch is noticed at once rather than at the next poll.

The **engine** (`tracker/engine.rs`) turns samples into sessions. It does no I/O and never reads the
clock, because each input carries its timestamp, so all its rules are tested in simulated time.

| Situation | Rule |
| --- | --- |
| Input stops | Idle is back-dated to the last input, so idle time is never counted as use |
| Input resumes | Active time restarts at the first input |
| A window is focused for under 2 seconds | The visit is absorbed into its neighbours |
| Long gap between ticks | The session closes at the last tick, which catches sleep with no event |
| Clock moves backwards | The session closes at the last good tick, so durations cannot go negative |
| Window title changes | The session continues, unless the classification changes |

The **runtime** (`tracker/runtime.rs`) owns the background thread. It applies exclusions and
classification, then coalesces the engine's output and writes it in one transaction every 30
seconds or at a lifecycle event. Excluded applications are dropped before any database access.

### Crash recovery

The open session is checkpointed with `is_open = 1`. After a crash, the next start closes it at its
last checkpoint, so at most one flush interval is lost. Time is never invented to fill the gap.

### Session types

| Type | Meaning | Counts as |
| --- | --- | --- |
| `active` | Application positively identified | Active |
| `unknown` | User present, application unknown | Active |
| `manual` | Timer or manual entry | Active |
| `idle` | No input past the threshold | Idle |
| `locked` | Session locked | Idle |

Time with no record is untracked, and it is never estimated. With no idle detection there is no
evidence that the user is present, so unknown focus records nothing.

### Platform backends

| Platform | Focus | Idle |
| --- | --- | --- |
| Windows | `GetForegroundWindow`, `SetWinEventHook` | `GetLastInputInfo` |
| X11 | EWMH `_NET_ACTIVE_WINDOW` | MIT-SCREEN-SAVER |
| GNOME Wayland | Bundled Shell extension over D-Bus | `org.gnome.Mutter.IdleMonitor` |
| KDE Wayland | KWin script calling back over D-Bus | `ext-idle-notify-v1` |
| wlroots | `wlr-foreign-toplevel-management` | `ext-idle-notify-v1` |

X11 is deliberately not a fallback under Wayland. Xwayland only knows about a fraction of the
windows, so it would report wrong applications as if they were accurate. `logind` provides lock,
suspend and shutdown events on Linux, and a hidden window receives them on Windows.

## Data

SQLite with numbered migrations tracked by `PRAGMA user_version`. Timestamps are UTC epoch
milliseconds. WAL mode lets the interface read while the tracker writes, through separate reader and
writer connections.

There is no `daily_summaries` table. Reports are computed from sessions, which stay the only source
of truth, and [docs/BENCHMARKS.md](docs/BENCHMARKS.md) shows that is fast enough. Sessions are clipped
to the requested range and split at local day and hour boundaries, so midnight-crossing sessions and
23- or 25-hour days stay correct.

## Classification

1. A category the user set on a session is never overwritten.
2. Enabled rules run by ascending priority, then by id. The first match decides.
3. The application's default category.

Applying rules to history only adds or changes assignments. A session no rule matches keeps what it has.

## Background running

Closing the window destroys the webview and leaves tracking running, unless Run in the background is
off. A second launch signals the running instance through the single-instance plugin.

## Updates

How an update is applied depends on the install method, detected in `updates/mod.rs`.

| Install | Behaviour |
| --- | --- |
| Windows installer, AppImage | Download, verify the signature, install, restart |
| `.deb`, `.rpm` | Notify only, because the package manager owns the files |
| Snap | Notify only, because `snapd` updates it |

Channels are separate manifest addresses, so channel selection needs no GitHub API calls. Stable uses
the latest release, and beta uses a rolling `channel-beta` release.

## Frontend

React with TypeScript, Tailwind and Recharts. `services/api.ts` is the only place that calls the
backend. `hooks/useData.ts` loads data and refreshes it on a timer and on `data-changed` events.
Colors, radii and timing are `--dt-*` tokens in `styles/app.css`, so the dark and light themes
switch in one place. The window has no system decorations: `components/app-shell/TitleBar.tsx`
draws the title bar, its buttons call Tauri's window API, and Tauri keeps the edges resizable.
