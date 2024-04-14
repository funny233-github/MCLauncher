use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

// runtime config
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MCMirror {
    pub version_manifest: String,
    pub assets: String,
    pub client: String,
    pub libraries: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeConfig {
    pub max_memory_size: u32,
    pub window_weight: u32,
    pub window_height: u32,
    pub user_name: String,
    pub user_type: String,
    pub game_dir: String,
    pub game_version: String,
    pub java_path: String,
    pub mirror: MCMirror,
}

// version manifest
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionManifestVersions {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde[rename = "releaseTime"]]
    pub release_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionManifestLatest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionManifestJson {
    pub latest: VersionManifestLatest,
    pub versions: Vec<VersionManifestVersions>,
}

// version type
#[derive(Subcommand, Debug)]
pub enum VersionType {
    All,
    Release,
    Snapshot,
}

// asset index
#[allow(non_snake_case)]
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
pub struct AssetJsonObject {
    pub hash: String,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetJson {
    pub objects: HashMap<String, AssetJsonObject>,
}

// version json libraries
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DownloadsArtifactObject {
    pub path: String,
    pub sha1: String,
    pub size: usize,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibDownloadsObject {
    pub artifact: DownloadsArtifactObject,
    pub classifiers: Option<HashMap<String, DownloadsArtifactObject>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LibRules {
    pub action: String,
    pub os: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibrarieObject {
    pub downloads: LibDownloadsObject,
    pub name: String,
    pub extract: Option<serde_json::Value>,
    pub rules: Option<Vec<LibRules>>,
}
pub type VersionJsonLibraries = Vec<LibrarieObject>;

// install descriptor
pub enum InstallType {
    Asset,
    Library,
    Client,
}

pub struct InstallSingleDescriptor {
    pub url: String,
    pub sha1: String,
    pub save_dir: String,
    pub file_name: String,
    pub r#type: InstallType,
}

pub type InstallDescriptors = VecDeque<InstallSingleDescriptor>;
