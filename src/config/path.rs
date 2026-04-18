//! Path-related methods for `ConfigHandler`.

use super::ConfigHandler;
use anyhow::{Context, Result};
use std::path::Path;

impl ConfigHandler {
    /// Searches upward for config.toml starting from the current directory.
    ///
    /// Returns the directory containing config.toml, or an error if the
    /// filesystem root is reached without finding it.
    ///
    /// # Errors
    /// Returns an error if config.toml is not found before reaching the filesystem root.
    pub(crate) fn find_config_root() -> Result<std::path::PathBuf> {
        let mut current = std::env::current_dir()?;
        loop {
            let config_path = current.join("config.toml");
            if config_path.exists() {
                return Ok(current);
            }
            current = current
                .parent()
                .context("reached filesystem root without finding config.toml")?
                .to_path_buf();
        }
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
