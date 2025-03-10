/// provide related function with minecraft official api
use super::{DomainReplacer, Sha1Compare};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

// version type
#[derive(Debug)]
pub enum VersionType {
    All,
    Release,
    Snapshot,
}

// version json libraries
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Artifact {
    pub path: String,
    pub sha1: Option<String>,
    pub size: Option<i32>,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LibDownloads {
    pub artifact: Artifact,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rules {
    pub action: String,
    pub os: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Library {
    pub downloads: LibDownloads,
    pub name: String,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rules>>,
}

impl Library {
    /// return true if the Library is target lib which is required
    /// # Examples
    /// ```
    /// use mc_api::official::{VersionManifest, Version,Libraries};
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let version = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    /// let libraries = version.libraries;
    /// let targets = libraries.iter().filter(|x|x.is_target_lib()).map(|x|x.clone());
    /// let targets:Libraries = targets.collect();
    /// assert!(targets.len() > 0);
    /// ```
    pub fn is_target_lib(&self) -> bool {
        if let Some(rule) = &self.rules {
            let is_for_current_os = rule
                .iter()
                .find(|x| x.os.is_none() || x.os.as_ref().map(|x| x["name"] == OS).unwrap());
            self.downloads.classifiers.is_none() && is_for_current_os.is_some()
        } else {
            self.downloads.classifiers.is_none()
        }
    }

    /// return true if is required native
    /// # Examples
    /// ```
    /// use mc_api::official::{VersionManifest, Version,Libraries};
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let version = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    /// let libraries = version.libraries;
    /// let targets = libraries.iter().filter(|x|x.is_target_native()).map(|x|x.clone());
    /// let targets:Libraries = targets.collect();
    /// assert!(targets.len() > 0);
    /// ```
    pub fn is_target_native(&self) -> bool {
        self.natives.as_ref().and_then(|x| x.get(OS)).is_some()
    }
}

pub type Libraries = Vec<Library>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Versions {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde[rename = "releaseTime"]]
    pub release_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LatestVersion {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersion,
    pub versions: Vec<Versions>,
}

impl VersionManifest {
    /// fetch mc official version manifest based on mirror
    /// # Examples
    /// ```
    /// use mc_api::official::VersionManifest;
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let _ = VersionManifest::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned() + "mc/game/version_manifest.json";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }

    /// fetch version list from manifest
    /// # Examples
    /// ```
    /// use mc_api::official::VersionManifest;
    /// use mc_api::official::VersionType;
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(mirror).unwrap();
    /// let all = manifest.list(VersionType::All);
    /// let release = manifest.list(VersionType::Release);
    /// let snapshot = manifest.list(VersionType::Snapshot);
    /// assert!(all.len() > 0);
    /// assert!(release.len() > 0);
    /// assert!(snapshot.len() > 0);
    /// ```
    pub fn list(&self, version_type: VersionType) -> Vec<String> {
        match version_type {
            VersionType::All => self.versions.iter().map(|x| x.id.clone()).collect(),
            VersionType::Release => self
                .versions
                .iter()
                .filter(|x| x.r#type == "release")
                .map(|x| x.id.clone())
                .collect(),
            VersionType::Snapshot => self
                .versions
                .iter()
                .filter(|x| x.r#type == "snapshot")
                .map(|x| x.id.clone())
                .collect(),
        }
    }

    /// fetch url based on version
    /// attention: the url provided by official
    /// if version not exist then panic
    /// # Examples
    /// ```
    /// use mc_api::official::VersionManifest;
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(mirror).unwrap();
    /// let url = manifest.url("1.20.4");
    /// assert!(url.len() > 0);
    /// ```
    pub fn url(&self, version: &str) -> String {
        self.versions
            .iter()
            .find(|x| x.id == version)
            .unwrap()
            .url
            .to_owned()
    }
}

/// asset index in version.json
#[derive(Debug, Serialize, Deserialize)]
pub struct AssetIndex {
    #[serde[rename = "totalSize"]]
    pub total_size: usize,
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    pub hash: String,
    pub size: usize,
}

/// assets.json
/// which from minecraftfile/assets/indexes/'id'.json
#[derive(Debug, Serialize, Deserialize)]
pub struct Assets {
    pub objects: HashMap<String, Asset>,
}

impl Assets {
    /// fetch Assets from AssetIndex
    /// #Examples
    /// ```
    /// use mc_api::official::VersionManifest;
    /// use mc_api::official::Assets;
    /// use mc_api::official::Version;
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let assets_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let version = Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// let _ = Assets::fetch(&version.asset_index, assets_mirror).unwrap();
    /// ```
    pub fn fetch(asset_index: &AssetIndex, mirror: &str) -> anyhow::Result<Self> {
        let url = asset_index.url.replace_domain(mirror);
        let client = reqwest::blocking::Client::new();
        let sha1 = &asset_index.sha1;
        let data = fetch!(client, url, sha1, text)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// install Assets
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

/// version.json
/// which from minecraftfile/versions/'version'/'version'.json
#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub arguments: Arguments,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
    pub assets: String,
    #[serde(rename = "complianceLevel")]
    pub compliance_level: usize,
    pub downloads: serde_json::Value,
    pub id: String,
    #[serde(rename = "javaVersion")]
    pub java_version: serde_json::Value,
    pub libraries: Libraries,
    pub logging: serde_json::Value,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minimumLauncherVersion")]
    pub minimum_launcher_version: usize,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub time: String,
    pub r#type: String,
}

pub trait MergeVersion {
    fn official_libraries(&self) -> Option<Vec<Library>>;
    fn main_class(&self) -> Option<String>;
    fn arguments_game(&self) -> Option<Vec<serde_json::Value>>;
    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>>;
}

impl Version {
    /// fetch version json from VersionManifest
    /// #Examples
    /// ```
    /// use mc_api::official::{VersionManifest, Version};
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let _ = Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// ```
    pub fn fetch(manifest: VersionManifest, version: &str, mirror: &str) -> anyhow::Result<Self> {
        let url = manifest.url(version).replace_domain(mirror);
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }

    /// install version json
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    /// merge other api such as fabric prifile and official version json
    /// # Examples
    /// ```
    /// use mc_api::official;
    /// struct TestProfile {};
    /// impl official::MergeVersion for TestProfile {
    ///     fn official_libraries(&self) -> Option<Vec<official::Library>> {
    ///         None
    ///     }
    ///     fn main_class(&self) -> Option<String> {
    ///         None
    ///     }
    ///     fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
    ///         None
    ///     }
    ///     fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
    ///         None
    ///     }
    /// }
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = official::VersionManifest::fetch(manifest_mirror).unwrap();
    /// let mut version = official::Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// let other = TestProfile{};
    /// version.merge(other);
    /// ```
    pub fn merge<T>(&mut self, other: T)
    where
        T: MergeVersion,
    {
        if let Some(mut libs) = other.official_libraries() {
            self.libraries.append(&mut libs);
        }
        if let Some(main_class) = other.main_class() {
            self.main_class = main_class;
        }
        if let Some(mut arguments_game) = other.arguments_game() {
            self.arguments.game.append(&mut arguments_game)
        }
        if let Some(mut arguments_jvm) = other.arguments_jvm() {
            self.arguments.jvm.append(&mut arguments_jvm)
        }
    }
}
