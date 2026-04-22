//! Mod management methods for `ConfigHandler`.

use super::{ConfigHandler, LockedModConfig, ModConfig};
use crate::modmanage::{fetch_version, fetch_version_blocking};
use anyhow::Result;
use modrinth_api::Version;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

impl ConfigHandler {
    /// Checks if a mod exists in the runtime config.
    #[must_use]
    pub fn has_mod_name(&self, mod_name: &str) -> bool {
        self.config()
            .mods
            .as_ref()
            .is_some_and(|mods| mods.contains_key(mod_name))
    }

    /// Checks if a mod exists in the locked config.
    #[must_use]
    pub fn has_locked_mod_name(&self, mod_name: &str) -> bool {
        self.locked_config()
            .mods
            .as_ref()
            .is_some_and(|mods| mods.contains_key(mod_name))
    }

    /// Checks if a mod configuration matches the given config.
    #[must_use]
    pub fn is_mod_config_match(&self, name: &str, mod_conf: &ModConfig) -> bool {
        self.config()
            .mods
            .as_ref()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
    }

    /// Checks if a locked mod configuration matches the given config.
    #[must_use]
    pub fn is_locked_mod_config_match(&self, name: &str, mod_conf: &LockedModConfig) -> bool {
        self.locked_config()
            .mods
            .as_ref()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
    }

    /// Adds a local mod to the configuration.
    ///
    /// The mod file must exist in the game's mods directory.
    ///
    /// # Errors
    /// - `anyhow::Error` if the mod file does not exist
    pub fn add_mod_local(&mut self, name: &str) -> Result<()> {
        let path = Path::new(&self.get_absolute_game_dir()?)
            .join("mods")
            .join(name);
        if !fs::exists(path)? {
            return Err(anyhow::anyhow!("The {name} not exist"));
        }

        if !self.has_mod_name(name) {
            self.config_mut().add_local_mod(name);
        }

        if !self.has_locked_mod_name(name) {
            let mc_version = &self.config().game_version.clone();
            self.locked_config_mut().add_local_mod(name, mc_version);
        }

        Ok(())
    }

    /// Adds a remote mod from Modrinth (blocking).
    ///
    /// Fetches the mod version from Modrinth and adds it to both the runtime
    /// and locked configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if network request to Modrinth fails
    /// - `anyhow::Error` if no compatible versions are found
    pub fn add_mod_unlocal_blocking(&mut self, name: &str, version: Option<&String>) -> Result<()> {
        let version = fetch_version_blocking(name, version, &self.config)?.remove(0);

        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from_version(version, &self.config().game_version);
        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Adds a remote mod from Modrinth (async).
    ///
    /// Fetches the mod version from Modrinth and adds it to both the runtime
    /// and locked configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if network request to Modrinth fails
    /// - `anyhow::Error` if no compatible versions are found
    pub async fn add_mod_unlocal(&mut self, name: &str, version: Option<&String>) -> Result<()> {
        let version = fetch_version(name, version, &self.config).await?.remove(0);

        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from_version(version, &self.config().game_version);
        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Adds a mod from Version data to both config and locked config.
    ///
    /// Updates the runtime config with the mod version information and
    /// creates a corresponding entry in the locked config with file details.
    ///
    /// # Errors
    /// - `anyhow::Error` if configuration cannot be serialized to TOML
    /// - `anyhow::Error` if configuration cannot be written to file
    pub fn add_mod_from(&mut self, name: &str, version: Version) -> Result<()> {
        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from_version(version, &self.config().game_version);

        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Removes a mod from both config and locked config.
    ///
    /// Also removes the mod file from the game's mods directory.
    ///
    /// # Errors
    /// - `anyhow::Error` if mod is not found in locked config
    /// - `anyhow::Error` if mod file cannot be removed from filesystem
    /// - `anyhow::Error` if mod configuration cannot be updated
    pub fn remove_mod(&mut self, name: &str) -> Result<()> {
        let locked_mods = self
            .locked_config
            .mods
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No mods in locked config"))?;
        let mod_info = locked_mods
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Mod '{name}' not found in locked config"))?;
        let file_path = Path::new(&self.get_absolute_game_dir()?)
            .join("mods")
            .join(&mod_info.file_name);
        // the config independent with file of mod
        // so the file of mod may not exist
        if fs::metadata(&file_path).is_ok() {
            fs::remove_file(file_path)?;
        }

        self.config_mut().remove_mod(name);
        self.locked_config_mut().remove_mod(name);

        Ok(())
    }

    /// Disables mod files that are not listed in `config.toml` by renaming them.
    ///
    /// Files in the mods directory that are not configured in the config will be
    /// renamed with a `.unuse` extension to prevent them from being loaded.
    ///
    /// # Errors
    /// - `anyhow::Error` if mods directory cannot be accessed
    /// - `anyhow::Error` if file renaming fails
    /// - `anyhow::Error` if file metadata cannot be read
    #[allow(clippy::unnecessary_wraps, reason = "wraps is human readable")]
    #[allow(
        clippy::redundant_closure_for_method_calls,
        reason = "wraps is human readable"
    )]
    #[allow(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "case_sensitive is need"
    )]
    pub fn disable_unuse_mods(&self) -> Result<()> {
        let mod_dir = Path::new(&self.get_absolute_game_dir()?).join("mods");
        if fs::metadata(&mod_dir).is_err() {
            return Ok(());
        }

        // Collect all used mod file names
        let used_files: Vec<String> = match (&self.config().mods, &self.locked_config().mods) {
            (Some(config_mods), Some(locked_mods)) => config_mods
                .keys()
                .filter_map(|name| locked_mods.get(name).map(|m| m.file_name.clone()))
                .collect(),
            _ => Vec::new(),
        };

        for entry in WalkDir::new(&mod_dir).into_iter().filter_map(|e| e.ok()) {
            if let Some(name) = entry.file_name().to_str() {
                // WalkDir returns the directory itself ("mods") as an entry, which we must filter out
                // to avoid incorrectly processing the mods directory as a mod file
                if name == "mods" || name.ends_with(".unuse") {
                    continue;
                }

                if !used_files.contains(&name.to_string()) {
                    let path = mod_dir.join(name);
                    let new_path = mod_dir.join(format!("{name}.unuse"));
                    fs::rename(path, new_path)?;
                }
            }
        }
        Ok(())
    }

    /// Enables mod files that are listed in `config.toml` by renaming them.
    ///
    /// Files in the mods directory that have a `.unuse` extension and are
    /// configured in the config will be renamed back to their original name.
    ///
    /// # Panics
    /// Panics if mod information in locked config is invalid.
    ///
    /// # Errors
    /// - `anyhow::Error` if mods directory cannot be accessed
    /// - `anyhow::Error` if file renaming fails
    /// - `anyhow::Error` if file metadata cannot be read
    #[allow(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "case_sensitive is need"
    )]
    pub fn enable_used_mods(&self) -> Result<()> {
        let mod_dir = Path::new(&self.get_absolute_game_dir()?).join("mods");
        if fs::metadata(&mod_dir).is_err() {
            return Ok(());
        }

        let used_files = match (&self.config().mods, &self.locked_config().mods) {
            (Some(config_mods), Some(locked_mods)) => config_mods
                .keys()
                .filter_map(|name| locked_mods.get(name).map(|m| m.file_name.clone()))
                .collect(),
            _ => Vec::new(),
        };

        for entry in WalkDir::new(&mod_dir)
            .into_iter()
            .filter(|entry| match entry {
                Ok(e) => e
                    .file_name()
                    .to_str()
                    .unwrap_or_else(|| panic!("Failed to convert str to String"))
                    .ends_with(".unuse"),
                Err(e) => panic!("{}", e),
            })
        {
            if let Some(name) = &entry?.file_name().to_str() {
                if used_files.iter().any(|x| &format!("{x}.unuse") == name) {
                    let path = mod_dir.join(name);
                    let mut new_name = name.to_string();
                    new_name.truncate(name.len() - 6);
                    let new_path = mod_dir.join(new_name);
                    fs::rename(path, new_path)?;
                }
            }
        }
        Ok(())
    }
}
