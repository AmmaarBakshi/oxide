//! Locating and reading the user's config file.

use std::path::PathBuf;

/// Path to the config file: `$OXIDE_CONFIG` if set, otherwise
/// `~/.oxiderc.toml` (or `./.oxiderc.toml` if the home directory is unknown).
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("OXIDE_CONFIG") {
        return PathBuf::from(p);
    }
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".oxiderc.toml");
    path
}

/// Read the config file's contents, or `None` if it doesn't exist / can't be
/// read.
pub fn read() -> Option<String> {
    std::fs::read_to_string(config_path()).ok()
}
