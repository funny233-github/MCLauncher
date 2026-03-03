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
/// configured game version.
///
/// # Errors
/// Returns an error if:
/// - Network request to Modrinth fails
/// - No compatible versions are found
/// - Response cannot be parsed
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
/// using async/await.
///
/// # Errors
/// Returns an error if:
/// - Network request to Modrinth fails
/// - No compatible versions are found
/// - Response cannot be parsed
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
/// Either adds a local mod file or downloads a mod from Modrinth.
/// Optionally installs the mod files to the game directory.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Mod file cannot be found (for local mods)
/// - Network request to Modrinth fails (for remote mods)
/// - No compatible versions are found
/// - Mod installation fails
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
/// Deletes the mod entry from both config.toml and config.lock,
/// and removes the mod file from the game directory.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Mod is not found in configuration
/// - Mod file cannot be removed
pub fn remove(name: &str) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    config_handler.remove_mod(name)?;

    println!("mod {} removed", &name);
    Ok(())
}

/// Updates all mods in the configuration.
///
/// Fetches the latest versions of all configured mods and updates
/// the configuration. Optionally installs the updated mod files.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Network requests to Modrinth fail
/// - No compatible versions are found for any mod
/// - Mod installation fails
pub fn update(config_only: bool) -> Result<()> {
    sync_or_update(false)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

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
/// Downloads and installs all mods listed in the configuration
/// that are compatible with the current game version.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read
/// - No mods are configured
/// - Mod download fails
/// - File system operations fail
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
/// Ensures all mods in the configuration are at the exact versions
/// specified in config.toml. Optionally installs the synced mod files.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read or written
/// - Network requests to Modrinth fail
/// - No compatible versions are found for any mod
/// - Mod installation fails
pub fn sync(config_only: bool) -> Result<()> {
    sync_or_update(true)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

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

struct SyncUpdateHandle {
    name: String,
    conf: ModConfig,
    sync: bool,
    origin_config_share: Arc<RuntimeConfig>,
    handle_share: Arc<RwLock<ConfigHandler>>,
    bar_share: ProgressBar,
}

impl SyncUpdateHandle {
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

#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "case sensitive is need"
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

/// Cleans up unused mod files.
///
/// Removes mod entries from config.lock that are not in config.toml,
/// and deletes any mod files in the mods directory that are marked
/// as unused (have a .unuse extension).
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read or written
/// - File system operations fail
/// - Files cannot be removed
pub fn clean() -> Result<()> {
    clean_locked_config_mods()?;
    clean_file_mods()?;
    println!("mods cleaned");
    Ok(())
}

#[derive(Debug, Tabled)]
struct HitsInfo {
    slug: String,
    description: String,
}

/// Searches for mods on Modrinth.
///
/// Searches for mods by name and filters results to only show
/// mods that are compatible with the configured game version and loader.
///
/// # Errors
/// Returns an error if:
/// - Configuration cannot be read
/// - Network request to Modrinth fails
/// - No loader is configured
/// - Response cannot be parsed
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
