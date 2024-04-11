use crate::config::{RuntimeConfig, VersionManifestJson, VersionType};
use regex::Regex;
use reqwest::header;
use std::fs;
use log::debug;

pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    // install version.json then write it in version dir
    let version_json = get_version_json(config)?;
    let version_dir = "versions/".to_string() + config.game_version.as_ref() + "/";
    let version_json_file = version_dir.clone() + config.game_version.as_ref() + ".json";
    fs::create_dir_all(version_dir).unwrap_or(());
    fs::write(
        version_json_file,
        serde_json::to_string_pretty(&version_json)?,
    )?;

    // install assets
    install_assets(config, &version_json)?;
    Ok(())
}

pub fn install_assets(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<()> {
    let regex = Regex::new(r"(?<replace>https://\S+?/)")?;
    let asset_index = &version_json["assetIndex"];
    let id = &asset_index["id"].as_str().unwrap().to_string();
    let url = &asset_index["url"].as_str().unwrap().to_string();
    let replace = regex.captures(url.as_str()).unwrap();
    let url = url.replace(&replace["replace"], config.mirror.version_manifest.as_ref());
    let _asset_index_sha1 = &asset_index["sha1"].to_string();
    let asset_index_dir = "assets/indexes/".to_string();
    let asset_index_file = asset_index_dir.clone() + id + ".json";


    debug!("get {}",&url);
    let client = reqwest::blocking::Client::new();
    let data = client
        .get(&url)
        .header(header::USER_AGENT, "mc_launcher")
        .send()?
        .text()?;
    
    fs::create_dir_all(asset_index_dir)?;
    fs::write(asset_index_file, data)?;

    Ok(())
}

pub fn get_version_json(config: &RuntimeConfig) -> anyhow::Result<serde_json::Value> {
    let version = config.game_version.as_ref();
    let regex = Regex::new(r"(?<replace>https://\S+?/)")?;
    let manifest = VersionManifestJson::new(config)?;
    let mut url = manifest
        .versions
        .iter()
        .find(|x| x.id == version)
        .unwrap()
        .url
        .clone();

    let replace = regex.captures(url.as_str()).unwrap();
    url = url.replace(&replace["replace"], config.mirror.version_manifest.as_ref());

    let client = reqwest::blocking::Client::new();
    debug!("get {}",&url);
    let data = client
        .get(&url)
        .header(header::USER_AGENT, "mc_launcher")
        .send()?
        .text()?;

    let data: serde_json::Value = serde_json::from_str(&data.as_str())?;
    Ok(data)
}

impl VersionManifestJson {
    pub fn new(config: &RuntimeConfig) -> anyhow::Result<VersionManifestJson> {
        let mut url = config.mirror.version_manifest.clone();
        url += "mc/game/version_manifest.json";
        let client = reqwest::blocking::Client::new();
        debug!("{}",&url);
        let data: VersionManifestJson = client
            .get(&url)
            .header(header::USER_AGENT, "mc_launcher")
            .send()?
            .json()?;
        Ok(data)
    }

    pub fn version_list(&self, version_type: VersionType) -> Vec<String> {
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
}

#[test]
fn test_get_manifest() {
    let config = RuntimeConfig {
        max_memory_size: 5000,
        window_weight: 854,
        window_height: 480,
        user_name: "no_name".to_string(),
        user_type: "offline".to_string(),
        game_dir: "somepath".to_string(),
        game_version: "1.20.4".to_string(),
        java_path: "/usr/bin/java".to_string(),
        mirror: crate::config::MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".to_string(),
        },
    };
    let _ = VersionManifestJson::new(&config).unwrap();
}

#[test]
fn test_get_version_json() {
    let config = RuntimeConfig {
        max_memory_size: 5000,
        window_weight: 854,
        window_height: 480,
        user_name: "no_name".to_string(),
        user_type: "offline".to_string(),
        game_dir: "somepath".to_string(),
        game_version: "1.20.4".to_string(),
        java_path: "/usr/bin/java".to_string(),
        mirror: crate::config::MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".to_string(),
        },
    };
    let _ = get_version_json(&config).unwrap();
}
