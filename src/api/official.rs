// provide related function with minecraft official api
use super::{DomainReplacer, Sha1Compare};
use crate::config::VersionType;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

// version json libraries
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: usize,
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
    /// example:
    /// ```
    /// use launcher::api::official::{VersionManifest, Version,Libraries};
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let version = Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// let libraries = version.libraries;
    /// let targets = libraries.iter().filter(|x|x.is_target_lib()).map(|x|x.clone());
    /// let targets:Libraries = targets.collect();
    /// assert!(targets.len() > 0);
    /// ```
    pub fn is_target_lib(&self) -> bool {
        if let Some(rule) = &self.rules {
            let flag = rule
                .iter()
                .find(|x| x.os.clone().unwrap_or_default()["name"] == OS);
            self.downloads.classifiers.is_none() && flag.is_some()
        } else {
            self.downloads.classifiers.is_none()
        }
    }

    /// return true if is required native
    /// example:
    /// ```
    /// use launcher::api::official::{VersionManifest, Version,Libraries};
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
    /// example:
    /// ```
    /// use launcher::api::official::VersionManifest;
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let _ = VersionManifest::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(version_manifest_mirror: &str) -> anyhow::Result<VersionManifest> {
        let url = version_manifest_mirror.to_owned() + "mc/game/version_manifest.json";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }

    /// fetch version list fromm manifest
    /// example:
    /// ```
    /// use launcher::api::official::VersionManifest;
    /// use launcher::config::VersionType;
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
    /// example:
    /// ```
    /// use launcher::api::official::VersionManifest;
    /// use launcher::config::VersionType;
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
    /// example:
    /// ```
    /// use launcher::api::official::VersionManifest;
    /// use launcher::api::official::Assets;
    /// use launcher::api::official::Version;
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let assets_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let version = Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// let _ = Assets::fetch(&version.asset_index, assets_mirror).unwrap();
    /// ```
    pub fn fetch(asset_index: &AssetIndex, mirror: &str) -> anyhow::Result<Assets> {
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
    pub game: serde_json::Value,
    pub jvm: serde_json::Value,
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

impl Version {
    /// fetch version json from VersionManifest
    /// example:
    /// ```
    /// use launcher::api::official::{VersionManifest, Version};
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    /// let _ = Version::fetch(manifest, "1.20.4", manifest_mirror).unwrap();
    /// ```
    pub fn fetch(
        manifest: VersionManifest,
        version: &str,
        mirror: &str,
    ) -> anyhow::Result<Version> {
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
}
