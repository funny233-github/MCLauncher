use crate::asyncuntil::AsyncIterator;
use crate::config::{ConfigHandler, LockedModConfig, MCLoader, RuntimeConfig};
use crate::install::{InstallTask, InstallType, TaskPool};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use modrinth_api::{Version, Versions};
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Arc, RwLock},
};

pub async fn fetch_version(
    name: &str,
    version: &Option<String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch(name).await?;
    Ok(versions
        .into_iter()
        .filter(|x| {
            x.game_versions.iter().any(|x| x == &config.game_version)
                && x.loaders.iter().any(|x| match config.loader {
                    MCLoader::None => false,
                    MCLoader::Fabric(_) => x == "fabric",
                })
        })
        .filter(|x| {
            if let Some(v) = &version {
                &x.version_number == v
            } else {
                true
            }
        })
        .collect())
}

pub fn fetch_version_blocking(
    name: &str,
    version: &Option<String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch_blocking(name)?;
    Ok(versions
        .into_iter()
        .filter(|x| {
            x.game_versions.iter().any(|x| x == &config.game_version)
                && x.loaders.iter().any(|x| match config.loader {
                    MCLoader::None => false,
                    MCLoader::Fabric(_) => x == "fabric",
                })
        })
        .filter(|x| {
            if let Some(v) = &version {
                &x.version_number == v
            } else {
                true
            }
        })
        .collect())
}

pub fn add(name: &str, version: Option<String>, local: bool) -> Result<()> {
    let mut config_handler = ConfigHandler::new()?;
    let message = if local {
        config_handler.add_mod_local(name)?;
        format!("Add local mod {} successful", name)
    } else {
        config_handler.add_mod_unlocal_blocking(name, &version)?;
        format!("Add mod {} surcessful", name)
    };
    config_handler.write()?;
    println!("{}", message);
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let mut config_handler = ConfigHandler::new()?;
    config_handler.remove_mod(name)?;
    config_handler.write()?;

    println!("mod {} removed", &name);
    Ok(())
}

pub fn update() -> Result<()> {
    sync_or_update(false)
}

fn mod_installtasks(config: &HashMap<String, LockedModConfig>) -> VecDeque<InstallTask> {
    config
        .iter()
        .map(|(_, v)| {
            let save_file = Path::new("mods").join(&v.file_name);
            InstallTask {
                url: v.url.to_owned().unwrap(),
                sha1: Some(v.sha1.to_owned().unwrap()),
                save_file,
                r#type: InstallType::Mods,
            }
        })
        .collect()
}

// TODO: update file when install
pub fn install() -> Result<()> {
    let mut config_handler = ConfigHandler::new()?;
    config_handler.locked_config.mods = Some(
        config_handler
            .locked_config
            .mods
            .unwrap()
            .into_iter()
            .filter(|(name, _)| {
                config_handler
                    .config
                    .mods
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|(x, _)| x == name)
            })
            .collect(),
    );
    let tasks = mod_installtasks(&config_handler.locked_config.mods.unwrap());
    TaskPool::from(tasks).install()?;
    Ok(())
}

pub fn sync() -> Result<()> {
    sync_or_update(true)
}

fn progress_bar(len: usize) -> ProgressBar {
    let bar = ProgressBar::new(len as u64);
    bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );
    bar
}

fn sync_or_update(sync: bool) -> Result<()> {
    let config_handler = ConfigHandler::new()?;
    if let Some(mods) = config_handler.config.mods.clone() {
        let origin_config = Arc::new(ConfigHandler::new()?);
        let config_handler = Arc::new(RwLock::new(config_handler));
        let bar = progress_bar(mods.len());
        mods.into_iter()
            .map(|x| {
                let origin_config_share = origin_config.clone();
                let handle_share = config_handler.clone();
                let bar_share = bar.clone();
                async move {
                    let (name, conf) = x;
                    if let Some(ver) = conf.version {
                        let version = {
                            let config = &origin_config_share.config;
                            let ver = if sync { Some(ver) } else { None };
                            fetch_version(&name, &ver, config).await.unwrap().remove(0)
                        };
                        handle_share
                            .write()
                            .unwrap()
                            .add_mod_from(&name, version)
                            .unwrap();
                        bar_share.inc(1);
                        if sync {
                            bar_share.set_message(format!("Mod {} synced", name));
                        } else {
                            bar_share.set_message(format!("Mod {} updated", name));
                        }
                    }
                }
            })
            .async_execute(10);
        println!("{:#?}", config_handler);
        config_handler.write().unwrap().write()?;
    }
    if sync {
        println!("mod config synced");
    } else {
        println!("mod config updated");
    }
    Ok(())
}
