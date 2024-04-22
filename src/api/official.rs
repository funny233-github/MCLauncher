use crate::config::VersionType;
use log::warn;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

macro_rules! fetch {
    ($client:ident,$url:ident, json) => {{
        for _ in 0..5 {
            let send = $client
                .get(&$url)
                .header(header::USER_AGENT, "mc_launcher")
                .send();
            let data = send.and_then(|x| x.json());
            if let Ok(_data) = data {
                return Ok(_data);
            };
            warn!("install fail, then retry");
            thread::sleep(Duration::from_secs(3));
        }
        Err(anyhow::anyhow!("fetch json fail"))
    }};
}

// version manifest
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

// asset index
#[derive(Debug, Serialize, Deserialize)]
pub struct AssetIndex {
    #[serde[rename = "totalSize"]]
    pub total_size: usize,
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: usize,
}

// asset json
#[derive(Debug, Serialize, Deserialize)]
pub struct Asset {
    pub hash: String,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Assets {
    pub objects: HashMap<String, Asset>,
}

// version json libraries
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: usize,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibDownloads {
    pub artifact: Artifact,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rules {
    pub action: String,
    pub os: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Library {
    pub downloads: LibDownloads,
    pub name: String,
    pub extract: Option<serde_json::Value>,
    pub rules: Option<Vec<Rules>>,
}

pub type Libraries = Vec<Library>;

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
