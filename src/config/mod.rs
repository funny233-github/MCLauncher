//! # Configuration Management Module
//!
//! Manages runtime configuration, locked mod configuration, and user account information.
//! Handles reading and writing TOML files for the Minecraft launcher.
//!
//! ## Components
//!
//! - [`RuntimeConfig`]: User-editable configuration for game settings and mods
//! - [`LockedConfig`]: Auto-generated configuration with exact mod versions
//! - [`UserAccount`]: Authentication information (offline or Microsoft)
//! - [`ConfigHandler`]: Main handler for reading and writing all configurations
//!
//! ## Configuration Files
//!
//! - `config.toml`: User-editable runtime configuration
//! - `config.lock`: Auto-generated locked configuration with exact mod versions
//! - `account.toml`: User account and authentication information
//!
//! # Example
//! ```no_run
//! use gluon::config::ConfigHandler;
//!
//! let mut config = ConfigHandler::read()?;
//! println!("Game version: {}", config.config().game_version);
//! config.add_mod_local("fabric-api.jar")?;
//! config.write_all()?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use crate::modmanage::{fetch_version, fetch_version_blocking};
use anyhow::{Context, Result};
use clap::Subcommand;
use mc_api::official;
use mc_oauth::MinecraftAuthenticator;
use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

// runtime config
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
    game_dir: String,
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

// version type
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

impl From<Version> for LockedModConfig {
    /// Creates a locked mod config from a Modrinth version.
    ///
    /// Extracts the file information, version number, and download URL from
    /// the version data and reads the current Minecraft version from the config.
    fn from(version: Version) -> Self {
        let file = version.files.clone().remove(0);
        let mc_version = ConfigHandler::read().unwrap().config().game_version.clone();
        Self {
            file_name: file.filename,
            version: Some(version.version_number),
            mc_version,
            url: Some(file.url),
            sha1: Some(file.hashes.sha1),
        }
    }
}
impl LockedModConfig {
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

/// User account information for authentication.
///
/// Contains user details and access token for either offline mode
/// or Microsoft account authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserAccount {
    /// Display username.
    pub user_name: String,
    /// Account type ("offline" or "msa").
    pub user_type: String,
    /// User UUID string.
    pub user_uuid: String,
    /// Access token for Microsoft accounts.
    pub access_token: Option<String>,
}

impl Default for UserAccount {
    /// Creates a default offline user account with a generated UUID.
    fn default() -> Self {
        Self {
            user_name: "noname".to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
            access_token: None,
        }
    }
}

impl UserAccount {
    /// Creates an offline account with the given username.
    ///
    /// # Example
    /// ```
    /// use gluon::config::UserAccount;
    /// let account = UserAccount::new_offline("Steve");
    /// assert_eq!(account.user_name, "Steve");
    /// assert_eq!(account.user_type, "offline");
    /// ```
    #[must_use]
    pub fn new_offline(name: &str) -> Self {
        Self {
            user_name: name.to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
            access_token: None,
        }
    }

    /// Creates a new Microsoft account by authenticating through device code flow.
    ///
    /// This method initiates an interactive authentication process where the user
    /// must visit a URL and enter a code to authorize the application.
    ///
    /// # Errors
    /// - `anyhow::Error` if Microsoft device flow initialization fails
    /// - `anyhow::Error` if user authentication times out
    /// - `anyhow::Error` if Xbox Live authentication fails
    /// - `anyhow::Error` if Minecraft authentication fails
    pub fn new_microsoft() -> anyhow::Result<Self> {
        // Step 1: Start device flow
        let device_flow_state = MinecraftAuthenticator::from_compile_env().start_device_flow()?;
        println!("{}", device_flow_state.initial_response.message);

        // Step 2: Wait for token
        let token_state = device_flow_state.wait_for_token()?;
        println!("Got access token");

        // Step 3: Request Xbox Live token
        let xbox_live_state = token_state.request_xbox_token()?;
        println!("Authenticated with Xbox Live");

        // Step 4: Request XSTS token
        let xsts_state = xbox_live_state.request_xsts_token()?;
        println!("Got XSTS token");

        // Step 5: Request Minecraft token
        let minecraft_state = xsts_state.request_minecraft_token()?;
        println!("Authenticated with Minecraft");

        // Step 6: Fetch Minecraft profile
        let profile = minecraft_state.fetch_minecraft_profile()?;
        println!("Got Minecraft profile: {}", profile.name);
        Ok(Self {
            user_name: profile.name,
            user_type: "msa".into(),
            user_uuid: profile.id,
            access_token: minecraft_state.minecraft_token_data.access_token.into(),
        })
    }
}

/// Mutable access counter wrapper.
///
/// Tracks whether a value has been accessed mutably to enable
/// selective writing of only modified configuration fields.
#[derive(Debug, Clone)]
struct Mac<T> {
    /// The wrapped value.
    value: T,
    /// Whether the value has been accessed mutably.
    has_mut_accessed: bool,
}

impl<T> Mac<T> {
    /// Creates a new wrapper with the given value.
    ///
    /// The mutable access flag is initially set to false.
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            value,
            has_mut_accessed: false,
        }
    }

    /// Returns a reference to the wrapped value.
    #[inline]
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Returns a mutable reference to the wrapped value.
    ///
    /// Sets the mutable access flag to true.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.has_mut_accessed = true;
        &mut self.value
    }

    /// Checks if the value has been accessed mutably.
    #[inline]
    pub fn has_mut_accessed(&self) -> bool {
        self.has_mut_accessed
    }
}

impl<T> From<T> for Mac<T> {
    /// Creates a wrapper from a value.
    #[inline]
    fn from(value: T) -> Self {
        Mac::new(value)
    }
}

impl<T> Deref for Mac<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> DerefMut for Mac<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// Configuration file paths.
///
/// Stores the paths for the three configuration files:
/// config.toml, config.lock, and account.toml.
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// Path to config.toml file.
    config: String,
    /// Path to config.lock file.
    locked_config: String,
    /// Path to account.toml file.
    user_account: String,
}

impl Default for ConfigPaths {
    /// Creates default paths: config.toml, config.lock, and account.toml.
    fn default() -> Self {
        Self {
            config: "config.toml".into(),
            locked_config: "config.lock".into(),
            user_account: "account.toml".into(),
        }
    }
}

/// # Writing with Mutable Access
///
/// When a mutable reference is used with `ConfigHandler.config`, the `config` field is
/// updated and written to the file upon `ConfigHandler`'s drop.
/// In contrast, the `locked_config` field will not write.
///
/// Main handler for managing all launcher configurations.
///
/// Provides methods for reading, writing, and modifying runtime config,
/// locked config, and user account information. Automatically writes
/// modified configurations when dropped.
#[derive(Debug, Clone)]
pub struct ConfigHandler {
    /// Runtime configuration with mutable access tracking.
    config: Mac<RuntimeConfig>,
    /// Locked configuration with mutable access tracking.
    locked_config: Mac<LockedConfig>,
    /// User account with mutable access tracking.
    user_account: Mac<UserAccount>,
    /// Paths to configuration files.
    paths: ConfigPaths,
}

impl ConfigHandler {
    /// Returns a reference to the runtime configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Returns a mutable reference to the runtime configuration.
    ///
    /// Accessing the config mutably marks it as modified, causing it to be
    /// written when the handler is dropped.
    #[inline]
    pub fn config_mut(&mut self) -> &mut RuntimeConfig {
        &mut self.config
    }

    /// Returns a reference to the locked configuration.
    #[inline]
    #[must_use]
    pub fn locked_config(&self) -> &LockedConfig {
        &self.locked_config
    }

    /// Returns a mutable reference to the locked configuration.
    ///
    /// Accessing the locked config mutably marks it as modified, causing it to be
    /// written when the handler is dropped.
    #[inline]
    pub fn locked_config_mut(&mut self) -> &mut LockedConfig {
        &mut self.locked_config
    }

    /// Returns a reference to the user account.
    #[inline]
    #[must_use]
    pub fn user_account(&self) -> &UserAccount {
        &self.user_account
    }

    /// Returns a mutable reference to the user account.
    ///
    /// Accessing the user account mutably marks it as modified, causing it to be
    /// written when the handler is dropped.
    #[inline]
    pub fn user_account_mut(&mut self) -> &mut UserAccount {
        &mut self.user_account
    }

    /// Initializes the configuration handler with default paths.
    ///
    /// Creates default configuration files if they don't exist.
    ///
    /// # Errors
    /// - `anyhow::Error` if the configuration files cannot be written.
    pub fn init() -> Result<()> {
        ConfigHandler::init_for_paths(ConfigPaths::default())
    }

    /// Initializes the configuration handler with custom paths.
    ///
    /// Creates configuration files at the specified paths if they don't exist.
    ///
    /// # Errors
    /// - `anyhow::Error` if configuration files do not exist
    /// - `anyhow::Error` if configuration files contain invalid TOML
    /// - `anyhow::Error` if configuration validation fails
    pub fn init_for_paths(paths: ConfigPaths) -> Result<()> {
        let handle = Self {
            config: Mac::new(RuntimeConfig::default()),
            locked_config: Mac::new(LockedConfig::default()),
            user_account: Mac::new(UserAccount::default()),
            paths,
        };
        handle.write_all()?;
        Ok(())
    }

    /// Checks if a mod exists in the runtime config.
    #[must_use]
    pub fn has_mod_name(&self, mod_name: &str) -> bool {
        self.config()
            .mods
            .clone()
            .is_some_and(|mods| mods.iter().any(|(name, _)| name == mod_name))
    }

    /// Checks if a mod exists in the locked config.
    #[must_use]
    pub fn has_locked_mod_name(&self, mod_name: &str) -> bool {
        self.locked_config()
            .mods
            .clone()
            .is_some_and(|mods| mods.iter().any(|(name, _)| name == mod_name))
    }

    /// Checks if a mod configuration matches the given config.
    #[must_use]
    pub fn is_mod_config_match(&self, name: &str, mod_conf: &ModConfig) -> bool {
        self.config()
            .mods
            .clone()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
    }

    /// Checks if a locked mod configuration matches the given config.
    #[must_use]
    pub fn is_locked_mod_config_match(&self, name: &str, mod_conf: &LockedModConfig) -> bool {
        self.locked_config()
            .mods
            .clone()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
    }

    fn find_config_root() -> Result<std::path::PathBuf> {
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

    /// Reads configuration files by searching for config.toml upward from current directory.
    ///
    /// Searches upward from the current working directory until it finds config.toml,
    /// then uses that directory as the root for all configuration files.
    /// Reads config.toml, config.lock, and account.toml. If config.lock or
    /// account.toml don't exist, they are created with default values.
    ///
    /// # Errors
    /// - `anyhow::Error` if config.toml is not found (search reaches filesystem root)
    /// - `anyhow::Error` if config.toml contains invalid TOML
    /// - `anyhow::Error` if configuration validation fails
    pub fn read() -> Result<Self> {
        let root = Self::find_config_root()?;
        let paths = ConfigPaths {
            config: root.join("config.toml").display().to_string(),
            locked_config: root.join("config.lock").display().to_string(),
            user_account: root.join("account.toml").display().to_string(),
        };
        ConfigHandler::read_from_paths(paths)
    }

    /// Reads configuration files from custom paths.
    ///
    /// Reads config.toml, config.lock, and account.toml from the specified paths.
    /// If config.lock or account.toml don't exist, they are created with default values.
    ///
    /// # Errors
    /// - `anyhow::Error` if config.toml does not exist
    /// - `anyhow::Error` if config.toml contains invalid TOML
    /// - `anyhow::Error` if configuration validation fails
    pub fn read_from_paths(paths: ConfigPaths) -> Result<Self> {
        let config = fs::read_to_string(&paths.config)?;
        let config: RuntimeConfig = toml::from_str(&config)?;

        if let Some(mods) = config.mods.as_ref() {
            for (name, conf) in mods {
                if conf.file_name.is_some() && conf.version.is_some() {
                    return Err(anyhow::anyhow!(
                        "The mod {name} have file_name and version in same time!",
                    ));
                }
            }
        }
        let config = Mac::new(config);

        let locked_config = if fs::exists(&paths.locked_config).is_ok() {
            let data = fs::read_to_string(&paths.locked_config)?;
            Mac::new(toml::from_str(&data)?)
        } else {
            Mac::new(LockedConfig::default())
        };

        let user_account = if fs::exists(&paths.user_account).is_ok() {
            Mac::new(toml::from_str(&fs::read_to_string(&paths.user_account)?)?)
        } else {
            Mac::new(UserAccount::default())
        };

        Ok(ConfigHandler {
            config,
            locked_config,
            user_account,
            paths,
        })
    }

    /// Adds an offline account with the given username.
    ///
    /// # Example
    /// ```no_run
    /// use gluon::config::ConfigHandler;
    /// let mut config = ConfigHandler::read().unwrap();
    /// config.add_offline_account("Steve");
    /// ```
    pub fn add_offline_account(&mut self, name: &str) {
        *self.user_account_mut() = UserAccount::new_offline(name);
    }

    /// Adds a Microsoft account to the configuration.
    ///
    /// Initiates an interactive authentication process where the user
    /// must visit a URL and enter a code to authorize the application.
    ///
    /// # Errors
    /// - `anyhow::Error` if Microsoft device flow initialization fails
    /// - `anyhow::Error` if user authentication times out
    /// - `anyhow::Error` if Xbox Live authentication fails
    /// - `anyhow::Error` if Minecraft authentication fails
    pub fn add_microsoft_account(&mut self) -> anyhow::Result<()> {
        *self.user_account_mut() = UserAccount::new_microsoft()?;
        Ok(())
    }

    /// Writes all configuration files to disk.
    ///
    /// Writes the runtime config, locked config, and user account to their
    /// respective files, overwriting any existing content. Also manages mod file
    /// enabling/disabling based on configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if configuration files cannot be written
    /// - `anyhow::Error` if configuration cannot be serialized to TOML
    /// - `anyhow::Error` if mod file enabling/disabling fails
    pub fn write_all(&self) -> Result<()> {
        fs::write(
            &self.paths.config,
            toml::to_string_pretty(self.config.get())?,
        )?;
        fs::write(
            &self.paths.locked_config,
            toml::to_string_pretty(self.locked_config.get())?,
        )?;
        fs::write(
            &self.paths.user_account,
            toml::to_string_pretty(self.user_account())?,
        )?;

        let mod_dir = Path::new(&self.get_absolute_game_dir()?).join("mods");
        if fs::metadata(mod_dir).is_ok() {
            self.disable_unuse_mods()?;
            self.enable_used_mods()?;
        }
        Ok(())
    }

    /// Writes only modified configuration files to disk.
    ///
    /// Only writes files that have been accessed mutably since the last write.
    /// Also manages mod file enabling/disabling based on configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if modified configuration cannot be written
    /// - `anyhow::Error` if configuration cannot be serialized to TOML
    /// - `anyhow::Error` if mod file enabling/disabling fails
    pub fn write_with_mut(&self) -> Result<()> {
        if self.config.has_mut_accessed() {
            log::debug!("write config");
            fs::write(
                &self.paths.config,
                toml::to_string_pretty(self.config.get())?,
            )?;
        }
        if self.locked_config.has_mut_accessed() {
            log::debug!("write locked config");
            fs::write(
                &self.paths.locked_config,
                toml::to_string_pretty(self.locked_config.get())?,
            )?;
        }
        if self.user_account.has_mut_accessed() {
            log::debug!("write user account");
            fs::write(
                &self.paths.user_account,
                toml::to_string_pretty(self.user_account())?,
            )?;
        }

        let mod_dir = Path::new(&self.get_absolute_game_dir()?).join("mods");
        if fs::exists(mod_dir)? {
            log::debug!("disable unuse mods and enable used mods");
            self.disable_unuse_mods()?;
            self.enable_used_mods()?;
        }
        Ok(())
    }

    /// Adds a local mod to the configuration.
    ///
    /// The mod file must exist in the game's mods directory.
    ///
    /// # Errors
    /// - `anyhow::Error` if the mod file does not exist
    pub fn add_mod_local(&mut self, name: &str) -> Result<()> {
        // Error when file not found
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

        let locked_modconf = LockedModConfig::from(version);
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

        let locked_modconf = LockedModConfig::from(version);
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

        let locked_modconf = LockedModConfig::from(version);

        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Removes a mod from both config and locked config.
    ///
    /// Also removes the mod file from the game's mods directory.
    ///
    /// # Panics
    /// Panics if mod is not found in config.lock.
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
        let mod_dir = Path::new(&self.config().game_dir).join("mods");
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
                    let path = Path::new("mods").join(name);
                    let mut new_name = name.to_string();
                    new_name.truncate(name.len() - 6);
                    let new_path = Path::new("mods").join(new_name);
                    fs::rename(path, new_path)?;
                }
            }
        }
        Ok(())
    }

    /// TODO:
    /// Complete docs
    ///
    /// # Errors
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

impl Drop for ConfigHandler {
    /// Automatically writes modified configurations when dropped.
    ///
    /// Calls `write_with_mut()` to persist any configuration changes that were
    /// made through mutable access.
    ///
    /// # Panics
    /// Panics if writing fails.
    #[inline]
    fn drop(&mut self) {
        if let Err(e) = self.write_with_mut() {
            panic!("Failed to write config on drop: {e}");
        }
    }
}
