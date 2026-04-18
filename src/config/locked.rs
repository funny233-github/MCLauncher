//! Locked configuration structures for the Minecraft launcher.
//!
//! Contains auto-generated configuration with exact mod versions, file names,
//! and checksums. These configurations should not be manually edited.

use clap::Subcommand;
use mc_api::official;
use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Minecraft version type for filtering version lists.
///
/// Used when listing available Minecraft versions to filter by release type.
#[derive(Subcommand, Debug)]
pub enum VersionType {
    /// Include all versions (releases and snapshots).
    All,
    /// Include only release versions.
    Release,
    /// Include only snapshot versions.
    Snapshot,
}

impl From<VersionType> for official::VersionType {
    fn from(r#type: VersionType) -> Self {
        match r#type {
            VersionType::All => official::VersionType::All,
            VersionType::Release => official::VersionType::Release,
            VersionType::Snapshot => official::VersionType::Snapshot,
        }
    }
}

/// Locked configuration for a mod with exact file information.
///
/// Contains the resolved file name, version, download URL, and checksum for
/// a specific mod. This is auto-generated and should not be manually edited.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LockedModConfig {
    /// Name of the mod file.
    pub file_name: String,
    /// Version string if downloaded from Modrinth.
    pub version: Option<String>,
    /// Minecraft version this mod is compatible with.
    pub mc_version: String,
    /// Download URL if downloaded from Modrinth.
    pub url: Option<String>,
    /// SHA1 checksum of the mod file.
    pub sha1: Option<String>,
}

impl LockedModConfig {
    /// Creates a locked mod config from a Modrinth version.
    ///
    /// Extracts the file information, version number, and download URL from
    /// the version data. The `mc_version` must be provided explicitly to avoid
    /// circular dependency on `ConfigHandler`.
    ///
    /// # Example
    /// ```
    /// use gluon::config::LockedModConfig;
    /// use modrinth_api::{Version, VersionFile, Hashes};
    ///
    /// let version = Version {
    ///     name: String::from("Fabric API"),
    ///     version_number: String::from("0.92.0"),
    ///     game_versions: vec![String::from("1.20.1")],
    ///     version_type: String::from("release"),
    ///     loaders: vec![String::from("fabric")],
    ///     files: vec![VersionFile {
    ///         filename: String::from("fabric-api-0.92.0.jar"),
    ///         hashes: Hashes {
    ///             sha1: String::from("abc123"),
    ///             sha512: String::from("def456"),
    ///         },
    ///         url: String::from("https://example.com/fabric-api-0.92.0.jar"),
    ///     }],
    /// };
    /// let locked = LockedModConfig::from_version(version, "1.20.1");
    /// assert_eq!(locked.file_name, "fabric-api-0.92.0.jar");
    /// assert_eq!(locked.mc_version, "1.20.1");
    /// ```
    #[must_use]
    pub fn from_version(version: Version, mc_version: &str) -> Self {
        let file = version.files.clone().remove(0);
        Self {
            file_name: file.filename,
            version: Some(version.version_number),
            mc_version: mc_version.to_owned(),
            url: Some(file.url),
            sha1: Some(file.hashes.sha1),
        }
    }

    /// Creates a locked config for a local mod file.
    ///
    /// # Example
    /// ```
    /// use gluon::config::LockedModConfig;
    /// let config = LockedModConfig::from_local("fabric-api.jar", "1.20.1");
    /// assert_eq!(config.file_name, "fabric-api.jar");
    /// assert_eq!(config.mc_version, "1.20.1");
    /// ```
    #[must_use]
    pub fn from_local(file_name: &str, mc_version: &str) -> Self {
        Self {
            file_name: file_name.to_owned(),
            version: None,
            mc_version: mc_version.to_owned(),
            url: None,
            sha1: None,
        }
    }
}

/// Locked configuration with resolved mod information.
///
/// Contains the exact versions, file names, and checksums for all configured mods.
/// This file is auto-generated and should not be manually edited.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LockedConfig {
    /// Mod configurations keyed by mod name.
    pub mods: Option<BTreeMap<String, LockedModConfig>>,
}

impl LockedConfig {
    /// Adds a mod configuration with locked version information.
    ///
    /// # Example
    /// ```
    /// use gluon::config::{LockedConfig, LockedModConfig};
    /// let mut config = LockedConfig::default();
    /// let mod_conf = LockedModConfig::from_local("file name","1.1.1");
    /// config.add_mod("mod name", mod_conf);
    /// ```
    pub fn add_mod(&mut self, name: &str, modconf: LockedModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(BTreeMap::from([(name.to_owned(), modconf)]));
        }
    }

    /// Adds a local mod configuration.
    ///
    /// # Example
    /// ```
    /// use gluon::config::LockedConfig;
    /// let mut config = LockedConfig::default();
    /// config.add_local_mod("file name","1.1.1");
    /// ```
    pub fn add_local_mod(&mut self, name: &str, mc_version: &str) {
        self.add_mod(name, LockedModConfig::from_local(name, mc_version));
    }

    /// Removes a mod from the locked configuration.
    ///
    /// # Example
    /// ```
    /// use gluon::config::LockedConfig;
    /// let mut config = LockedConfig::default();
    /// config.add_local_mod("file name","1.1.1");
    /// config.remove_mod("file name");
    /// ```
    ///
    pub fn remove_mod(&mut self, name: &str) {
        if let Some(mods) = self.mods.as_mut() {
            mods.remove(name);
            if mods.is_empty() {
                self.mods = None;
            }
        }
    }
}
