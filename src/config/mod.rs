use clap::Subcommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
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

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionManifestVersions {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    pub releaseTime: String,
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

#[derive(Subcommand, Debug)]
pub enum VersionType {
    All,
    Release,
    Snapshot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCMirror {
    pub version_manifest: String,
}
