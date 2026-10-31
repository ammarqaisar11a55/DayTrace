# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Removed

- Projects, including project assignments, project analytics and project exclusions. Sessions,
  applications, categories and their links are kept when upgrading. Rules that only assigned a
  project are deleted, and rules that also assigned a category keep working.

### Changed

- Classification rules moved to the Categories page and now always assign a category
- Preview last 30 days also works with no rules, so application defaults can be applied to history

### Fixed

- Delete all history now removes the copy kept after a restore, and history limits apply to that copy

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
