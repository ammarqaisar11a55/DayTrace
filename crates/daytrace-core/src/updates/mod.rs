//! Update decisions that do not depend on the updater itself.
//!
//! How an update is applied depends on how DayTrace was installed. Only the
//! Windows installer and AppImage can update themselves. Packages owned by a
//! system package manager must be upgraded by that package manager.

use std::path::Path;
use std::process::Command;

use semver::Version;
use serde::Serialize;

use crate::config::UpdateChannel;

const STABLE_MANIFEST: &str =
    "https://github.com/ammarqaisar11a55/DayTrace/releases/latest/download/latest.json";
const BETA_MANIFEST: &str =
    "https://github.com/ammarqaisar11a55/DayTrace/releases/download/channel-beta/latest.json";

pub fn manifest_url(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_MANIFEST,
        UpdateChannel::Beta => BETA_MANIFEST,
    }
}

/// Whether `remote` may replace `current`. Older and equal versions are refused
/// so that a replayed manifest cannot downgrade the application, and the stable
/// channel never takes a prerelease.
pub fn should_update(current: &str, remote: &str, channel: UpdateChannel) -> bool {
    let (Ok(current), Ok(remote)) = (Version::parse(current), Version::parse(remote)) else {
        return false;
    };
    if channel == UpdateChannel::Stable && !remote.pre.is_empty() {
        return false;
    }
    remote > current
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMethod {
    Nsis,
    AppImage,
    Deb,
    Rpm,
    Snap,
    Unsupported,
}

impl InstallMethod {
    pub fn can_self_update(self) -> bool {
        matches!(self, Self::Nsis | Self::AppImage)
    }

    /// What the user should do instead, for installs DayTrace must not replace.
    pub fn guidance(self) -> Option<&'static str> {
        match self {
            Self::Deb => Some(
                "This copy was installed as a .deb package. If you added the DayTrace repository, upgrade with apt. Otherwise download the new package from GitHub Releases.",
            ),
            Self::Rpm => Some(
                "This copy was installed as an RPM package. Upgrade it with dnf or your package manager, or download the new package from GitHub Releases.",
            ),
            Self::Snap => Some("Snap updates are installed automatically by snapd."),
            Self::Unsupported => Some(
                "This copy was not installed by a supported installer, so it cannot update itself. Download the latest version from GitHub Releases.",
            ),
            Self::Nsis | Self::AppImage => None,
        }
    }
}

pub struct InstallFacts<'a> {
    pub windows: bool,
    pub debug_build: bool,
    pub appimage: Option<&'a str>,
    pub snap: Option<&'a str>,
    pub owned_by_dpkg: bool,
    pub owned_by_rpm: bool,
}

pub fn classify_install(facts: &InstallFacts<'_>) -> InstallMethod {
    if facts.debug_build {
        InstallMethod::Unsupported
    } else if facts.windows {
        InstallMethod::Nsis
    } else if facts.snap.is_some() {
        InstallMethod::Snap
    } else if facts.appimage.is_some() {
        InstallMethod::AppImage
    } else if facts.owned_by_dpkg {
        InstallMethod::Deb
    } else if facts.owned_by_rpm {
        InstallMethod::Rpm
    } else {
        InstallMethod::Unsupported
    }
}

/// Tauri derives the package name from the product name, so it differs from
/// the binary's name.
const DPKG_FILE_LIST: &str = "/var/lib/dpkg/info/day-trace.list";

fn dpkg_owns(executable: &Path) -> bool {
    std::fs::read_to_string(DPKG_FILE_LIST)
        .is_ok_and(|files| files.lines().any(|line| Path::new(line) == executable))
}

fn rpm_owns(executable: &Path) -> bool {
    Command::new("rpm").arg("-qf").arg(executable).output().is_ok_and(|output| output.status.success())
}

pub fn detect_install() -> InstallMethod {
    let executable = std::env::current_exe().ok();
    let appimage = std::env::var("APPIMAGE").ok();
    let snap = std::env::var("SNAP").ok();
    let owned_by_dpkg = executable.as_deref().is_some_and(dpkg_owns);
    classify_install(&InstallFacts {
        windows: cfg!(windows),
        debug_build: cfg!(debug_assertions),
        appimage: appimage.as_deref(),
        snap: snap.as_deref(),
        owned_by_dpkg,
        owned_by_rpm: !owned_by_dpkg && executable.as_deref().is_some_and(rpm_owns),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> InstallFacts<'static> {
        InstallFacts {
            windows: false,
            debug_build: false,
            appimage: None,
            snap: None,
            owned_by_dpkg: false,
            owned_by_rpm: false,
        }
    }

    #[test]
    fn newer_versions_update() {
        assert!(should_update("1.0.0", "1.0.1", UpdateChannel::Stable));
        assert!(should_update("1.9.0", "1.10.0", UpdateChannel::Stable));
    }

    #[test]
    fn downgrades_and_equal_versions_are_refused() {
        assert!(!should_update("1.2.0", "1.1.9", UpdateChannel::Stable));
        assert!(!should_update("1.2.0", "1.2.0", UpdateChannel::Beta));
    }

    #[test]
    fn prereleases_only_reach_the_beta_channel() {
        assert!(!should_update("1.0.0", "1.1.0-beta.1", UpdateChannel::Stable));
        assert!(should_update("1.0.0", "1.1.0-beta.1", UpdateChannel::Beta));
        assert!(should_update("1.1.0-beta.1", "1.1.0-beta.2", UpdateChannel::Beta));
        assert!(should_update("1.1.0-beta.2", "1.1.0", UpdateChannel::Beta));
    }

    #[test]
    fn malformed_versions_are_refused() {
        assert!(!should_update("1.0.0", "latest", UpdateChannel::Stable));
        assert!(!should_update("nonsense", "1.0.0", UpdateChannel::Stable));
    }

    #[test]
    fn channels_use_separate_manifests() {
        assert_ne!(manifest_url(UpdateChannel::Stable), manifest_url(UpdateChannel::Beta));
        assert!(manifest_url(UpdateChannel::Stable).starts_with("https://"));
        assert!(manifest_url(UpdateChannel::Beta).starts_with("https://"));
    }

    #[test]
    fn install_methods_are_classified() {
        assert_eq!(classify_install(&InstallFacts { windows: true, ..facts() }), InstallMethod::Nsis);
        assert_eq!(
            classify_install(&InstallFacts { appimage: Some("/a"), ..facts() }),
            InstallMethod::AppImage
        );
        assert_eq!(classify_install(&InstallFacts { owned_by_dpkg: true, ..facts() }), InstallMethod::Deb);
        assert_eq!(classify_install(&InstallFacts { owned_by_rpm: true, ..facts() }), InstallMethod::Rpm);
        assert_eq!(classify_install(&facts()), InstallMethod::Unsupported);
    }

    #[test]
    fn snap_wins_because_snaps_are_never_replaced_from_inside() {
        let snap = InstallFacts { snap: Some("/snap/daytrace/1"), appimage: Some("/a"), ..facts() };
        assert_eq!(classify_install(&snap), InstallMethod::Snap);
    }

    #[test]
    fn development_builds_never_self_update() {
        let debug = InstallFacts { debug_build: true, windows: true, ..facts() };
        assert_eq!(classify_install(&debug), InstallMethod::Unsupported);
    }

    #[test]
    fn only_self_contained_installs_can_update_themselves() {
        assert!(InstallMethod::Nsis.can_self_update());
        assert!(InstallMethod::AppImage.can_self_update());
        for method in
            [InstallMethod::Deb, InstallMethod::Rpm, InstallMethod::Snap, InstallMethod::Unsupported]
        {
            assert!(!method.can_self_update());
            assert!(method.guidance().is_some());
        }
    }
}
