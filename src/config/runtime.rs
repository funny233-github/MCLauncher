//! Runtime configuration structures for the Minecraft launcher.
//!
//! Contains user-configurable settings including mirror URLs, mod loaders,
//! mod configurations, and the main runtime configuration.

use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Mirror URLs for downloading Minecraft resources.
///
/// Contains endpoints for downloading version manifests, assets, client files,
/// libraries, and Fabric loader components from various mirror sources.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MCMirror {
    /// URL for version manifest downloads.
    pub version_manifest: String,
    /// URL for asset downloads.
    pub assets: String,
    /// URL for client downloads.
    pub client: String,
    /// URL for library downloads.
    pub libraries: String,
    /// URL for Fabric metadata downloads.
    pub fabric_meta: String,
    /// URL for Fabric Maven downloads.
    pub fabric_maven: String,
    /// URL for `NeoForgeForge` downloads.
    pub neoforge_forge: String,
    /// URL for `NeoForgeNeoForge` downloads.
    pub neoforge_neoforge: String,
}

impl MCMirror {
    /// Creates a new mirror configuration using official Mojang servers.
    ///
    /// Uses the official Minecraft download servers and Fabric Maven repository.
    #[must_use]
    pub fn official_mirror() -> Self {
        MCMirror {
            version_manifest: "https://piston-meta.mojang.com/".into(),
            assets: "https://resources.download.minecraft.net/".into(),
            client: "https://launcher.mojang.com/".into(),
            libraries: "https://libraries.minecraft.net/".into(),
            fabric_meta: "https://meta.fabricmc.net/".into(),
            fabric_maven: "https://maven.fabricmc.net/".into(),
            neoforge_forge: "https://maven.neoforged.net/releases/net/neoforged/forge".into(),
            neoforge_neoforge: "https://maven.neoforged.net/releases/net/neoforged/neoforge".into(),
        }
    }

    /// Creates a new mirror configuration using BMCLAPI (Bangbang93) servers.
    ///
    /// Uses BMCLAPI servers in China, which provide faster download speeds
    /// for Chinese users due to domestic mirrors.
    #[must_use]
    pub fn bmcl_mirror() -> Self {
        MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".into(),
            assets: "https://bmclapi2.bangbang93.com/assets/".into(),
            client: "https://bmclapi2.bangbang93.com/".into(),
            libraries: "https://bmclapi2.bangbang93.com/maven/".into(),
            fabric_meta: "https://bmclapi2.bangbang93.com/fabric-meta/".into(),
            fabric_maven: "https://bmclapi2.bangbang93.com/maven/".into(),
            neoforge_forge: "https://bmclapi2.bangbang93.com/maven/net/neoforged/forge".into(),
            neoforge_neoforge: "https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge"
                .into(),
        }
    }
}

/// Minecraft mod loader type and version.
///
/// Represents the mod loader to use with the game, such as Fabric.
/// The `None` variant indicates vanilla Minecraft without any mod loader.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MCLoader {
    /// No mod loader (vanilla Minecraft).
    None,
    /// Fabric mod loader with specified version.
    Fabric(String),
    /// Neoforge mod loader with specified version.
    Neoforge(String),
}

/// Configuration for a mod in the runtime config.
///
/// Specifies either a specific version to download from Modrinth or a local
/// file name to use. Only one of these should be set at a time.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ModConfig {
    /// Version string to download from Modrinth.
    pub version: Option<String>,
    /// Name of the local mod file in the mods directory.
    pub file_name: Option<String>,
}

impl From<Version> for ModConfig {
    /// Creates a mod configuration from a Modrinth version.
    ///
    /// Extracts the version number from the Modrinth version data.
    /// The `file_name` field is set to None since the file name will be
    /// determined during download and stored in the locked config.
    fn from(version: Version) -> Self {
        Self {
            version: Some(version.version_number),
            file_name: None,
        }
    }
}

impl ModConfig {
    /// Creates a mod configuration for a local mod file.
    ///
    /// # Example
    /// ```
    /// use gluon::config::ModConfig;
    /// let config = ModConfig::from_local("fabric-api.jar");
    /// assert!(config.file_name.is_some());
    /// assert!(config.version.is_none());
    /// ```
    #[must_use]
    pub fn from_local(file_name: &str) -> Self {
        Self {
            version: None,
            file_name: Some(file_name.to_owned()),
        }
    }
}

/// Runtime configuration for the Minecraft launcher.
///
/// Contains user-configurable settings for game launching, including memory,
/// window size, game directory, Java path, loader type, mirror URLs, and mods.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory allocation in MB.
    pub max_memory_size: u32,
    /// Window width in pixels.
    pub window_weight: u32,
    /// Window height in pixels.
    pub window_height: u32,
    /// Path to the game directory. To get `game_dir`, use `ConfigHandler::get_absolute_game_dir`
    /// instead.
    pub(crate) game_dir: String,
    /// Minecraft versions directory string.
    pub game_version: String,
    /// Path to Java executable.
    pub java_path: String,
    /// Minecraft vanilla version string,
    pub vanilla: String,
    /// Mod loader type and version.
    pub loader: MCLoader,
    /// Mirror URLs for downloads.
    pub mirror: MCMirror,
    /// Mod configurations keyed by mod name.
    pub mods: Option<BTreeMap<String, ModConfig>>,
}

impl RuntimeConfig {
    /// Adds a mod configuration.
    ///
    /// # Example
    /// ```
    /// use gluon::config::{RuntimeConfig, ModConfig};
    /// let mut config = RuntimeConfig::default();
    /// let mod_conf = ModConfig::from_local("file name");
    /// config.add_mod("mod name", mod_conf);
    /// ```
    pub fn add_mod(&mut self, name: &str, modconf: ModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(BTreeMap::from([(name.to_owned(), modconf)]));
        }
    }

    /// Adds a local mod by file name.
    ///
    /// # Example
    /// ```
    /// use gluon::config::RuntimeConfig;
    /// let mut config = RuntimeConfig::default();
    /// config.add_local_mod("file name");
    /// ```
    pub fn add_local_mod(&mut self, file_name: &str) {
        let modconf = ModConfig::from_local(file_name);
        self.add_mod(file_name, modconf);
    }

    /// Removes a mod from the configuration.
    ///
    /// # Example
    /// ```
    /// use gluon::config::RuntimeConfig;
    /// let mut config = RuntimeConfig::default();
    /// config.add_local_mod("file name");
    /// config.remove_mod("file name");
    /// ```
    pub fn remove_mod(&mut self, name: &str) {
        if let Some(mods) = self.mods.as_mut() {
            mods.remove(name);
            if mods.is_empty() {
                self.mods = None;
            }
        }
    }
}

impl Default for RuntimeConfig {
    /// Creates a default runtime configuration.
    ///
    /// Sets sensible defaults: 5GB max memory, 854x480 window size, current directory
    /// as game directory, "java" as Java path, no loader (vanilla), and official Mojang mirrors.
    fn default() -> Self {
        RuntimeConfig {
            max_memory_size: 5000,
            window_weight: 854,
            window_height: 480,
            game_dir: "./".into(),
            game_version: "no_game_version".into(),
            java_path: "java".into(),
            vanilla: "no game vanilla version".into(),
            loader: MCLoader::None,
            mirror: MCMirror::official_mirror(),
            mods: None,
        }
    }
}
