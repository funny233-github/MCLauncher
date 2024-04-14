use crate::config::{
    AssetIndex, AssetJson, AssetJsonObject, RuntimeConfig, VersionJsonLibraries,
    VersionManifestJson, VersionType,
};
use log::{debug, error, info};
use regex::Regex;
use reqwest::header;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::thread;
use std::thread::JoinHandle;

const MAX_THREAD: usize = 32;

trait Sha1Compare {
    fn sha1_cmp(&self, sha1code: &String) -> Ordering;
}

trait DomainReplacer<T> {
    fn replace_domain(&self, domain: &String) -> T;
}

trait PathExist {
    fn path_exists(&self) -> bool;
}

impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &String) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

impl<T> Sha1Compare for T
where
    T: AsRef<[u8]>,
{
    fn sha1_cmp(&self, sha1code: &String) -> Ordering {
        let mut hasher = Sha1::new();
        hasher.update(self);
        let sha1 = hasher.finalize();
        hex::encode(sha1).cmp(sha1code)
    }
}

impl PathExist for String {
    fn path_exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }
}

pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    // install version.json then write it in version dir
    let version_json = get_version_json(config)?;
    let version_dir = config.game_dir.clone() + "versions/" + config.game_version.as_ref() + "/";
    let version_json_file = version_dir.clone() + config.game_version.as_ref() + ".json";
    let native_dir = config.game_dir.clone() + "natives/";
    fs::create_dir_all(native_dir).unwrap_or(());
    fs::create_dir_all(version_dir).unwrap_or(());
    fs::write(
        version_json_file,
        serde_json::to_string_pretty(&version_json)?,
    )?;

    install_assets_and_asset_index(config, &version_json)?;
    install_libraries_and_native(config, &version_json)?;
    install_client(config, &version_json)?;
    Ok(())
}

fn install_bytes_with_timeout(url: &String, sha1: &String) -> anyhow::Result<bytes::Bytes> {
    let client = reqwest::blocking::Client::new();
    for _ in 0..3 {
        let send = client
            .get(url)
            .header(header::USER_AGENT, "mc_launcher")
            .send();
        if let Ok(_send) = send {
            let data = _send.bytes()?;
            if let Ordering::Equal = data.sha1_cmp(sha1) {
                return Ok(data);
            }
        }
    }
    return Err(anyhow::anyhow!("download {url} fail"));
}

fn install_single_library(config: RuntimeConfig, value: Option<(String, String)>) {
    if let Some((path, hash)) = value {
        let url = config.mirror.libraries.clone() + &path;
        let lib_path = config.game_dir.clone() + "libraries/" + &path;
        let lib_path = Path::new(&lib_path);
        let lib_dir = lib_path.parent().unwrap().to_str().unwrap().to_string();
        let lib_path = lib_path.to_str().unwrap().to_string();
        if lib_path.path_exists() && Ordering::Equal == fs::read(&lib_path).unwrap().sha1_cmp(&hash)
        {
            println!("library {} has installed", path);
        }
        let data = install_bytes_with_timeout(&url, &hash).unwrap();
        fs::create_dir_all(lib_dir).unwrap();
        fs::write(lib_path, data).unwrap();
        println!("library {} has installed", path);
    }
}

fn install_libraries_and_native(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<()> {
    fs::create_dir_all(config.game_dir.clone() + "libraries/").unwrap();
    let libraries: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone())?;
    let mut libraries_url_sha1: VecDeque<(String, String)> = libraries
        .iter()
        .filter(|obj| {
            let objs = &obj.rules.clone();
            if let Some(_objs) = objs {
                let flag = _objs
                    .iter()
                    .find(|rules| rules.os.clone().unwrap_or_default()["name"] == "linux");
                obj.downloads.classifiers == None && flag.clone() != None
            } else {
                obj.downloads.classifiers == None
            }
        })
        .map(|x| {
            let path = x.downloads.artifact.path.clone();
            let sha1 = x.downloads.artifact.sha1.clone();
            (path, sha1)
        })
        .collect();
    while libraries_url_sha1.len() > 0 {
        let mut handles: VecDeque<JoinHandle<()>> = VecDeque::new();
        for _ in 0..MAX_THREAD {
            let value = libraries_url_sha1.pop_back();
            let conf = config.clone();
            let thr = thread::spawn(move || install_single_library(conf, value));
            handles.push_back(thr);
        }
        for handle in handles {
            handle.join().unwrap();
        }
    }
    Ok(())
}

fn install_single_asset(config: RuntimeConfig, value: Option<(String, AssetJsonObject)>) {
    if let Some(_value) = value {
        let (_, v) = _value;
        let hash = &v.hash;
        let url = config.mirror.assets.clone() + &hash[0..2] + "/" + hash;
        let dir = config.game_dir.clone() + "assets/objects/" + &hash[0..2] + "/";
        let file = dir.clone() + hash;
        if file.path_exists() && Ordering::Equal == fs::read(&file).unwrap().sha1_cmp(hash) {
            println!("asset {} has installed", hash);
            return;
        }
        let data = install_bytes_with_timeout(&url, hash).unwrap();
        fs::create_dir_all(dir).unwrap();
        fs::write(file, data).unwrap();
        println!("asset {} has installed", hash);
    }
}

fn install_assets(config: &RuntimeConfig, asset_json: AssetJson) -> anyhow::Result<()> {
    let mut queue: VecDeque<(String, AssetJsonObject)> = asset_json.objects.into_iter().collect();
    while queue.len() > 0 {
        let mut handles: VecDeque<JoinHandle<()>> = VecDeque::new();
        for _ in 0..MAX_THREAD {
            let value = queue.pop_back();
            let conf = config.clone();
            let thr = thread::spawn(move || install_single_asset(conf, value));
            handles.push_back(thr);
        }
        for handle in handles {
            handle.join().unwrap();
        }
    }
    Ok(())
}

fn install_client(config: &RuntimeConfig, version_json: &serde_json::Value) -> anyhow::Result<()> {
    let json_client = &version_json["downloads"]["client"];
    let url = &json_client["url"].as_str().unwrap().to_string();
    let url = url.replace_domain(&config.mirror.client);
    let sha1 = &json_client["sha1"].as_str().unwrap().to_string();
    let data = install_bytes_with_timeout(&url, sha1)?;
    let file_dir = config.game_dir.clone() + "versions/" + config.game_version.as_ref() + "/";
    let file = file_dir.clone() + config.game_version.as_ref() + ".jar";
    if file.path_exists() && Ordering::Equal == fs::read(&file).unwrap().sha1_cmp(sha1) {
        println!("client installed");
        return Ok(());
    }
    fs::create_dir_all(file_dir)?;
    fs::write(file, data)?;
    println!("client installed");
    Ok(())
}

fn install_assets_and_asset_index(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<()> {
    let ass: AssetIndex = serde_json::from_value(version_json["assetIndex"].clone())?;
    let url = ass.url.replace_domain(&config.mirror.version_manifest);
    let asset_index_dir = config.game_dir.clone() + "assets/indexes/";
    let asset_index_file = asset_index_dir.clone() + &ass.id + ".json";

    info!("get {}", &url);
    let client = reqwest::blocking::Client::new();
    for i in 0..=3 {
        let data = client
            .get(&url)
            .header(header::USER_AGENT, "mc_launcher")
            .send()?
            .text()?;
        if let Ordering::Equal = data.sha1_cmp(&ass.sha1) {
            fs::create_dir_all(asset_index_dir)?;
            fs::write(asset_index_file, &data)?;
            info!("get assets json");
            let datajson: AssetJson = serde_json::from_str(data.as_ref())?;
            install_assets(config, datajson)?;
            break;
        };
        if i == 3 {
            return Err(anyhow::anyhow!("can't get assets json"));
        }
        error!("get assets json fail, then retry");
    }

    println!("assets installed");
    Ok(())
}

pub fn get_version_json(config: &RuntimeConfig) -> anyhow::Result<serde_json::Value> {
    let version = config.game_version.as_ref();
    let manifest = VersionManifestJson::new(config)?;
    let url = manifest
        .versions
        .iter()
        .find(|x| x.id == version)
        .unwrap()
        .url
        .clone();

    let url = url.replace_domain(&config.mirror.version_manifest);

    let client = reqwest::blocking::Client::new();
    debug!("get {}", &url);
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
        debug!("{}", &url);
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
            assets: "...".to_string(),
            client: "...".to_string(),
            libraries: "...".to_string(),
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
            assets: "...".to_string(),
            client: "...".to_string(),
            libraries: "...".to_string(),
        },
    };
    let _ = get_version_json(&config).unwrap();
}

#[test]
fn test_get_version_json_libraries() {
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
            assets: "...".to_string(),
            client: "...".to_string(),
            libraries: "...".to_string(),
        },
    };
    let version_json = get_version_json(&config).unwrap();
    let _: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone()).unwrap();
}
