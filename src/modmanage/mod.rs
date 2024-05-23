use crate::asyncuntil::AsyncIterator;
use crate::config::{LockedConfig, LockedModConfig, MCLoader, ModConfig, RuntimeConfig};
use crate::install::{InstallTask, InstallType, TaskPool};
use anyhow::Result;
use modrinth_api::{Version, Versions};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use toml;

async fn fetch_version(
    name: &str,
    version: Option<String>,
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

fn fetch_version_blocking(
    name: &str,
    version: Option<String>,
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

pub fn add(name: &str, version: Option<String>) -> Result<()> {
    let config = fs::read_to_string("config.toml")?;
    let mut config: RuntimeConfig = toml::from_str(&config)?;
    let versions = fetch_version_blocking(name, version, &config)?;

    let modconf = ModConfig {
        version: versions[0].version_number.clone(),
    };

    println!("mod {} added,version:{}", &name, &modconf.version);
    config.add_mod(name, modconf);
    fs::write("config.toml", toml::to_string_pretty(&config)?)?;

    let mod_lockedconf = LockedModConfig {
        file_name: versions[0].files[0].filename.clone(),
        version: versions[0].version_number.clone(),
        url: versions[0].files[0].url.clone(),
        sha1: versions[0].files[0].hashes.sha1.clone(),
    };

    let mut lockedconfig;
    if fs::metadata("config.lock").is_ok() {
        lockedconfig = fs::read_to_string("config.lock")?;
        let mut config: LockedConfig = toml::from_str(&lockedconfig)?;
        config.add_mod(name, mod_lockedconf);
        lockedconfig = toml::to_string_pretty(&config)?;
    } else {
        lockedconfig = toml::to_string_pretty(&LockedConfig {
            mods: Some(HashMap::from([(name.to_owned(), mod_lockedconf)])),
        })?;
    }
    fs::write("config.lock", lockedconfig)?;
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let config = fs::read_to_string("config.toml")?;
    let mut config: RuntimeConfig = toml::from_str(&config)?;
    config.remove_mod(name)?;
    fs::write("config.toml", toml::to_string_pretty(&config)?)?;

    let lockedconfig = fs::read_to_string("config.lock")?;
    let mut lockedconfig: LockedConfig = toml::from_str(&lockedconfig)?;
    let file_path =
        Path::new("mods").join(lockedconfig.mods.as_ref().unwrap()[name].file_name.clone());
    lockedconfig.remove_mod(name)?;
    fs::write("config.lock", toml::to_string_pretty(&lockedconfig)?)?;

    if fs::metadata(&file_path).is_ok() {
        fs::remove_file(file_path)?;
    }

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
                url: v.url.clone(),
                sha1: Some(v.sha1.clone()),
                save_file,
                r#type: InstallType::Mods,
            }
        })
        .collect()
}

pub fn install() -> Result<()> {
    let lockedconfig = fs::read_to_string("config.lock")?;
    let mut lockedconfig: LockedConfig = toml::from_str(&lockedconfig)?;
    let config = fs::read_to_string("config.toml")?;
    let config: RuntimeConfig = toml::from_str(&config)?;
    if config.mods.is_none() || lockedconfig.mods.is_none() {
        return Err(anyhow::anyhow!("No mod can install"));
    }
    lockedconfig.mods = Some(
        lockedconfig
            .mods
            .unwrap()
            .into_iter()
            .filter(|(name, _)| config.mods.clone().unwrap().iter().any(|(x, _)| x == name))
            .collect(),
    );
    let tasks = mod_installtasks(&lockedconfig.mods.unwrap());
    TaskPool::from(tasks).install()?;
    Ok(())
}

pub fn sync() -> Result<()> {
    sync_or_update(true)
}

fn sync_or_update(sync: bool) -> Result<()> {
    let config = fs::read_to_string("config.toml")?;
    let mut config: RuntimeConfig = toml::from_str(&config)?;
    let config_arc = Arc::new(config.clone());
    let mut lockedconfig: LockedConfig;
    if fs::metadata("config.lock").is_ok() {
        let _lockedconfig = fs::read_to_string("config.lock")?;
        lockedconfig = toml::from_str(&_lockedconfig)?;
    }else {
        lockedconfig = LockedConfig::default();
    }

    let config_mods = Arc::new(Mutex::new(HashMap::new()));
    let lockedconfig_mods = Arc::new(Mutex::new(HashMap::new()));

    if let Some(mods) = config_arc.mods.clone() {
        mods.into_iter()
            .map(|x| {
                let config_share = config_arc.clone();
                let config_mods_share = config_mods.clone();
                let lockedconfig_mods_share = lockedconfig_mods.clone();
                return async move {
                    let (name, conf) = x;
                    let versions;
                    if sync {
                        versions = fetch_version(&name, Some(conf.version), &config_share)
                            .await
                            .unwrap();
                    } else {
                        versions = fetch_version(&name, None, &config_share).await.unwrap();
                    }
                    let version = &versions[0];

                    let modconf = ModConfig {
                        version: version.version_number.clone(),
                    };
                    let locked_modconf = LockedModConfig {
                        file_name: version.files[0].filename.clone(),
                        version: version.version_number.clone(),
                        url: version.files[0].url.clone(),
                        sha1: version.files[0].hashes.sha1.clone(),
                    };
                    config_mods_share
                        .lock()
                        .unwrap()
                        .insert(name.clone(), modconf);
                    lockedconfig_mods_share
                        .lock()
                        .unwrap()
                        .insert(name, locked_modconf);
                };
            })
            .async_execute(10);
    }
    config.mods = Some(config_mods.lock().unwrap().to_owned());
    lockedconfig.mods = Some(lockedconfig_mods.lock().unwrap().to_owned());
    let config = toml::to_string_pretty(&config)?;
    let lockedconfig = toml::to_string_pretty(&lockedconfig)?;
    fs::write("config.toml", config)?;
    fs::write("config.lock", lockedconfig)?;
    if sync {
        println!("mod config synced");
    } else {
        println!("mod config updated");
    }
    Ok(())
}
