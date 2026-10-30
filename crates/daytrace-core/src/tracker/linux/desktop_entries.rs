//! Friendly application names and icons from freedesktop `.desktop` files.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tracker::ForegroundApp;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    icon: Option<String>,
}

#[derive(Debug, Default)]
pub struct DesktopEntries {
    by_key: HashMap<String, Entry>,
    data_dirs: Vec<PathBuf>,
}

fn data_dirs() -> Vec<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
    let shared = env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    let mut directories: Vec<PathBuf> = data_home.into_iter().collect();
    directories.extend(shared.split(':').filter(|path| !path.is_empty()).map(PathBuf::from));
    directories.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    directories.push(PathBuf::from("/var/lib/snapd/desktop"));
    if let Some(home) = home {
        directories.push(home.join(".local/share/flatpak/exports/share"));
    }
    directories
}

/// Reads the keys DayTrace needs from a `.desktop` file's main section.
fn parse(contents: &str) -> Option<(Entry, Option<String>, Option<String>)> {
    let mut in_main_section = false;
    let (mut name, mut icon, mut wm_class, mut exec) = (None, None, None, None);

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_main_section {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().to_string();
            match key.trim() {
                "Name" => name = Some(value),
                "Icon" => icon = Some(value),
                "StartupWMClass" => wm_class = Some(value),
                "Exec" => exec = Some(value),
                _ => {}
            }
        }
    }

    let command = exec.and_then(|exec| {
        let program = exec.split_whitespace().find(|word| !word.contains('='))?.to_string();
        Path::new(&program).file_name().map(|name| name.to_string_lossy().to_string())
    });
    Some((Entry { name: name?, icon }, wm_class, command))
}

impl DesktopEntries {
    pub fn load() -> Self {
        let data_dirs = data_dirs();
        let mut entries = Self { by_key: HashMap::new(), data_dirs: data_dirs.clone() };
        for directory in data_dirs.iter().rev() {
            entries.scan(&directory.join("applications"));
        }
        entries
    }

    fn scan(&mut self, directory: &Path) {
        let Ok(files) = fs::read_dir(directory) else { return };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_none_or(|extension| extension != "desktop") {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else { continue };
            let Some((entry, wm_class, command)) = parse(&contents) else { continue };

            let stem = path.file_stem().map(|stem| stem.to_string_lossy().to_lowercase());
            for key in [stem, wm_class.map(|c| c.to_lowercase()), command.map(|c| c.to_lowercase())]
                .into_iter()
                .flatten()
            {
                self.by_key.insert(key, entry.clone());
            }
        }
    }

    fn find(&self, keys: &[&str]) -> Option<&Entry> {
        keys.iter().find_map(|key| {
            let key = key.trim_end_matches(".desktop").to_lowercase();
            self.by_key.get(&key)
        })
    }

    fn icon_path(&self, icon: &str) -> Option<String> {
        if Path::new(icon).is_absolute() {
            return Path::new(icon).exists().then(|| icon.to_string());
        }
        const SIZES: [&str; 6] = ["256x256", "128x128", "96x96", "64x64", "48x48", "scalable"];
        for directory in &self.data_dirs {
            for size in SIZES {
                for extension in ["png", "svg"] {
                    let path = directory.join(format!("icons/hicolor/{size}/apps/{icon}.{extension}"));
                    if path.exists() {
                        return Some(path.to_string_lossy().to_string());
                    }
                }
            }
            for extension in ["png", "svg"] {
                let path = directory.join(format!("pixmaps/{icon}.{extension}"));
                if path.exists() {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
        None
    }

    /// Fills in the display name and icon. The identifier is kept as reported.
    pub fn enrich(&self, mut app: ForegroundApp) -> ForegroundApp {
        let command = app
            .executable_path
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .map(|name| name.to_string_lossy().to_string());

        let mut keys = vec![app.identifier.as_str()];
        keys.extend(command.as_deref());
        if let Some(entry) = self.find(&keys) {
            app.name = entry.name.clone();
            app.icon_path = entry.icon.as_deref().and_then(|icon| self.icon_path(icon));
        }
        app
    }
}

/// A readable name for an identifier with no desktop file, such as turning
/// `org.gnome.TextEditor` into `TextEditor`.
pub fn friendly_name(identifier: &str) -> String {
    let last = identifier.trim_end_matches(".desktop").rsplit('.').next().unwrap_or(identifier);
    let mut characters = last.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => identifier.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "[Desktop Entry]\nName=Visual Studio Code\nExec=env FOO=1 /usr/share/code/code %F\nIcon=vscode\nStartupWMClass=Code\n[Desktop Action new]\nName=New Window\n";

    #[test]
    fn parses_only_the_main_section() {
        let (entry, wm_class, command) = parse(CODE).unwrap();
        assert_eq!(entry.name, "Visual Studio Code");
        assert_eq!(entry.icon.as_deref(), Some("vscode"));
        assert_eq!(wm_class.as_deref(), Some("Code"));
        assert_eq!(command.as_deref(), Some("env"));
    }

    #[test]
    fn entries_without_a_name_are_ignored() {
        assert!(parse("[Desktop Entry]\nExec=foo\n").is_none());
    }

    #[test]
    fn scanning_matches_by_id_wm_class_and_executable() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("code.desktop"), CODE).unwrap();
        let mut entries = DesktopEntries::default();
        entries.scan(directory.path());

        let app = ForegroundApp { identifier: "code".into(), name: "code".into(), ..Default::default() };
        assert_eq!(entries.enrich(app).name, "Visual Studio Code");

        let by_file = ForegroundApp {
            identifier: "code.desktop".into(),
            name: "x".into(),
            ..Default::default()
        };
        assert_eq!(entries.enrich(by_file).name, "Visual Studio Code");
    }

    #[test]
    fn unknown_apps_keep_their_reported_name() {
        let entries = DesktopEntries::default();
        let app = ForegroundApp { identifier: "mystery".into(), name: "Mystery".into(), ..Default::default() };
        assert_eq!(entries.enrich(app).name, "Mystery");
    }

    #[test]
    fn friendly_names() {
        assert_eq!(friendly_name("org.gnome.TextEditor"), "TextEditor");
        assert_eq!(friendly_name("firefox"), "Firefox");
        assert_eq!(friendly_name("org.mozilla.firefox.desktop"), "Firefox");
    }
}
