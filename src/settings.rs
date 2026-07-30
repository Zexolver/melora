use std::fs;
use std::io;
use std::path::PathBuf;

/// The customizable part of Melora's look and feel. Deliberately small --
/// one choice, not a general theming engine -- but structured so growing
/// it (accent color, font size, tab width, ...) later is additive, not a
/// redesign: add a field, add it to `save`/`parse`, add it to the
/// settings panel in `melora.slint`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    fn as_byte(self) -> u8 {
        match self {
            Theme::Dark => 0,
            Theme::Light => 1,
        }
    }

    fn from_byte(b: u8) -> Option<Theme> {
        match b {
            0 => Some(Theme::Dark),
            1 => Some(Theme::Light),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Theme::Dark }
    }
}

const MAGIC: &[u8; 4] = b"MLC1";

/// Reads and writes the on-disk settings file -- one byte of real payload
/// today, but file-based (rather than e.g. squeezed into the session
/// file) so it survives independently of session save/restore/discard,
/// and grows without touching that format at all.
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new() -> Self {
        Self { path: crate::paths::data_dir().join("settings.bin") }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// The saved settings, or `Settings::default()` if there's no file yet
    /// (first run) or it's unreadable/corrupt -- a bad settings file
    /// should never stop the browser from starting.
    pub fn load(&self) -> Settings {
        let Ok(bytes) = fs::read(&self.path) else {
            return Settings::default();
        };
        if bytes.len() < 5 || &bytes[0..4] != MAGIC {
            return Settings::default();
        }
        let theme = Theme::from_byte(bytes[4]).unwrap_or(Theme::Dark);
        Settings { theme }
    }

    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        let mut buf = Vec::with_capacity(5);
        buf.extend_from_slice(MAGIC);
        buf.push(settings.theme.as_byte());

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, &buf)?;
        fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> SettingsStore {
        let path = std::env::temp_dir().join(format!(
            "melora-settings-test-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        SettingsStore::at(path)
    }

    #[test]
    fn loading_with_no_file_present_returns_defaults() {
        let store = temp_store();
        assert_eq!(store.load(), Settings::default());
    }

    #[test]
    fn round_trips_a_saved_theme() {
        let store = temp_store();
        store.save(&Settings { theme: Theme::Light }).unwrap();
        assert_eq!(store.load(), Settings { theme: Theme::Light });

        store.save(&Settings { theme: Theme::Dark }).unwrap();
        assert_eq!(store.load(), Settings { theme: Theme::Dark });

        let _ = fs::remove_file(&store.path);
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_instead_of_failing() {
        let store = temp_store();
        fs::write(&store.path, b"not a settings file").unwrap();
        assert_eq!(store.load(), Settings::default());
        let _ = fs::remove_file(&store.path);
    }
}
