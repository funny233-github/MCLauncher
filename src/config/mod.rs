use crate::modmanage::{fetch_version, fetch_version_blocking};
use anyhow::Result;
use clap::Subcommand;
use mc_api::official;
use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::ops::{Deref, DerefMut};
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
    /// Add mod for config
    /// # Examples
    /// ```
    /// use launcher::config::{RuntimeConfig, ModConfig};
    /// let mut config = RuntimeConfig::default();
    /// let mod_conf = ModConfig::from_local("file name");
    /// config.add_mod("mod name", mod_conf);
    /// ```
    pub fn add_mod(&mut self, name: &str, modconf: ModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(HashMap::from([(name.to_owned(), modconf)]));
        }
    }

    /// Add local mod for config
    /// # Examples
    /// ```
    /// use launcher::config::RuntimeConfig;
    /// let mut config = RuntimeConfig::default();
    /// config.add_local_mod("file name");
    /// ```
    pub fn add_local_mod(&mut self, file_name: &str) {
        let modconf = ModConfig::from_local(file_name);
        self.add_mod(file_name, modconf);
    }

    /// Remove mod for config
    /// # Examples
    /// ```
    /// use launcher::config::RuntimeConfig;
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
        let file = version.files.remove(0);
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
    /// add mod for locked config
    /// # Examples
    /// ```
    /// use launcher::config::{LockedConfig, LockedModConfig};
    /// let mut config = LockedConfig::default();
    /// let mod_conf = LockedModConfig::from_local("file name");
    /// config.add_mod("mod name", mod_conf);
    /// ```
    pub fn add_mod(&mut self, name: &str, modconf: LockedModConfig) {
        if let Some(mods) = self.mods.as_mut() {
            mods.insert(name.to_owned(), modconf);
        } else {
            self.mods = Some(HashMap::from([(name.to_owned(), modconf)]));
        }
    }

    /// add local mod for locked config
    /// # Examples
    /// ```
    /// use launcher::config::LockedConfig;
    /// let mut config = LockedConfig::default();
    /// config.add_local_mod("file name");
    /// ```
    pub fn add_local_mod(&mut self, name: &str) {
        let modconf = LockedModConfig {
            file_name: name.to_owned(),
            version: None,
            url: None,
            sha1: None,
        };
        self.add_mod(name, modconf);
    }

    /// remove mod for locked config
    /// # Examples
    /// ```
    /// use launcher::config::LockedConfig;
    /// let mut config = LockedConfig::default();
    /// config.add_local_mod("file name");
    /// config.remove_mod("file name");
    /// ```
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

/// Mutable access count
#[derive(Debug, Clone)]
struct Mac<T> {
    value: T,
    has_mut_accessed: bool,
}

impl<T> Mac<T> {
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            value,
            has_mut_accessed: false,
        }
    }

    #[inline]
    pub const fn get(&self) -> &T {
        &self.value
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.has_mut_accessed = true;
        &mut self.value
    }

    #[inline]
    pub fn has_mut_accessed(&self) -> bool {
        self.has_mut_accessed
    }
}

impl<T> From<T> for Mac<T> {
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

/// # Writing with Mutable Access
///
/// When a mutable reference is used with `ConfigHandler.config`, the `config` field is
/// updated and written to the file upon `ConfigHandler`'s drop.
/// In contrast, the `locked_config` field will not write.
#[derive(Debug, Clone)]
pub struct ConfigHandler {
    config: Mac<RuntimeConfig>,
    locked_config: Mac<LockedConfig>,
}

impl ConfigHandler {
    #[inline]
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    #[inline]
    pub fn config_mut(&mut self) -> &mut RuntimeConfig {
        &mut self.config
    }

    #[inline]
    pub fn locked_config(&self) -> &LockedConfig {
        &self.locked_config
    }

    #[inline]
    pub fn locked_config_mut(&mut self) -> &mut LockedConfig {
        &mut self.locked_config
    }
    /// Read config.toml and config.lock
    /// # Error
    /// Error when config.toml not exist
    /// Error When config.toml or config.lock context is invalid
    /// # Note
    /// When config.lock not exist, this function will create a default config.lock data
    pub fn read() -> Result<Self> {
        let config = fs::read_to_string("config.toml")?;
        let config: RuntimeConfig = toml::from_str(&config)?;

        if let Some(mods) = config.mods.as_ref() {
            for (name, conf) in mods {
                if conf.file_name.is_some() && conf.version.is_some() {
                    return Err(anyhow::anyhow!(
                        "The mod {} have file_name and version in same time!",
                        name
                    ));
                }
            }
        }
        let config = Mac::new(config);

        let locked_config = if fs::metadata("config.lock").is_ok() {
            let data = fs::read_to_string("config.lock")?;
            toml::from_str(&data)?
        } else {
            LockedConfig::default()
        };

        let locked_config = Mac::new(locked_config);
        Ok(ConfigHandler {
            config,
            locked_config,
        })
    }

    /// Write `config.toml` and `config.lock`
    /// # Writing with Mutable Access
    ///
    /// When a mutable reference is used with `ConfigHandler.config`, the `config` field is
    /// updated and written to the file upon write() is call.
    /// In contrast, the `locked_config` field will not write.
    pub fn write(&self) -> Result<()> {
        if self.config.has_mut_accessed() {
            fs::write("config.toml", toml::to_string_pretty(self.config.get())?)?;
        }
        if self.locked_config.has_mut_accessed() {
            fs::write(
                "config.lock",
                toml::to_string_pretty(self.locked_config.get())?,
            )?;
        }
        self.disable_unuse_mods()?;
        self.enable_used_mods()?;
        Ok(())
    }

    /// Add local mod
    /// # Error
    /// Error when file mods/name not found
    pub fn add_mod_local(&mut self, name: &str) -> Result<()> {
        // Error when file not found
        let path = Path::new("mods").join(name);
        fs::metadata(path)?;

        self.config_mut().add_local_mod(name);
        self.locked_config_mut().add_local_mod(name);

        Ok(())
    }

    /// Add unlocal mod with block
    /// # Error
    /// Error when fetch mod fail
    pub fn add_mod_unlocal_blocking(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version_blocking(name, version, &self.config)?.remove(0);

        let modconf = ModConfig::from(version.clone());
        self.config_mut().add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config_mut().add_mod(name, locked_modconf);
        Ok(())
    }

    /// Add unlocal mod
    /// # Error
    /// Error when fetch mod fail
    pub async fn add_mod_unlocal(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version(name, version, &self.config).await?.remove(0);

        let modconf = ModConfig::from(version.clone());
        self.config_mut().add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config_mut().add_mod(name, locked_modconf);
        Ok(())
    }

    /// Add mod from Version data
    pub fn add_mod_from(&mut self, name: &str, version: Version) -> Result<()> {
        let modconf = ModConfig::from(version.clone());
        self.config_mut().add_mod(name, modconf);

        let locked_modconf = LockedModConfig::from(version);
        self.locked_config_mut().add_mod(name, locked_modconf);
        Ok(())
    }

    /// Remove mod for configs
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

        self.config_mut().remove_mod(name);
        self.locked_config_mut().remove_mod(name)?;

        Ok(())
    }

    /// Rename file which not list in `config.toml` to `mod_filename.unuse`
    pub fn disable_unuse_mods(&self) -> Result<()> {
        let file_names = self.config().mods.as_ref().map(|x| {
            x.iter().map(|(name, _)| {
                self.locked_config()
                    .mods
                    .as_ref()
                    .map(|x| &x[name].file_name)
            })
        });

        for entry in WalkDir::new("mods").into_iter().filter(|x| {
            let name = x.as_ref().unwrap().file_name().to_str().unwrap();
            name != "mods" && (!name.ends_with(".unuse"))
        }) {
            let name = &entry?.file_name().to_str().unwrap().to_owned();

            if !(file_names.is_some()
                && file_names
                    .as_ref()
                    .unwrap()
                    .to_owned()
                    .any(|x| x.unwrap() == name))
            {
                let path = Path::new("mods").join(name);
                let new_name = format!("{}.unuse", name);
                let new_path = Path::new("mods").join(new_name);
                fs::rename(path, new_path)?;
            }
        }
        Ok(())
    }

    /// Rename file which list in `config.toml` from `mod_filename.unuse` to `mod_filename`
    pub fn enable_used_mods(&self) -> Result<()> {
        let file_names = self.config().mods.as_ref().map(|x| {
            x.iter().map(|(name, _)| {
                self.locked_config()
                    .mods
                    .as_ref()
                    .map(|x| &x[name].file_name)
            })
        });

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
                    .as_ref()
                    .unwrap()
                    .to_owned()
                    .any(|x| &format!("{}.unuse", x.unwrap()) == name)
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

impl Drop for ConfigHandler {
    #[inline]
    fn drop(&mut self) {
        self.write().unwrap();
    }
}
