use clap::Subcommand;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug,Serialize,Deserialize,Clone)]
pub enum MCLoader {
    None,
    Fabric(String),
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
            mirror: MCMirror::official_mirror(),
            loader: MCLoader::None,
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
