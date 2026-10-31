# Privacy

DayTrace is local-first. All activity data is stored in a SQLite database on your computer, and it
is never sent anywhere. There is no account, no cloud synchronization and no telemetry.

Automatic tracking stays off until you turn it on.

## What is recorded

| Data | When |
| --- | --- |
| Application name and identifier | While tracking is on |
| Program path | While tracking is on, where the platform reports it |
| Application icon location | While tracking is on |
| Session start and end times | While tracking is on |
| Idle and locked periods | While tracking is on |
| Categories, labels and rules you create | When you create them |
| Window titles | Only if you turn on "Collect window titles" |

If the application cannot be identified, DayTrace records only that you were active. It does not
guess. If it also cannot tell whether you are present, it records nothing.

## What is never recorded

Keystrokes, passwords, clipboard contents, screenshots, webcam or microphone data, private
messages, and browser history.

## Where data is stored

| System | Location |
| --- | --- |
| Linux | `~/.local/share/io.github.ammarqaisar11a55.DayTrace/` |
| Windows | `%APPDATA%\io.github.ammarqaisar11a55.DayTrace\` |

On Linux the database is readable only by your user. The database is not encrypted, so anyone with
access to your user account can read it. Backups you create are not encrypted either.

Restoring a backup keeps your previous data beside the database as
`daytrace.before-restore.db`, so that you can undo it. Backups you saved yourself are yours to
manage, and DayTrace does not change them.

## Deleting data

**Delete all history** removes every session and recorded application, and the copy kept after a
restore. Your categories, rules and exclusions stay, because you created them. Those can contain
application names you typed, so remove them too if you want no trace of them.

When sessions are deleted, whether all at once, one at a time or by a history limit, DayTrace
overwrites them inside its database and empties its log, so they cannot be read back from those
files. History limits apply to the copy kept after a restore as well.

This works at the level of files. DayTrace cannot reach copies outside its folder, such as your own
backups, filesystem snapshots or system backups, and SSDs can keep old data internally until it is
reclaimed. For stronger protection, use full-disk encryption.

Application logs are stored in the standard log directory for your system.

## Network use

The only network requests are update checks and update downloads from GitHub, using HTTPS. GitHub
can see your IP address and that an update was requested, as with any download. You can turn off
checking in Settings.

## The GNOME extension

GNOME does not let applications see which window has focus, so DayTrace includes a small Shell
extension, installed only when you ask. It answers requests on your session bus with the focused
application, and with the window title if the caller asks for it.

Any program running as your user could make the same request. Such a program can already read
your DayTrace database, so this adds little exposure, but it does mean the extension relaxes
GNOME's isolation between applications. Remove it from Settings at any time.

## Your controls

In Settings you can turn tracking off, pause it, exclude applications and program paths,
limit how long history is kept, export your data as JSON or CSV, back it up, and delete all of it.

Exclusions take effect before anything is stored: an excluded application is never written to the
database. Adding an exclusion does not remove what was already recorded, but you can delete an
application's history from the Applications page.
