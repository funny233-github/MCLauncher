//! Minecraft mod management module.
//!
//! Provides comprehensive functionality for managing Minecraft mods, including fetching,
//! installing, updating, syncing, and searching mods from Modrinth. Supports both local
//! mod files and remote mod downloads. Uses two configuration files: config.toml for
//! user-defined mod list with version preferences, and config.lock for locked versions
//! with download URLs and checksums.

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
/// Verifies that a mod version supports both the configured game version and
/// the configured mod loader. Returns true if compatible, false otherwise.
fn is_version_supported(version: &Version, config: &RuntimeConfig) -> bool {
    version.game_versions.iter().any(|x| x == &config.vanilla)
        && version.loaders.iter().any(|x| match config.loader {
            MCLoader::None => false,
            MCLoader::Fabric(_) => x == "fabric",
            MCLoader::Neoforge(_) => x == "neoforge",
        })
}

/// Filters mod versions based on version number and compatibility.
///
/// Takes a list of mod versions and filters them to only include versions that
/// are compatible with the current game version and loader, optionally also filtering
/// by a specific version number. Returns a filtered list of compatible versions.
///
/// # Errors
/// - `anyhow::Error` if no matching compatible versions are found.
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
/// Retrieves version information for the specified mod name, optionally filtering
/// by version. Returns versions that are compatible with the configured game version
/// and loader. Returns all compatible versions ordered by release date (newest first).
///
/// # Example
/// ```no_run
/// use gluon::modmanage::fetch_version;
/// use gluon::config::RuntimeConfig;
/// use gluon::config::MCLoader;
/// use gluon::config::MCMirror;
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
///
/// # Errors
/// - `anyhow::Error` if network request to Modrinth fails
/// - `anyhow::Error` if no compatible versions are found for the specified game version
/// - `anyhow::Error` if response cannot be parsed
/// - `anyhow::Error` if the mod does not exist on Modrinth
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
/// Blocking version of `fetch_version` that runs synchronously instead of using
/// async/await. Returns all compatible versions ordered by release date (newest first).
///
/// # Errors
/// - `anyhow::Error` if network request to Modrinth fails
/// - `anyhow::Error` if no compatible versions are found for the specified game version
/// - `anyhow::Error` if response cannot be parsed
/// - `anyhow::Error` if the mod does not exist on Modrinth
///
/// # Note
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
/// Handles both local mod files and mods from Modrinth. Updates the configuration
/// with the mod information and optionally installs the mod files to the game directory.
///
/// # Example
/// ```no_run
/// use gluon::modmanage::add;
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
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
/// - `anyhow::Error` if local mod file cannot be found (for local mods)
/// - `anyhow::Error` if network request to Modrinth fails (for remote mods)
/// - `anyhow::Error` if no compatible versions are found
/// - `anyhow::Error` if mod installation fails
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
/// Completely removes a mod from the launcher, including removing the mod entry
/// from config.toml and config.lock, and removing the mod file from the game's mods directory.
///
/// # Example
/// ```no_run
/// use gluon::modmanage::remove;
///
/// remove("fabric-api")?;
/// println!("Mod removed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
/// - `anyhow::Error` if mod is not found in the configuration
/// - `anyhow::Error` if mod file cannot be removed from the filesystem
pub fn remove(name: &str) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    config_handler.remove_mod(name)?;

    println!("mod {} removed", &name);
    Ok(())
}

/// Updates all mods in the configuration.
///
/// Updates all configured mods to their latest compatible versions by fetching the
/// newest versions from Modrinth that support the configured game version and loader,
/// then updates config.lock. Uses concurrent requests for better performance (10 concurrent),
/// shows progress bar during the update process, and skips mods that are already at
/// the latest compatible version.
///
/// # Example
/// ```no_run
/// use gluon::modmanage::update;
///
/// // Update all mods and install them
/// update(false)?;
///
/// // Only update configuration, don't install files
/// update(true)?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
/// - `anyhow::Error` if network requests to Modrinth fail for any mod
/// - `anyhow::Error` if no compatible versions are found for any mod
/// - `anyhow::Error` if mod installation fails (when `config_only` is `false`)
pub fn update(config_only: bool) -> Result<()> {
    sync_or_update(false)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

/// Creates download tasks for all configured mods.
///
/// Generates installation tasks for mods listed in config.lock, filtering to only
/// include mods that are also present in config.toml to ensure only active mods
/// are installed. Returns a vector of `InstallTask` objects for downloading and
/// verifying mod files. Does not perform the actual download; use `TaskPool::install()`
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
/// Downloads and installs all mods listed in config.lock that are compatible
/// with the current game version by filtering the locked mods to only include mods
/// that are also in config.toml, then performs the downloads with SHA-1 verification.
/// Only installs mods that appear in both config.toml and config.lock, downloads
/// mods with concurrent requests for better performance, verifies SHA-1 checksums
/// after download, creates the mods directory if it doesn't exist, and skips
/// installation if no mods are configured (returns Ok).
///
/// # Example
/// ```no_run
/// use gluon::modmanage::install;
///
/// install()?;
/// println!("All mods installed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read
/// - `anyhow::Error` if no mods are configured
/// - `anyhow::Error` if mod download fails
/// - `anyhow::Error` if file system operations fail
/// - `anyhow::Error` if SHA-1 verification fails
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
/// Ensures all mods are at the exact versions specified in config.toml rather than
/// the latest available version, useful for reproducible builds or when you need
/// specific mod versions. Fetches the exact versions specified in config.toml,
/// updates config.lock with the specified versions, uses concurrent requests for
/// better performance (10 concurrent), shows progress bar during the sync process,
/// and skips mods that are already at the correct version.
///
/// # Example
/// ```no_run
/// use gluon::modmanage::sync;
///
/// // Sync all mods to configured versions and install them
/// sync(false)?;
///
/// // Only sync configuration, don't install files
/// sync(true)?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
/// - `anyhow::Error` if network requests to Modrinth fail
/// - `anyhow::Error` if no compatible versions are found for any mod
/// - `anyhow::Error` if mod installation fails (when `config_only` is `false`)
pub fn sync(config_only: bool) -> Result<()> {
    sync_or_update(true)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

/// Creates a progress bar for tracking mod operations.
///
/// Creates a progress bar with a consistent style for use during sync/update operations.
/// Shows elapsed time, visual progress bar, position/total, and current message
/// with format: `[elapsed] [bar] pos/len message`.
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
/// Contains the context needed to fetch and update a specific mod, including shared
/// references to the configuration handler, progress bar, and runtime configuration.
struct SyncUpdateHandle {
    /// Name of the mod being processed.
    name: String,
    /// Configuration for the mod including version preferences.
    conf: ModConfig,
    /// Whether this is a sync operation (true) or update operation (false).
    sync: bool,
    /// Shared reference to the runtime configuration.
    origin_config_share: Arc<RuntimeConfig>,
    /// Shared reference to the configuration handler with read/write lock.
    handle_share: Arc<RwLock<ConfigHandler>>,
    /// Shared progress bar for tracking operation progress.
    bar_share: ProgressBar,
}

impl SyncUpdateHandle {
    /// Renames a disabled mod file back to its original name.
    ///
    /// Removes the `.unuse` suffix from the mod file if it exists, enabling the mod
    /// for use in the game. The function checks both locked and regular configurations
    /// to determine the correct original filename.
    ///
    /// # Errors
    /// - `anyhow::Error` if filesystem rename operation fails
    fn rename_unuse_mod(&self) -> Result<()> {
        let handle = &self.handle_share.read().unwrap();
        let locked_config_mods = handle.locked_config().mods.as_ref();
        let config_mods = handle.config().mods.as_ref();
        let file_name;
        if let Some(locked_config_mods) = locked_config_mods {
            file_name = locked_config_mods
                .get(&self.name)
                .unwrap()
                .file_name
                .clone();
        } else if let Some(config_mods) = config_mods {
            if let Some(name) = config_mods
                .get(&self.name)
                .cloned()
                .and_then(|x| x.file_name)
            {
                file_name = name;
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }
        let unuse_file_name = format!("{file_name}.unuse");
        let game_dir = &handle.config().game_dir;
        let file_path = Path::new(game_dir).join("mods").join(&unuse_file_name);
        let target_file_path = Path::new(&game_dir).join("mods").join(file_name);
        if fs::exists(&file_path).is_ok_and(|x| x) {
            fs::rename(&file_path, target_file_path)?;
        }
        Ok(())
    }

    /// Checks if the mod is already at the correct version in config.lock.
    ///
    /// Used to avoid unnecessary re-downloads during sync operations.
    /// Returns true if already synced to the correct version, false otherwise.
    fn is_mod_synced(&self) -> bool {
        let handle = self.handle_share.read().unwrap();
        let mc_version: &str = handle.config().vanilla.as_ref();
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
    /// Async function that fetches the appropriate version of the mod (either the
    /// specified version for sync or latest for update) and adds it to the locked
    /// configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if network request to Modrinth fails
    /// - `anyhow::Error` if no compatible versions are found
    /// - `anyhow::Error` if configuration cannot be updated
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
    /// Increments the progress counter and sets an appropriate message indicating
    /// whether the mod was synced or updated.
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
    /// Main entry point for processing a mod during bulk sync/update operations.
    /// Checks if the mod is already synced, fetches the appropriate version if needed,
    /// and updates the progress bar. Skips mods that are already at the correct version
    /// (sync mode only), fetches specified version for sync mode, latest for update mode,
    /// and handles local mod files if specified in configuration.
    ///
    /// # Errors
    /// - `anyhow::Error` if network request to Modrinth fails
    /// - `anyhow::Error` if no compatible versions are found
    /// - `anyhow::Error` if configuration cannot be updated
    async fn execute(self) -> Result<()> {
        self.rename_unuse_mod()?;
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
/// Coordinates the bulk sync or update of all configured mods by creating concurrent
/// tasks for each mod and processing them with a progress bar. Processes up to 10
/// mods concurrently, shows a progress bar with status messages, skips mods that are
/// already at the correct version (sync mode only), and uses shared configuration
/// handler for thread-safe updates. Uses `#[tokio::main(flavor = "current_thread")]`
/// to run async code in a synchronous context.
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read
/// - `anyhow::Error` if network requests to Modrinth fail for any mod
/// - `anyhow::Error` if no compatible versions are found for any mod
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
/// Identifies mods that are present in config.lock but not in config.toml (i.e.,
/// mods that have been removed from the user's configuration) and removes them from
/// config.lock. Ensures that config.lock only contains mods that are actually
/// configured by the user in config.toml, preventing stale mod entries from
/// accumulating.
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
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
            .for_each(|(name, _)| handle.locked_config_mut().remove_mod(name));
    }
    Ok(())
}

/// Removes unused mod files from the mods directory.
///
/// Scans the mods directory for files with `.unuse` extension (which are marked
/// as unused) and deletes them. Performs case-sensitive file extension comparisons
/// for cross-platform compatibility.
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read
/// - `anyhow::Error` if file system operations fail
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
/// Performs two cleanup operations: removes mods from config.lock that are not
/// in config.toml, and deletes unused mod files (files with `.unuse` extension)
/// from the mods directory. Prevents accumulation of stale mod entries in config.lock
/// and old mod files that are no longer needed.
///
/// # Example
/// ```no_run
/// use gluon::modmanage::clean;
///
/// clean()?;
/// println!("Cleanup completed successfully");
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read or written
/// - `anyhow::Error` if file system operations fail
/// - `anyhow::Error` if files cannot be removed
pub fn clean() -> Result<()> {
    clean_locked_config_mods()?;
    clean_file_mods()?;
    println!("mods cleaned");
    Ok(())
}

/// Structure for displaying mod search results in a table.
/// Information about a mod search result for display in tables.
#[derive(Debug, Tabled)]
struct HitsInfo {
    /// Unique identifier (slug) of the mod on Modrinth.
    slug: String,
    /// Short description of the mod.
    description: String,
}

/// Searches for mods on Modrinth.
///
/// Searches for mods by name and filters results to only show mods that are compatible
/// with the configured game version and loader. Displays results in a formatted table
/// showing mod slug (unique identifier) and mod description. Fetches search results from
/// Modrinth, filters results to only show compatible mods, fetches version info for
/// each result to verify compatibility, and shows message if too many results are found
/// (suggests using --limit).
///
/// # Example
/// ```no_run
/// use gluon::modmanage::search;
///
/// // Search for mods (default limit)
/// search("inventory", None)?;
///
/// // Search with a specific limit
/// search("inventory", Some(5))?;
///
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// - `anyhow::Error` if configuration cannot be read
/// - `anyhow::Error` if network request to Modrinth fails
/// - `anyhow::Error` if no loader is configured in config.toml
/// - `anyhow::Error` if response cannot be parsed
pub fn search(name: &str, limit: Option<usize>) -> Result<()> {
    let handle = ConfigHandler::read()?;

    let loader = match handle.config().loader {
        MCLoader::Neoforge(_) => "neforge",
        MCLoader::Fabric(_) => "fabric",
        MCLoader::None => return Err(anyhow::anyhow!("config.toml not have loader")),
    };

    let game_version = handle.config().vanilla.as_ref();

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
