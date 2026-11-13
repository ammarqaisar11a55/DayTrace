# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-11-13

### Added

- A title bar drawn by DayTrace, with minimize, maximize or restore, and close. It drags the
  window, maximizes on double-click, and leaves the window edges resizable.
- Search and sorting on the Applications page, with each application's share of tracked time.
- Today and Yesterday buttons on the Timeline.
- Settings grouped into General, Appearance, Tracking, Applications, Privacy, Startup, Updates
  and About.

### Changed

- A new purple design, dark by default, with the Inter typeface. Light keeps the same colors on
  pale backgrounds, and existing theme choices are kept.
- The app icon is redrawn in purple.
- The dashboard shows four summary cards, active and idle time, and recent activity with
  application icons and categories.

### Fixed

- DayTrace could keep showing Locked after the screen was unlocked on Linux.
- A hard scroll at the bottom of Analytics moved the whole window content up and left an empty
  band below it.

## [0.2.0] - 2026-10-31

### Removed

- Projects, including project assignments, project analytics and project exclusions. Sessions,
  applications, categories and their links are kept when upgrading. Rules that only assigned a
  project are deleted, and rules that also assigned a category keep working.

### Changed

- Classification rules moved to the Categories page and now always assign a category
- Preview last 30 days also works with no rules, so application defaults can be applied to history

### Fixed

- Delete all history now removes the copy kept after a restore, and history limits apply to that copy
- Delete all history now also clears recorded applications, including their default categories
- Deleted sessions are now overwritten and can no longer be read back from the database files
- Add entry no longer defaults to a time in the future, and its times follow the selected day
- Undoing a restore works on Windows, and a failed restore keeps the existing safety copy

## [0.1.0] - 2026-10-31

First public test release. See docs/VERIFICATION.md for what has and has not been tested.

### Added

- Automatic tracking on Windows, X11, GNOME on Wayland, KDE Plasma and wlroots compositors
- Idle, lock and suspend handling, crash recovery and batched writes
- Dashboard, timeline with corrections, applications, projects, categories and analytics
- Classification rules with precedence and a preview
- Manual timers and manual entries
- Privacy controls: pause, exclusions, retention, export, backup, restore and deletion
- Options to start with the computer and to run in the background
- System tray menu
- Signed updates with stable and beta channels, respecting each install method
- Windows, AppImage, Debian, RPM and Snap packaging
