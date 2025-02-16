use crate::config::{ConfigHandler, MCLoader, ModConfig, RuntimeConfig};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use installer::{InstallTask, TaskPool};
use modrinth_api::{Projects, Version, Versions};
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

fn mod_installtasks(handle: &ConfigHandler) -> Result<VecDeque<InstallTask>> {
    handle
        .locked_config()
        .mods
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("there are no mod in config.lock"))?
        .iter()
        .filter(|(_, v)| v.url.is_some() && v.sha1.is_some())
        .map(|(_, v)| {
            let save_file = Path::new(&handle.config().game_dir)
                .join("mods")
                .join(&v.file_name);
            Ok(InstallTask {
                url: v
                    .url
                    .to_owned()
                    .ok_or_else(|| anyhow::anyhow!("url is none"))?,
                sha1: Some(
                    v.sha1
                        .to_owned()
                        .ok_or_else(|| anyhow::anyhow!("sha1 is none"))?,
                ),
                message: format!("mod {:?} installed", &save_file),
                save_file,
            })
        })
        .collect()
}

pub fn install() -> Result<()> {
    let mut config_handler = ConfigHandler::read()?;
    if config_handler.locked_config().mods.is_none() || config_handler.config().mods.is_none() {
        return Ok(());
    }
    config_handler.locked_config_mut().mods = Some(
        config_handler
            .locked_config()
            .mods
            .to_owned()
            .ok_or_else(|| anyhow::anyhow!("there are no mod in config.lock"))?
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

async fn sync_or_update_handle(
    (name, conf, sync): (String, ModConfig, bool),
    origin_config_share: Arc<RuntimeConfig>,
    handle_share: Arc<RwLock<ConfigHandler>>,
    bar_share: ProgressBar,
) -> Result<()> {
    if let Some(mods) = handle_share.read().unwrap().locked_config().mods.as_ref() {
        if sync
            && mods.iter().any(|(mod_name, locked_conf)| {
                mod_name == &name && conf.version == locked_conf.version
            })
        {
            bar_share.inc(1);
            bar_share.set_message(format!("Mod {} synced", name));
            return Ok(());
        }
    }

    if let Some(ver) = conf.version {
        let version = {
            let ver = if sync { Some(ver) } else { None };
            fetch_version(&name, &ver, &origin_config_share)
                .await?
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
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn sync_or_update(sync: bool) -> Result<()> {
    let config_handler = ConfigHandler::read()?;
    if let Some(mods) = config_handler.config().mods.to_owned() {
        let origin_config = Arc::new(config_handler.config().to_owned());
        let config_handler = Arc::new(RwLock::new(config_handler));
        let bar = progress_bar(mods.len())?;
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
        x.to_owned()
            .try_for_each(|(name, _)| handle.locked_config_mut().remove_mod(name))?;
    }
    Ok(())
}

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
            if !file_path.is_dir() {
                fs::remove_file(file_path)
            } else {
                Ok(())
            }
        })?;
    Ok(())
}

pub fn clean() -> Result<()> {
    clean_locked_config_mods()?;
    clean_file_mods()?;
    println!("mods cleaned");
    Ok(())
}

pub fn search(name: &str, limit: Option<usize>) -> Result<()> {
    let handle = ConfigHandler::read()?;
    let loader = match handle.config().loader {
        MCLoader::Fabric(_) => "fabric",
        MCLoader::None => return Err(anyhow::anyhow!("config.toml not have loader")),
    };
    let game_version = handle.config().game_version.as_ref();
    let projects = Projects::fetch_blocking(name, limit)?;
    let res: Vec<_> = projects
        .hits
        .iter()
        .filter(|hit| {
            let project_slug = hit.slug.as_ref();
            let project_version = Versions::fetch_blocking(project_slug).unwrap();
            hit.is_mod()
                && project_version
                    .into_iter()
                    .any(|v| v.is_support_loader(loader) && v.is_support_game_version(game_version))
        })
        .map(|hit| (hit.slug.to_owned(), hit.description.to_owned()))
        .collect();
    println!("{:#?}", res);
    Ok(())
}
