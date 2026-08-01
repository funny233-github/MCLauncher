//! Path-related methods for `ConfigHandler`.

use super::ConfigHandler;
use anyhow::{Context, Result};
use std::path::Path;

/// Searches upward for `config.toml` starting from the given path.
///
/// The given path may be a directory or a file. Returns the directory
/// containing `config.toml`, or `None` if the filesystem root is reached
/// without finding it.
pub(super) fn try_find_config_root_from(path: &Path) -> Option<std::path::PathBuf> {
    // Relative paths have no directory component to climb up from (e.g. "config.toml"
    // pops to an empty path), so normalize to an absolute path first. `absolute` does
    // not touch the filesystem and does not require the path to exist.
    let mut current = std::path::absolute(path).ok()?;
    loop {
        if current.join("config.toml").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

impl ConfigHandler {
    /// Searches upward for config.toml starting from the current directory.
    ///
    /// Returns the directory containing config.toml, or `None` if the current
    /// directory cannot be determined or the filesystem root is reached
    /// without finding it.
    pub(crate) fn try_find_config_root() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        try_find_config_root_from(&cwd)
    }

    /// Gets the absolute path to the game directory.
    ///
    /// This method resolves `game_dir` to an absolute path for internal use.
    /// If `game_dir` in `config.toml` is already an absolute path, it is returned as-is.
    /// If `game_dir` is a relative path, it is resolved relative to the directory
    /// containing `config.toml`.
    ///
    /// This ensures that file operations always work correctly regardless of the
    /// current working directory when the program is run.
    ///
    /// # Errors
    /// - Returns an error if the parent directory of `config.toml` cannot be determined
    /// - Returns an error if the resolved path cannot be converted to a valid string
    pub fn get_absolute_game_dir(&self) -> Result<String> {
        let config_path = Path::new(&self.paths.config)
            .parent()
            .with_context(|| {
                format!(
                    "Failed to get parent directory of config file: {}",
                    self.paths.config
                )
            })?;
        let game_dir = Path::new(&self.config.game_dir);
        if game_dir.is_absolute() {
            return Ok(self.config.game_dir.clone());
        }
        let path = config_path.join(game_dir);
        let path_str = path
            .to_str()
            .with_context(|| {
                format!("Failed to convert path to string: {}", path.display())
            })?;
        Ok(path_str.to_string())
    }
}
