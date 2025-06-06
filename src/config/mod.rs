use crate::modmanage::{fetch_version, fetch_version_blocking};
use anyhow::Result;
use clap::Subcommand;
use mc_api::official;
use modrinth_api::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
            version_manifest: "https://piston-meta.mojang.com/".into(),
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub game_dir: String,
    pub game_version: String,
    pub java_path: String,
    pub loader: MCLoader,
    pub mirror: MCMirror,
    pub mods: Option<BTreeMap<String, ModConfig>>,
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
            self.mods = Some(BTreeMap::from([(name.to_owned(), modconf)]));
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
            game_dir: "./".into(),
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LockedModConfig {
    pub file_name: String,
    pub version: Option<String>,
    pub mc_version: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
}

impl From<Version> for LockedModConfig {
    fn from(version: Version) -> Self {
        let file = version.files.to_owned().remove(0);
        let mc_version = ConfigHandler::read()
            .unwrap()
            .config()
            .game_version
            .to_owned();
        Self {
            file_name: file.filename,
            version: Some(version.version_number),
            mc_version,
            url: Some(file.url),
            sha1: Some(file.hashes.sha1),
        }
    }
}
impl LockedModConfig {
    pub fn from_local(file_name: &str) -> Self {
        let mc_version = ConfigHandler::read()
            .unwrap()
            .config()
            .game_version
            .to_owned();
        Self {
            file_name: file_name.to_owned(),
            version: None,
            mc_version,
            url: None,
            sha1: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LockedConfig {
    pub mods: Option<BTreeMap<String, LockedModConfig>>,
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
            self.mods = Some(BTreeMap::from([(name.to_owned(), modconf)]));
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
        self.add_mod(name, LockedModConfig::from_local(name));
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserAccount {
    pub user_name: String,
    pub user_type: String,
    pub user_uuid: String,
}

impl Default for UserAccount {
    fn default() -> Self {
        Self {
            user_name: "noname".to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
        }
    }
}

impl UserAccount {
    pub fn new_offline(name: &str) -> Self {
        Self {
            user_name: name.to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
        }
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
    user_account: Mac<UserAccount>,
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

    #[inline]
    pub fn user_account(&self) -> &UserAccount {
        &self.user_account
    }

    #[inline]
    pub fn user_account_mut(&mut self) -> &mut UserAccount {
        &mut self.user_account
    }
    pub fn init() -> Result<()> {
        let handle = Self {
            config: Mac::new(RuntimeConfig::default()),
            locked_config: Mac::new(LockedConfig::default()),
            user_account: Mac::new(UserAccount::default()),
        };
        handle.write_all()?;
        Ok(())
    }

    pub fn has_mod_name(&self, mod_name: &str) -> bool {
        self.config()
            .mods
            .clone()
            .is_some_and(|mods| mods.iter().any(|(name, _)| name == mod_name))
    }

    pub fn has_locked_mod_name(&self, mod_name: &str) -> bool {
        self.locked_config()
            .mods
            .clone()
            .is_some_and(|mods| mods.iter().any(|(name, _)| name == mod_name))
    }

    pub fn is_mod_config_match(&self, name: &str, mod_conf: &ModConfig) -> bool {
        self.config()
            .mods
            .clone()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
    }

    pub fn is_locked_mod_config_match(&self, name: &str, mod_conf: &LockedModConfig) -> bool {
        self.locked_config()
            .mods
            .clone()
            .is_some_and(|mods| mods.get(name).is_some_and(|conf| conf == mod_conf))
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

        let locked_config = if fs::exists("config.lock").is_ok() {
            let data = fs::read_to_string("config.lock")?;
            Mac::new(toml::from_str(&data)?)
        } else {
            Mac::new(LockedConfig::default())
        };

        let user_account = if fs::exists("account.toml").is_ok() {
            Mac::new(toml::from_str(&fs::read_to_string("account.toml")?)?)
        } else {
            Mac::new(UserAccount::default())
        };

        Ok(ConfigHandler {
            config,
            locked_config,
            user_account,
        })
    }

    /// Add offline account which contain name
    pub fn add_offline_account(&mut self, name: &str) {
        *self.user_account_mut() = UserAccount::new_offline(name);
    }

    pub fn write_all(&self) -> Result<()> {
        fs::write("config.toml", toml::to_string_pretty(self.config.get())?)?;
        fs::write(
            "config.lock",
            toml::to_string_pretty(self.locked_config.get())?,
        )?;
        fs::write("account.toml", toml::to_string_pretty(self.user_account())?)?;

        let mod_dir = Path::new(&self.config().game_dir).join("mods");
        if fs::metadata(mod_dir).is_ok() {
            self.disable_unuse_mods()?;
            self.enable_used_mods()?;
        }
        Ok(())
    }

    /// Write `config.toml` and `config.lock`
    /// # Writing with Mutable Access
    ///
    /// When a mutable reference is used with `ConfigHandler.config`, the `config` field is
    /// updated and written to the file upon write() is call.
    /// In contrast, the `locked_config` field will not write.
    pub fn write_with_mut(&self) -> Result<()> {
        if self.config.has_mut_accessed() {
            fs::write("config.toml", toml::to_string_pretty(self.config.get())?)?;
        }
        if self.locked_config.has_mut_accessed() {
            fs::write(
                "config.lock",
                toml::to_string_pretty(self.locked_config.get())?,
            )?;
        }
        if self.user_account.has_mut_accessed() {
            fs::write("account.toml", toml::to_string_pretty(self.user_account())?)?;
        }

        let mod_dir = Path::new(&self.config().game_dir).join("mods");
        if fs::metadata(mod_dir).is_ok() {
            self.disable_unuse_mods()?;
            self.enable_used_mods()?;
        }
        Ok(())
    }

    /// Add local mod
    /// # Error
    /// Error when file mods/name not found
    pub fn add_mod_local(&mut self, name: &str) -> Result<()> {
        // Error when file not found
        let path = Path::new(&self.config().game_dir).join("mods").join(name);
        fs::metadata(path)?;

        if !self.has_mod_name(name) {
            self.config_mut().add_local_mod(name);
        }
        if !self.has_locked_mod_name(name) {
            self.locked_config_mut().add_local_mod(name);
        }

        Ok(())
    }

    /// Add unlocal mod with block
    /// # Error
    /// Error when fetch mod fail
    pub fn add_mod_unlocal_blocking(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version_blocking(name, version, &self.config)?.remove(0);

        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from(version);
        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Add unlocal mod
    /// # Error
    /// Error when fetch mod fail
    pub async fn add_mod_unlocal(&mut self, name: &str, version: &Option<String>) -> Result<()> {
        let version = fetch_version(name, version, &self.config).await?.remove(0);

        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from(version);
        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Add mod from Version data
    pub fn add_mod_from(&mut self, name: &str, version: Version) -> Result<()> {
        let modconf = ModConfig::from(version.clone());
        if !self.is_mod_config_match(name, &modconf) {
            self.config_mut().add_mod(name, modconf);
        }

        let locked_modconf = LockedModConfig::from(version);

        if !self.is_locked_mod_config_match(name, &locked_modconf) {
            self.locked_config_mut().add_mod(name, locked_modconf);
        }
        Ok(())
    }

    /// Remove mod for configs
    /// # Panic
    /// panic when can't found mod in config.lock
    pub fn remove_mod(&mut self, name: &str) -> Result<()> {
        let locked_mods = self
            .locked_config
            .mods
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No mods in locked config"))?;
        let mod_info = locked_mods
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found in locked config", name))?;
        let file_path = Path::new(&self.config().game_dir)
            .join("mods")
            .join(&mod_info.file_name);
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
        let mod_dir = Path::new(&self.config().game_dir).join("mods");
        if fs::metadata(&mod_dir).is_err() {
            return Ok(());
        }

        // Collect all used mod file names
        let used_files: Vec<String> = match (&self.config().mods, &self.locked_config().mods) {
            (Some(config_mods), Some(locked_mods)) => config_mods
                .keys()
                .filter_map(|name| locked_mods.get(name).map(|m| m.file_name.clone()))
                .collect(),
            _ => Vec::new(),
        };

        for entry in WalkDir::new(&mod_dir).into_iter().filter_map(|e| e.ok()) {
            if let Some(name) = entry.file_name().to_str() {
                // WalkDir returns the directory itself ("mods") as an entry, which we must filter out
                // to avoid incorrectly processing the mods directory as a mod file
                if name == "mods" || name.ends_with(".unuse") {
                    continue;
                }

                if !used_files.contains(&name.to_string()) {
                    let path = mod_dir.join(name);
                    let new_path = mod_dir.join(format!("{}.unuse", name));
                    fs::rename(path, new_path)?;
                }
            }
        }
        Ok(())
    }

    /// Rename file which list in `config.toml` from `mod_filename.unuse` to `mod_filename`
    pub fn enable_used_mods(&self) -> Result<()> {
        let mod_dir = Path::new(&self.config().game_dir).join("mods");
        if fs::metadata(&mod_dir).is_err() {
            return Ok(());
        }

        let used_files = match (&self.config().mods, &self.locked_config().mods) {
            (Some(config_mods), Some(locked_mods)) => config_mods
                .keys()
                .filter_map(|name| locked_mods.get(name).map(|m| m.file_name.clone()))
                .collect(),
            _ => Vec::new(),
        };

        for entry in WalkDir::new(&mod_dir)
            .into_iter()
            .filter(|entry| match entry {
                Ok(e) => e
                    .file_name()
                    .to_str()
                    .unwrap_or_else(|| panic!("Failed to convert str to String"))
                    .ends_with(".unuse"),
                Err(e) => panic!("{}", e),
            })
        {
            if let Some(name) = &entry?.file_name().to_str() {
                if used_files.iter().any(|x| &format!("{}.unuse", x) == name) {
                    let path = Path::new("mods").join(name);
                    let mut new_name = name.to_string();
                    new_name.truncate(name.len() - 6);
                    let new_path = Path::new("mods").join(new_name);
                    fs::rename(path, new_path)?;
                }
            };
        }
        Ok(())
    }
}

impl Drop for ConfigHandler {
    #[inline]
    fn drop(&mut self) {
        if let Err(e) = self.write_with_mut() {
            panic!("Failed to write config on drop: {}", e);
        }
    }
}
