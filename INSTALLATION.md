# Installation

Download packages from [GitHub Releases](https://github.com/ammarqaisar11a55/DayTrace/releases).

## Windows

Run `DayTrace_x.y.z_x64-setup.exe`. Administrator rights are not needed. Windows SmartScreen may
warn because the installer is not yet code-signed. Updates install from inside DayTrace.

## Ubuntu, Debian and Linux Mint

```bash
sudo apt install ./DayTrace_x.y.z_amd64.deb
```

DayTrace tells you when an update is available, and you install it the same way. An APT repository
is planned as a separate channel.

## Fedora and other RPM distributions

```bash
sudo dnf install ./DayTrace-x.y.z-1.x86_64.rpm
```

Upgrades go through your package manager.

## AppImage

```bash
chmod +x DayTrace_x.y.z_amd64.AppImage
./DayTrace_x.y.z_amd64.AppImage
```

Updates install from inside DayTrace. Some distributions need `libfuse2` to run AppImages.

## Snap

DayTrace is on the Snap Store's edge channel while it is being tested:

```bash
sudo snap install daytrace --edge
```

`snapd` keeps it updated.

To let DayTrace notice idle time and screen locks:

```bash
sudo snap connect daytrace:login-session-observe
```
 Strict confinement reduces what can be tracked, and Settings shows what
works. Use another package for full tracking.

## After installing

Setup explains what DayTrace records and asks whether to turn on tracking.

To start DayTrace with your computer, go to Settings, General and turn on **Start automatically
when you turn on your computer**. **Run in the background** decides whether closing the window keeps
tracking or quits DayTrace.

### GNOME on Wayland

GNOME needs the DayTrace integration to see the focused application. Go to Settings, Tracking,
choose **Install integration**, then log out and back in.

### If the tray icon is missing

Some desktops have no tray. GNOME needs an AppIndicator extension, which Ubuntu includes. Without a
tray, launch DayTrace again to reopen its window, and use the Quit button in Settings to exit.

## Removing DayTrace

Removing the package leaves your data in place. To remove it, use **Delete all history** in Settings
first, or delete the folder listed in [PRIVACY.md](PRIVACY.md). If you installed the GNOME
integration, remove it from Settings before uninstalling.
