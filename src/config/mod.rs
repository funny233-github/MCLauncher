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

mod account;
mod locked;
mod mod_manage;
mod path;
mod runtime;
mod user;

// Re-export public types
pub use locked::{LockedConfig, LockedModConfig, VersionType};
pub use runtime::{MCLoader, MCMirror, ModConfig, RuntimeConfig};
pub use user::UserAccount;

use anyhow::Result;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::Path;

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

impl Default for ConfigHandler {
    /// Creates a default configuration handler.
    fn default() -> Self {
        Self {
            config: Mac::new(RuntimeConfig::default()),
            locked_config: Mac::new(LockedConfig::default()),
            user_account: Mac::new(UserAccount::default()),
            paths: ConfigPaths::default(),
        }
    }
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
            eprintln!("Failed to write config on drop: {e}");
            std::process::exit(1);
        }
    }
}
