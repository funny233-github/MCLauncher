//! Minecraft mod management module.
//!
//! This module provides comprehensive functionality for managing Minecraft mods,
//! including fetching, installing, updating, syncing, and searching mods from Modrinth.
//! It supports both local mod files and remote mod downloads.
//!
//! # Features
//!
//! - **Mod Discovery**: Search for mods on Modrinth by name
//! - **Version Management**: Fetch available versions and filter by game version and loader
//! - **Installation**: Download and install mods with SHA-1 verification
//! - **Updates**: Update all mods to their latest compatible versions
//! - **Syncing**: Sync mods to exact versions specified in configuration
//! - **Cleanup**: Remove unused mods and stale mod files
//! - **Local Mods**: Support for manually added local mod files
//!
//! # Architecture
//!
//! The mod management system uses two configuration files:
//!
//! - `config.toml` - User-defined mod list with version preferences
//! - `config.lock` - Locked versions with download URLs and checksums
//!
//! # Workflow
//!
//! 1. **Add Mod**: Fetch mod info from Modrinth or use local file
//! 2. **Sync/Update**: Update config.lock with correct versions
//! 3. **Install**: Download and verify mods to game directory
//! 4. **Clean**: Remove unused mods from config and filesystem
//!
//! # Example
//!
//! ```no_run
//! use launcher::modmanage::{add, update, install};
//!
//! // Add a mod from Modrinth
//! add("fabric-api", None, false, false).expect("Failed to add mod");
//!
//! // Update all mods to latest versions
//! update(false).expect("Failed to update mods");
//!
//! // Install mods to game directory
//! install().expect("Failed to install mods");
//! ```

use crate::config::{ConfigHandler, MCLoader, ModConfig, RuntimeConfig};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use installer::{InstallTask, TaskPool};
use modrinth_api::{Projects, Version, Versions};
use std::{
    collections::VecDeque,
    fs,
    path::Path,
    sync::{Arc, RwLock},
};
use tabled::{settings::Style, Table, Tabled};
use walkdir::WalkDir;

/// Checks if a mod version is compatible with the current configuration.
///
/// This function verifies that a mod version supports both the configured
/// game version and the configured mod loader (e.g., Fabric).
///
/// # Arguments
///
/// * `version` - The mod version to check
/// * `config` - Runtime configuration containing game version and loader
///
/// # Returns
///
/// `true` if the version is compatible with the current configuration, `false` otherwise.
///
/// # Compatibility Rules
///
/// - The version's `game_versions` must include the configured game version
/// - The version's `loaders` must include the configured loader (if any)
/// - Mods cannot be installed without a loader (`MCLoader::None` is not supported)
fn is_version_supported(version: &Version, config: &RuntimeConfig) -> bool {
    version
        .game_versions
        .iter()
        .any(|x| x == &config.game_version)
        && version.loaders.iter().any(|x| match config.loader {
            MCLoader::None => false,
            MCLoader::Fabric(_) => x == "fabric",
        })
}

/// Filters mod versions based on version number and compatibility.
///
/// This function takes a list of mod versions and filters them to only
/// include versions that are compatible with the current game version
/// and loader, optionally also filtering by a specific version number.
///
/// # Arguments
///
/// * `versions` - List of all available versions for a mod
/// * `version` - Optional version number to filter for (e.g., "1.0.0")
/// * `config` - Runtime configuration containing game version and loader
/// * `name` - Name of the mod (used for error messages)
///
/// # Returns
///
/// A filtered list of compatible versions.
///
/// # Errors
///
/// Returns an error if no matching compatible versions are found.
fn filter_versions(
    versions: Vec<Version>,
    version: Option<&String>,
    config: &RuntimeConfig,
    name: &str,
) -> Result<Vec<Version>> {
    let res: Vec<Version> = versions
        .into_iter()
        .filter(|x| is_version_supported(x, config))
        .filter(|x| version.as_ref().is_none_or(|v| &&x.version_number == v))
        .collect();

    if res.is_empty() {
        Err(anyhow::anyhow!("No matching versions found for '{name}'"))
    } else {
        Ok(res)
    }
}

/// Fetches available versions of a mod from Modrinth.
///
/// Retrieves version information for the specified mod name, optionally
/// filtering by version. Returns versions that are compatible with the
/// configured game version and loader.
///
/// # Arguments
///
/// * `name` - The mod name or slug on Modrinth (e.g., "fabric-api")
/// * `version` - Optional version number to filter for (e.g., Some("0.92.0"))
/// * `config` - Runtime configuration containing game version and loader
///
/// # Returns
///
/// A vector of compatible `Version` objects. If no specific version is requested,
/// returns all compatible versions ordered by release date (newest first).
///
/// # Errors
///
/// Returns an error if:
/// - Network request to Modrinth fails
/// - No compatible versions are found for the specified game version
/// - Response cannot be parsed
/// - The mod does not exist on Modrinth
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::fetch_version;
/// use launcher::config::RuntimeConfig;
/// use launcher::config::MCLoader;
/// use launcher::config::MCMirror;
///
/// # #[tokio::test]
/// # async fn test() -> anyhow::Result<()> {
/// let config = RuntimeConfig {
///     max_memory_size: 1000000,
///     window_weight: 100,
///     window_height: 100,
///     game_dir: "/path/to/game".to_string(),
///     game_version: "1.16.5".to_string(),
///     java_path: "/path/to/java".to_string(),
///     loader: MCLoader::None,
///     mirror: MCMirror::official_mirror(),
///     mods: None,
/// };
/// let versions = fetch_version("fabric-api", None, &config).await?;
/// println!("Found {} compatible versions", versions.len());
/// # Ok(())
/// # }
/// ```
pub async fn fetch_version(
    name: &str,
    version: Option<&String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch(name).await?;
    filter_versions(versions, version, config, name)
}

/// Fetches available versions of a mod from Modrinth (blocking).
///
/// Blocking version of `fetch_version` that runs synchronously instead of
/// using async/await. This is useful in contexts where async is not available
/// or when you want to block the current thread until the fetch completes.
///
/// # Arguments
///
/// * `name` - The mod name or slug on Modrinth (e.g., "fabric-api")
/// * `version` - Optional version number to filter for (e.g., Some("0.92.0"))
/// * `config` - Runtime configuration containing game version and loader
///
/// # Returns
///
/// A vector of compatible `Version` objects. If no specific version is requested,
/// returns all compatible versions ordered by release date (newest first).
///
/// # Errors
///
/// Returns an error if:
/// - Network request to Modrinth fails
/// - No compatible versions are found for the specified game version
/// - Response cannot be parsed
/// - The mod does not exist on Modrinth
///
/// # Note
///
/// This function blocks the current thread and should not be used in async contexts.
/// Use `fetch_version` for async code.
pub fn fetch_version_blocking(
    name: &str,
    version: Option<&String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch_blocking(name)?;
    filter_versions(versions, version, config, name)
}

/// Adds a mod to the configuration.
///
/// This function handles both local mod files and mods from Modrinth.
/// It updates the configuration with the mod information and optionally
/// installs the mod files to the game directory.
///
/// # Arguments
///
/// * `name` - For remote mods: the mod name/slug on Modrinth; for local mods: the file path
/// * `version` - Optional version number for remote mods (e.g., Some("0.92.0"))
/// * `local` - If `true`, treats `name` as a local file path; if `false`, fetches from Modrinth
/// * `config_only` - If `true`, only updates configuration without installing files
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Local mod file cannot be found (for local mods)
/// - Network request to Modrinth fails (for remote mods)
/// - No compatible versions are found
/// - Mod installation fails
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::add;
///
/// // Add a mod from Modrinth and install it
/// add("fabric-api", None, false, false)?;
///
/// // Add a specific version from Modrinth without installing
/// add("fabric-api", Some(&"0.92.0".to_string()), false, true)?;
///
/// // Add a local mod file
/// add("/path/to/mod.jar", None, true, false)?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn add(name: &str, version: Option<&String>, local: bool, config_only: bool) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    let message = if local {
        config_handler.add_mod_local(name)?;
        format!("Add local mod {name} successful")
    } else {
        config_handler.add_mod_unlocal_blocking(name, version)?;
        format!("Add mod {name} surcessful")
    };
    drop(config_handler);

    if !config_only {
        install()?;
    }

    println!("{message}");
    Ok(())
}

/// Removes a mod from the configuration.
///
/// This function completely removes a mod from the launcher:
/// - Removes the mod entry from config.toml
/// - Removes the mod entry from config.lock
/// - Removes the mod file from the game's mods directory
///
/// # Arguments
///
/// * `name` - The mod name to remove (must match the name used in `add()`)
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Mod is not found in the configuration
/// - Mod file cannot be removed from the filesystem
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::remove;
///
/// remove("fabric-api")?;
/// println!("Mod removed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn remove(name: &str) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    config_handler.remove_mod(name)?;

    println!("mod {} removed", &name);
    Ok(())
}

/// Updates all mods in the configuration.
///
/// This function updates all configured mods to their latest compatible
/// versions. It fetches the newest versions from Modrinth that support
/// the configured game version and loader, then updates config.lock.
///
/// # Arguments
///
/// * `config_only` - If `true`, only updates configuration without installing files
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Network requests to Modrinth fail for any mod
/// - No compatible versions are found for any mod
/// - Mod installation fails (when `config_only` is `false`)
///
/// # Behavior
///
/// - Each mod is updated to the latest compatible version
/// - Uses concurrent requests for better performance (10 concurrent)
/// - Shows progress bar during the update process
/// - Skips mods that are already at the latest compatible version
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::update;
///
/// // Update all mods and install them
/// update(false)?;
///
/// // Only update configuration, don't install files
/// update(true)?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn update(config_only: bool) -> Result<()> {
    sync_or_update(false)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

/// Creates download tasks for all configured mods.
///
/// This function generates installation tasks for mods listed in config.lock,
/// filtering to only include mods that are also present in config.toml.
/// This ensures only active mods are installed.
///
/// # Arguments
///
/// * `handle` - Configuration handler providing access to locked config
///
/// # Returns
///
/// A vector of `InstallTask` objects for downloading and verifying mod files.
///
/// # Errors
///
/// Returns an error if:
/// - No mods are found in config.lock
/// - A mod is missing required metadata (URL or SHA-1)
/// - The game directory path cannot be constructed
///
/// # Filtering Logic
///
/// Only creates tasks for mods that:
/// - Have both URL and SHA-1 checksum metadata
/// - Are present in both config.toml and config.lock
///
/// # Note
///
/// This function does not perform the actual download. Use `TaskPool::install()`
/// to execute the download tasks.
fn mod_installtasks(handle: &ConfigHandler) -> Result<VecDeque<InstallTask>> {
    let mods = handle
        .locked_config()
        .mods
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No mods found in config.lock"))?;

    mods.iter()
        .filter(|(_, v)| v.url.is_some() && v.sha1.is_some())
        .map(|(name, v)| {
            let save_file = Path::new(&handle.config().game_dir)
                .join("mods")
                .join(&v.file_name);
            Ok(InstallTask {
                url: v
                    .url
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("Missing URL for mod {name}"))?,
                sha1: Some(
                    v.sha1
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Missing SHA1 for mod {name}"))?,
                ),
                message: format!("Mod {} installed", &save_file.display()),
                save_file,
            })
        })
        .collect()
}

/// Installs all configured mods.
///
/// This function downloads and installs all mods listed in config.lock
/// that are compatible with the current game version. It first filters
/// the locked mods to only include mods that are also in config.toml,
/// then performs the downloads with SHA-1 verification.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read
/// - No mods are configured
/// - Mod download fails
/// - File system operations fail
/// - SHA-1 verification fails
///
/// # Behavior
///
/// - Only installs mods that appear in both config.toml and config.lock
/// - Downloads mods with concurrent requests for better performance
/// - Verifies SHA-1 checksums after download
/// - Creates the mods directory if it doesn't exist
/// - Skips installation if no mods are configured (returns Ok)
///
/// # Example
///
/// ```no_run
/// use launcher::modmanage::install;
///
/// install()?;
/// println!("All mods installed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn install() -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    if config_handler.locked_config().mods.is_none() || config_handler.config().mods.is_none() {
        return Ok(());
    }
    config_handler.locked_config_mut().mods = Some(
        config_handler
            .locked_config()
            .mods
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No mods found in config.lock"))?
            .into_iter()
            .filter(|(name, _)| {
                config_handler
                    .config()
                    .mods
                    .as_ref()
                    .is_some_and(|mods| mods.iter().any(|(x, _)| x == name))
            })
            .collect(),
    );
    let tasks = mod_installtasks(&config_handler)?;
    TaskPool::from(tasks).install();
    Ok(())
}

/// Syncs all mods to their configured versions.
///
/// This function ensures all mods are at the exact versions specified
/// in config.toml, rather than the latest available version. This is
/// useful for reproducible builds or when you need specific mod versions.
///
/// # Arguments
///
/// * `config_only` - If `true`, only updates configuration without installing files
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Network requests to Modrinth fail
/// - No compatible versions are found for any mod
/// - Mod installation fails (when `config_only` is `false`)
///
/// # Behavior
///
/// - Fetches the exact versions specified in config.toml
/// - Updates config.lock with the specified versions
/// - Uses concurrent requests for better performance (10 concurrent)
/// - Shows progress bar during the sync process
/// - Skips mods that are already at the correct version
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::sync;
///
/// // Sync all mods to configured versions and install them
/// sync(false)?;
///
/// // Only sync configuration, don't install files
/// sync(true)?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn sync(config_only: bool) -> Result<()> {
    sync_or_update(true)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

/// Creates a progress bar for tracking mod operations.
///
/// This helper function creates a progress bar with a consistent style
/// for use during sync/update operations.
///
/// # Arguments
///
/// * `len` - The total number of items to process
///
/// # Returns
///
/// A configured `ProgressBar` ready to use.
///
/// # Progress Bar Style
///
/// Shows elapsed time, visual progress bar, position/total, and current message.
/// Format: `[elapsed] [bar] pos/len message`
fn progress_bar(len: usize) -> Result<ProgressBar> {
    let bar = ProgressBar::new(len as u64);
    bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )?
        .progress_chars("##-"),
    );
    Ok(bar)
}

/// Handle for processing a single mod during sync/update operations.
///
/// This struct contains the context needed to fetch and update a specific
/// mod, including shared references to the configuration handler, progress bar,
/// and runtime configuration.
struct SyncUpdateHandle {
    name: String,
    conf: ModConfig,
    sync: bool,
    origin_config_share: Arc<RuntimeConfig>,
    handle_share: Arc<RwLock<ConfigHandler>>,
    bar_share: ProgressBar,
}

impl SyncUpdateHandle {
    /// Checks if the mod is already at the correct version in config.lock.
    ///
    /// This is used to avoid unnecessary re-downloads during sync operations.
    ///
    /// # Returns
    ///
    /// `true` if the mod is already synced to the correct version, `false` otherwise.
    fn is_mod_synced(&self) -> bool {
        let handle = self.handle_share.read().unwrap();
        let mc_version: &str = handle.config().game_version.as_ref();
        let locked_config_mods = handle.locked_config().mods.as_ref();

        if let Some(mods) = locked_config_mods {
            if mods.iter().any(|(mod_name, locked_conf)| {
                mod_name == &self.name
                    && self.conf.version == locked_conf.version
                    && mc_version == locked_conf.mc_version
            }) {
                return true;
            }
        }
        false
    }

    /// Fetches mod information and updates the configuration.
    ///
    /// This async function fetches the appropriate version of the mod
    /// (either the specified version for sync or latest for update) and
    /// adds it to the locked configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request to Modrinth fails
    /// - No compatible versions are found
    /// - Configuration cannot be updated
    async fn fetch_mod_to_config(&self) -> Result<()> {
        if let Some(ver) = self.conf.version.clone() {
            let version = {
                let ver = if self.sync { Some(ver) } else { None };
                fetch_version(&self.name, ver.as_ref(), &self.origin_config_share)
                    .await?
                    .remove(0)
            };
            self.handle_share
                .write()
                .unwrap()
                .add_mod_from(&self.name, version)
                .unwrap();
        }
        Ok(())
    }

    /// Updates the progress bar for this mod.
    ///
    /// Increments the progress counter and sets an appropriate message
    /// indicating whether the mod was synced or updated.
    fn update_bar(&self) {
        self.bar_share.inc(1);
        if self.sync {
            self.bar_share
                .set_message(format!("Mod {} synced", self.name));
        } else {
            self.bar_share
                .set_message(format!("Mod {} updated", self.name));
        }
    }

    /// Executes the sync/update operation for a single mod.
    ///
    /// This is the main entry point for processing a mod during bulk
    /// sync/update operations. It checks if the mod is already synced,
    /// fetches the appropriate version if needed, and updates the progress bar.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request to Modrinth fails
    /// - No compatible versions are found
    /// - Configuration cannot be updated
    ///
    /// # Behavior
    ///
    /// - Skips mods that are already at the correct version (sync mode only)
    /// - Fetches specified version for sync mode, latest for update mode
    /// - Handles local mod files if specified in configuration
    async fn execute(self) -> Result<()> {
        if self.sync && self.is_mod_synced() {
            self.update_bar();
            return Ok(());
        }

        self.fetch_mod_to_config().await?;
        self.update_bar();

        if let Some(file_name) = self.conf.file_name {
            self.handle_share
                .write()
                .unwrap()
                .add_mod_local(&file_name)
                .unwrap();
        }
        Ok(())
    }
}

/// Async entry point for sync/update operations.
///
/// This function coordinates the bulk sync or update of all configured mods.
/// It creates concurrent tasks for each mod and processes them with a progress bar.
///
/// # Arguments
///
/// * `sync` - If `true`, sync to exact versions from config.toml; if `false`, update to latest versions
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read
/// - Network requests to Modrinth fail for any mod
/// - No compatible versions are found for any mod
///
/// # Behavior
///
/// - Processes up to 10 mods concurrently
/// - Shows a progress bar with status messages
/// - Skips mods that are already at the correct version (sync mode only)
/// - Uses shared configuration handler for thread-safe updates
///
/// # Note
///
/// This function uses `#[tokio::main(flavor = "current_thread")]` to run
/// async code in a synchronous context.
#[tokio::main(flavor = "current_thread")]
async fn sync_or_update(sync: bool) -> Result<()> {
    let config_handler = ConfigHandler::read()?;
    if let Some(mods) = config_handler.config().mods.clone() {
        let origin_config = Arc::new(config_handler.config().to_owned());
        let config_handler = Arc::new(RwLock::new(config_handler));
        let bar = progress_bar(mods.len())?;
        let tasks = mods.into_iter().map(|(name, conf)| {
            let sync_update_handle = SyncUpdateHandle {
                name,
                conf,
                sync,
                origin_config_share: origin_config.clone(),
                handle_share: config_handler.clone(),
                bar_share: bar.clone(),
            };
            sync_update_handle.execute()
        });
        stream::iter(tasks)
            .buffer_unordered(10)
            .try_collect::<Vec<_>>()
            .await?;
    }

    if sync {
        println!("mod config synced");
    } else {
        println!("mod config updated");
    }
    Ok(())
}

/// Removes unused mods from config.lock.
///
/// This function identifies mods that are present in config.lock but not
/// in config.toml (i.e., mods that have been removed from the user's
/// configuration) and removes them from config.lock.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read
/// - Configuration cannot be written
/// - A mod cannot be removed from config.lock
///
/// # Purpose
///
/// This cleanup operation ensures that config.lock only contains mods
/// that are actually configured by the user in config.toml, preventing
/// stale mod entries from accumulating.
fn clean_locked_config_mods() -> Result<()> {
    let origin_handle = ConfigHandler::read()?;
    let mut handle = origin_handle.clone();
    let mods = origin_handle.config().mods.as_ref();
    let unuse_locked_mods = origin_handle
        .locked_config()
        .mods
        .as_ref()
        .map(|locked_mods| {
            locked_mods.iter().filter(|(locked_mod_name, _)| {
                if let Some(mods) = mods {
                    let has_in_config = mods.iter().any(|(name, _)| &name == locked_mod_name);
                    !has_in_config
                } else {
                    true
                }
            })
        });

    if let Some(x) = unuse_locked_mods {
        x.clone()
            .try_for_each(|(name, _)| handle.locked_config_mut().remove_mod(name))?;
    }
    Ok(())
}

/// Removes unused mod files from the mods directory.
///
/// This function scans the mods directory for files with the `.unuse`
/// extension (which are marked as unused) and deletes them.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read
/// - The mods directory cannot be accessed
/// - A file cannot be removed
///
/// # File Naming Convention
///
/// Unused mod files are expected to have the `.unuse` extension.
/// For example: `fabric-api.jar.unuse`
///
/// # Note
///
/// This function performs case-sensitive file extension comparisons,
/// which is necessary because some filesystems are case-sensitive.
#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "case sensitive is needed for cross-platform compatibility"
)]
fn clean_file_mods() -> Result<()> {
    let handle = ConfigHandler::read()?;
    let mods_dir = Path::new(&handle.config().game_dir).join("mods");
    WalkDir::new(mods_dir)
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .unwrap()
                .to_string()
                .ends_with(".unuse")
                || entry.path().is_dir()
        })
        .try_for_each(|entry| {
            let file_path = entry?.path().to_owned();
            if file_path.is_dir() {
                Ok(())
            } else {
                fs::remove_file(file_path)
            }
        })?;
    Ok(())
}

/// Cleans up unused mod files and configuration entries.
///
/// This function performs two cleanup operations:
/// 1. Removes mods from config.lock that are not in config.toml
/// 2. Deletes unused mod files (files with `.unuse` extension) from the mods directory
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read or written
/// - File system operations fail
/// - Files cannot be removed
///
/// # Purpose
///
/// Regular cleanup prevents accumulation of:
/// - Stale mod entries in config.lock
/// - Old mod files that are no longer needed
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::clean;
///
/// clean()?;
/// println!("Cleanup completed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn clean() -> Result<()> {
    clean_locked_config_mods()?;
    clean_file_mods()?;
    println!("mods cleaned");
    Ok(())
}

/// Structure for displaying mod search results in a table.
#[derive(Debug, Tabled)]
struct HitsInfo {
    slug: String,
    description: String,
}

/// Searches for mods on Modrinth.
///
/// This function searches for mods by name and filters the results to
/// only show mods that are compatible with the configured game version
/// and loader. Results are displayed in a formatted table.
///
/// # Arguments
///
/// * `name` - The search query (mod name or keyword)
/// * `limit` - Optional maximum number of results to display (default varies)
///
/// # Errors
///
/// Returns an error if:
/// - Configuration cannot be read
/// - Network request to Modrinth fails
/// - No loader is configured in config.toml
/// - Response cannot be parsed
///
/// # Behavior
///
/// - Fetches search results from Modrinth
/// - Filters results to only show compatible mods
/// - Fetches version info for each result to verify compatibility
/// - Displays results in a formatted table
/// - Shows message if too many results are found (suggests using --limit)
///
/// # Output Format
///
/// Results are displayed as a table with:
/// - Column 1: Mod slug (unique identifier)
/// - Column 2: Mod description
///
/// # Examples
///
/// ```no_run
/// use launcher::modmanage::search;
///
/// // Search for mods (default limit)
/// search("inventory", None)?;
///
/// // Search with a specific limit
/// search("inventory", Some(5))?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn search(name: &str, limit: Option<usize>) -> Result<()> {
    let handle = ConfigHandler::read()?;

    let loader = match handle.config().loader {
        MCLoader::Fabric(_) => "fabric",
        MCLoader::None => return Err(anyhow::anyhow!("config.toml not have loader")),
    };

    let game_version = handle.config().game_version.as_ref();

    let projects = Projects::fetch_blocking(name, limit)?;

    let res: Result<Vec<_>> = projects
        .hits
        .iter()
        .filter_map(|hit| {
            let project_slug = hit.slug.as_ref();
            let project_version = Versions::fetch_blocking(project_slug);
            match project_version {
                Ok(versions) => {
                    let is_support_mod = hit.is_mod()
                        && versions.into_iter().any(|v| {
                            v.is_support_loader(loader) && v.is_support_game_version(game_version)
                        });
                    if is_support_mod {
                        Some(Ok(HitsInfo {
                            slug: hit.slug.clone(),
                            description: hit.description.clone(),
                        }))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            }
        })
        .collect();
    let res = res?;

    match res.len() {
        0 => println!("No match mods found!"),
        1..=10 => {
            let mut table = Table::new(res);
            println!("{}", table.with(Style::modern()));
        }
        _ => {
            let mut table = Table::new(res);
            println!("{}", table.with(Style::modern()));
            println!("use --limit N to see more");
        }
    }
    Ok(())
}
