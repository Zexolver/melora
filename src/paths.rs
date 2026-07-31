use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;

/// Android has no `$HOME`/`$XDG_DATA_HOME` (the app process gets a near-
/// empty environment) and its writable per-app directory can only be
/// obtained via JNI (`android_activity::AndroidApp::internal_data_path`,
/// the Rust equivalent of `Context.getFilesDir()`) -- there's no
/// environment variable or fixed path to fall back to the way the other
/// platforms below have. `android_main` in `lib.rs` sets this once, before
/// any code that might need `data_dir()` runs.
#[cfg(target_os = "android")]
static ANDROID_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(target_os = "android")]
pub fn set_android_data_dir(path: PathBuf) {
    let _ = ANDROID_DATA_DIR.set(path);
}

/// Resolves a real per-user data directory across platforms without
/// pulling in a directories crate: `$XDG_DATA_HOME` or `~/.local/share`
/// on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on
/// Windows, the app's JNI-provided internal storage dir on Android,
/// falling back to the system temp dir if none of those is available
/// (e.g. a minimal/sandboxed environment). Shared by everything that
/// persists something across restarts -- `session.rs`, `settings.rs`,
/// `swap.rs` -- so there's one place a real `directories`-crate
/// dependency would replace this, if it's ever worth pulling in.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(dir) = ANDROID_DATA_DIR.get() {
        return dir.clone();
    }
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
