use crate::modmanage::{fetch_version, fetch_version_blocking};
use anyhow::Result;
use clap::Subcommand;
use mc_api::official;
use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

// runtime config
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MCMirror {
    pub version_manifest: String,
    pub assets: String,
    pub client: String,
    pub libraries: String,
    pub fabric_meta: String,
    pub fabric_maven: String,
}

impl MCMirror {
    pub fn official_mirror() -> Self {
        MCMirror {
            version_manifest: "https://launchermeta.mojang.com/".into(),
            assets: "https://resources.download.minecraft.net/".into(),
            client: "https://launcher.mojang.com/".into(),
            libraries: "https://libraries.minecraft.net/".into(),
            fabric_meta: "https://meta.fabricmc.net/".into(),
            fabric_maven: "https://maven.fabricmc.net/".into(),
        }
    }
    pub fn bmcl_mirror() -> Self {
        MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".into(),
            assets: "https://bmclapi2.bangbang93.com/assets/".into(),
            client: "https://bmclapi2.bangbang93.com/".into(),
            libraries: "https://bmclapi2.bangbang93.com/maven/".into(),
            fabric_meta: "https://bmclapi2.bangbang93.com/fabric-meta/".into(),
            fabric_maven: "https://bmclapi2.bangbang93.com/maven/".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MCLoader {
    None,
    Fabric(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModConfig {
    pub version: Option<String>,
    pub file_name: Option<String>,
}

impl From<Version> for ModConfig {
    fn from(version: Version) -> Self {
        Self {
            version: Some(version.version_number),
            file_name: None,
        }
    }
}

impl ModConfig {
    pub fn from_local(file_name: &str) -> Self {
        Self {
            version: None,
            file_name: Some(file_name.to_owned()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeConfig {
    pub max_memory_size: u32,
    pub window_weight: u32,
    pub window_height: u32,
    pub user_name: String,
    pub user_type: String,
    pub user_uuid: String,
    pub game_dir: String,
    pub game_version: String,
    pub java_path: String,
    pub loader: MCLoader,
    pub mirror: MCMirror,
    pub mods: Option<HashMap<String, ModConfig>>,
}

impl RuntimeConfig {
    pub fn add_mod(&mut self, name: &str, modconf: ModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(HashMap::from([(name.to_owned(), modconf)]));
        }
    }

    pub fn add_local_mod(&mut self, file_name: &str) {
        let modconf = ModConfig::from_local(file_name);
        self.add_mod(file_name, modconf);
    }

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
    fn default() -> Self {
        RuntimeConfig {
            max_memory_size: 5000,
            window_weight: 854,
            window_height: 480,
            user_name: "no_name".into(),
            user_type: "offline".into(),
            user_uuid: Uuid::new_v4().into(),
            game_dir: std::env::current_dir()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
                + "/",
            game_version: "no_game_version".into(),
            java_path: "java".into(),
            loader: MCLoader::None,
            mirror: MCMirror::official_mirror(),
            mods: None,
        }
    }
}

// version type
#[derive(Subcommand, Debug)]
pub enum VersionType {
    All,
    Release,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LockedModConfig {
    pub file_name: String,
    pub version: Option<String>,
    pub url: Option<String>,
    pub sha1: Option<String>,
}

impl From<Version> for LockedModConfig {
    fn from(mut version: Version) -> Self {
        let file = version.files.pop().unwrap();
        Self {
            file_name: file.filename,
            version: Some(version.version_number),
            url: Some(file.url),
            sha1: Some(file.hashes.sha1),
        }
    }
}
impl LockedModConfig {
    pub fn from_local(file_name: &str) -> Self {
        Self {
            file_name: file_name.to_owned(),
            version: None,
            url: None,
            sha1: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LockedConfig {
    pub mods: Option<HashMap<String, LockedModConfig>>,
}

impl LockedConfig {
    pub fn add_mod(&mut self, name: &str, modconf: LockedModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(HashMap::from([(name.to_owned(), modconf)]));
        }
    }

    pub fn add_local_mod(&mut self, name: &str) {
        let modconf = LockedModConfig {
            file_name: name.to_owned(),
            version: None,
            url: None,
            sha1: None,
        };
        self.add_mod(name, modconf);
    }

    pub fn remove_mod(&mut self, name: &str) -> Result<()> {
        if let Some(mods) = self.mods.as_mut() {
            mods.remove(name);
            if mods.is_empty() {
                self.mods = None;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConfigHandler {
    pub config: RuntimeConfig,
    pub locked_config: LockedConfig,
}

impl ConfigHandler {
    pub fn read() -> Result<Self> {
        let config = fs::read_to_string("config.toml")?;
        let locked_config = if fs::metadata("config.lock").is_ok() {
            let data = fs::read_to_string("config.lock")?;
            toml::from_str(&data)?
        } else {
            LockedConfig::default()
        };
        Ok(ConfigHandler {
            config: toml::from_str(&config)?,
            locked_config,
        })
    }

    pub fn write(&self) -> Result<()> {
        fs::write("config.toml", toml::to_string_pretty(&self.config)?)?;
        fs::write("config.lock", toml::to_string_pretty(&self.locked_config)?)?;
        Ok(())
    }

    pub fn write_config(&self) -> Result<()> {
        fs::write("config.toml", toml::to_string_pretty(&self.config)?)?;
        Ok(())
    }

    pub fn write_locked_config(&self) -> Result<()> {
        fs::write("config.lock", toml::to_string_pretty(&self.locked_config)?)?;
        Ok(())
    }

    pub fn add_mod_local(&mut self, name: &str) -> Result<()> {
        // Error when file not found
        let path = Path::new("mods").join(name);
        fs::metadata(path)?;

        self.config.add_local_mod(name);
        self.locked_config.add_local_mod(name);

        Ok(())
    }

    pub fn add_mod_unlocal_blocking(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version_blocking(name, version, &self.config)?.remove(0);

        let modconf = ModConfig::from(version.clone());
        self.config.add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config.add_mod(name, locked_modconf);
        Ok(())
    }

    pub async fn add_mod_unlocal(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version(name, version, &self.config).await?.remove(0);

        let modconf = ModConfig::from(version.clone());
        self.config.add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config.add_mod(name, locked_modconf);
        Ok(())
    }

    pub fn add_mod_from(&mut self, name: &str, version: Version) -> Result<()> {
        let modconf = ModConfig::from(version.clone());
        self.config.add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config.add_mod(name, locked_modconf);
        Ok(())
    }

    /// # Panic
    /// panic when can't found mod in config.lock
    pub fn remove_mod(&mut self, name: &str) -> Result<()> {
        let file_path =
            Path::new("mods").join(&self.locked_config.mods.as_ref().unwrap()[name].file_name);
        // the config independent with file of mod
        // so the file of mod may not exist
        if fs::metadata(&file_path).is_ok() {
            fs::remove_file(file_path)?;
        }

        self.config.remove_mod(name);
        self.locked_config.remove_mod(name)?;

        Ok(())
    }

    pub fn disable_unuse_mods(&self) -> Result<()> {
        let mut file_names = self
            .locked_config
            .mods
            .as_ref()
            .map(|x| x.iter().map(|(_, x)| &x.file_name));

        for entry in WalkDir::new("mods").into_iter().filter(|x| {
            let name = x.as_ref().unwrap().file_name().to_str().unwrap();
            name != "mods" && (!name.ends_with(".unuse"))
        }) {
            let name = &entry?.file_name().to_str().unwrap().to_owned();

            if !(file_names.is_some() && file_names.as_mut().unwrap().any(|x| x == name)) {
                let path = Path::new("mods").join(name);
                let new_name = format!("{}.unuse", name);
                let new_path = Path::new("mods").join(new_name);
                fs::rename(path, new_path)?;
            }
        }
        Ok(())
    }

    pub fn enable_used_mods(&self) -> Result<()> {
        let mut file_names = self
            .locked_config
            .mods
            .as_ref()
            .map(|x| x.iter().map(|(_, x)| &x.file_name));

        for entry in WalkDir::new("mods").into_iter().filter(|x| {
            x.as_ref()
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".unuse")
        }) {
            let name = &entry?.file_name().to_str().unwrap().to_owned();

            if file_names.is_some()
                && file_names
                    .as_mut()
                    .unwrap()
                    .any(|x| &format!("{}.unuse", x) == name)
            {
                let path = Path::new("mods").join(name);
                let mut new_name = name.clone();
                new_name.truncate(name.len() - 6);
                let new_path = Path::new("mods").join(new_name);
                fs::rename(path, new_path)?;
            }
        }
        Ok(())
    }
}
