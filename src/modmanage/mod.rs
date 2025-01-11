use crate::config::{ConfigHandler, MCLoader, ModConfig, RuntimeConfig};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use installer::{InstallTask, TaskPool};
use modrinth_api::{Version, Versions};
use std::{
    collections::VecDeque,
    fs,
    path::Path,
    sync::{Arc, RwLock},
};
use walkdir::WalkDir;

pub async fn fetch_version(
    name: &str,
    version: &Option<String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch(name).await?;
    let res: Vec<Version> = versions
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
        .collect();
    if res.is_empty() {
        return Err(anyhow::anyhow!("Can't fetch {}", name));
    };
    Ok(res)
}

pub fn fetch_version_blocking(
    name: &str,
    version: &Option<String>,
    config: &RuntimeConfig,
) -> Result<Vec<Version>> {
    let versions = Versions::fetch_blocking(name)?;
    let res: Vec<Version> = versions
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
        .collect();
    if res.is_empty() {
        return Err(anyhow::anyhow!("Can't fetch {}", name));
    }
    Ok(res)
}

pub fn add(name: &str, version: Option<String>, local: bool, config_only: bool) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    let message = if local {
        config_handler.add_mod_local(name)?;
        format!("Add local mod {} successful", name)
    } else {
        config_handler.add_mod_unlocal_blocking(name, &version)?;
        format!("Add mod {} surcessful", name)
    };
    drop(config_handler);

    if !config_only {
        install()?;
    }

    println!("{}", message);
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    config_handler.remove_mod(name)?;

    println!("mod {} removed", &name);
    Ok(())
}

pub fn update(config_only: bool) -> Result<()> {
    sync_or_update(false)?;
    if !config_only {
        install()?;
    }
    Ok(())
}

fn mod_installtasks(handle: &ConfigHandler) -> VecDeque<InstallTask> {
    handle
        .locked_config()
        .mods
        .as_ref()
        .unwrap()
        .iter()
        .filter(|(_, v)| v.url.is_some() && v.sha1.is_some())
        .map(|(_, v)| {
            let save_file = Path::new(&handle.config().game_dir)
                .join("mods")
                .join(&v.file_name);
            InstallTask {
                url: v.url.to_owned().unwrap(),
                sha1: Some(v.sha1.to_owned().unwrap()),
                message: format!("mod {:?} installed", &save_file),
                save_file,
            }
        })
        .collect()
}

pub fn install() -> Result<()> {
    // Panic while mods is none which in config.lock and config.toml
    let mut config_handler = ConfigHandler::read()?;
    if config_handler.locked_config().mods.is_none() || config_handler.config().mods.is_none() {
        return Ok(());
    }
    config_handler.locked_config_mut().mods = Some(
        config_handler
            .locked_config()
            .mods
            .to_owned()
            .unwrap()
            .into_iter()
            .filter(|(name, _)| {
                config_handler
                    .config()
                    .mods
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|(x, _)| x == name)
            })
            .collect(),
    );
    let tasks = mod_installtasks(&config_handler);
    TaskPool::from(tasks).install();
    Ok(())
}

pub fn sync(config_only: bool) -> Result<()> {
    sync_or_update(true)?;
    if !config_only {
        install()?;
    }
    Ok(())
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

async fn sync_or_update_handle(
    (name, conf, sync): (String, ModConfig, bool),
    origin_config_share: Arc<RuntimeConfig>,
    handle_share: Arc<RwLock<ConfigHandler>>,
    bar_share: ProgressBar,
) {
    if let Some(mods) = handle_share.read().unwrap().locked_config().mods.as_ref() {
        if sync
            && mods.iter().any(|(mod_name, locked_conf)| {
                mod_name == &name && conf.version == locked_conf.version
            })
        {
            bar_share.inc(1);
            bar_share.set_message(format!("Mod {} synced", name));
            return;
        }
    }

    if let Some(ver) = conf.version {
        let version = {
            let ver = if sync { Some(ver) } else { None };
            fetch_version(&name, &ver, &origin_config_share)
                .await
                .unwrap()
                .remove(0)
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

    if let Some(file_name) = conf.file_name {
        handle_share
            .write()
            .unwrap()
            .add_mod_local(&file_name)
            .unwrap();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn sync_or_update(sync: bool) -> Result<()> {
    let config_handler = ConfigHandler::read()?;
    if let Some(mods) = config_handler.config().mods.to_owned() {
        let origin_config = Arc::new(config_handler.config().to_owned());
        let config_handler = Arc::new(RwLock::new(config_handler));
        let bar = progress_bar(mods.len());
        let tasks = mods.into_iter().map(|(name, conf)| {
            sync_or_update_handle(
                (name, conf, sync),
                origin_config.clone(),
                config_handler.clone(),
                bar.clone(),
            )
        });
        stream::iter(tasks)
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;
    }

    if sync {
        println!("mod config synced");
    } else {
        println!("mod config updated");
    }
    Ok(())
}

pub fn clean() -> Result<()> {
    let origin = ConfigHandler::read()?;
    let mut handle = origin.clone();
    if let Some(x) = origin.locked_config().mods.as_ref().map(|x| {
        x.iter().filter(|(locked_mod_name, _)| {
            let mods = origin.config().mods.as_ref();
            !(mods.is_some()
                && mods
                    .map(|x| x.iter().any(|(name, _)| &name == locked_mod_name))
                    .unwrap())
        })
    }) {
        x.to_owned()
            .for_each(|(name, _)| handle.locked_config_mut().remove_mod(name).unwrap());
    }
    let mods_dir = Path::new(&handle.config().game_dir).join("mods");
    for entry in WalkDir::new(mods_dir).into_iter().filter(|x| {
        x.as_ref()
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .ends_with(".unuse")
    }) {
        let file_path = entry?.path().to_owned();
        fs::remove_file(file_path)?;
    }
    println!("mods cleaned");
    Ok(())
}
