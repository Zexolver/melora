use std::path::PathBuf;

/// Resolves a real per-user data directory across platforms without
/// pulling in a directories crate: `$XDG_DATA_HOME` or `~/.local/share`
/// on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on
/// Windows, falling back to the system temp dir if none of those
/// environment variables are set (e.g. a minimal/sandboxed environment).
/// Shared by everything that persists something across restarts --
/// `session.rs`, `settings.rs` -- so there's one place a real
/// `directories`-crate dependency would replace this, if it's ever
/// worth pulling in.
pub fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("melora");
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("melora");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            if cfg!(target_os = "macos") {
                return PathBuf::from(home).join("Library/Application Support/melora");
            }
            return PathBuf::from(home).join(".local/share/melora");
        }
    }
    std::env::temp_dir().join("melora")
}
