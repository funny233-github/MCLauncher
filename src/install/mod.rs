use crate::config::{
    AssetIndex, AssetJson, InstallDescriptors, InstallSingleDescriptor, InstallType, RuntimeConfig,
    VersionJsonLibraries, VersionManifestJson, VersionType,
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

trait FileInstall {
    fn install(&self) -> anyhow::Result<()>;
}

trait Installer {
    fn install(&mut self) -> anyhow::Result<()>;
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

impl<T> PathExist for T
where
    T: AsRef<Path>,
{
    fn path_exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }
}

impl FileInstall for InstallSingleDescriptor {
    fn install(&self) -> anyhow::Result<()> {
        let path = Path::new(&self.save_dir).join(&self.file_name);
        if path.path_exists() && Ordering::Equal == fs::read(&path).unwrap().sha1_cmp(&self.sha1) {
            match &self.r#type {
                InstallType::Asset => println!("[CHECK] Asset {} installed", self.sha1),
                InstallType::Library => println!("[CHECK] library {} installed", self.file_name),
                InstallType::Client => println!("[CHECK] client installed"),
            }
            return Ok(());
        }
        let data = install_bytes_with_timeout(&self.url, &self.sha1)?;
        fs::create_dir_all(&self.save_dir).unwrap();
        fs::write(path, data).unwrap();
        match &self.r#type {
            InstallType::Asset => println!("Asset {} installed", self.sha1),
            InstallType::Library => println!("library {} installed", self.file_name),
            InstallType::Client => println!("client installed"),
        }
        Ok(())
    }
}

impl Installer for InstallDescriptors {
    fn install(&mut self) -> anyhow::Result<()> {
        while self.len() > 0 {
            let mut handles: VecDeque<JoinHandle<()>> = VecDeque::new();
            for _ in 0..MAX_THREAD {
                let des = self.pop_back();
                if let Some(_des) = des {
                    let thr = thread::spawn(move || {
                        if let Err(e) = _des.install() {
                            error!("{:#?}", e);
                        }
                    });
                    handles.push_back(thr);
                }
            }
            for handle in handles {
                handle.join().unwrap();
            }
        }
        Ok(())
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

    let mut descripts = InstallDescriptors::new();

    descripts.append(&mut install_asset_index_and_get_assets_descript(
        config,
        &version_json,
    )?);
    descripts.append(&mut get_libraries_and_native_descript(
        config,
        &version_json,
    )?);
    descripts.push_back(get_client_descript(config, &version_json)?);
    descripts.install()?;

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

fn get_libraries_and_native_descript(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<InstallDescriptors> {
    fs::create_dir_all(config.game_dir.clone() + "libraries/").unwrap();
    let libraries: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone())?;
    let descripts: InstallDescriptors = libraries
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
            let artifact_path = x.downloads.artifact.path.clone();
            let url = config.mirror.libraries.clone() + &artifact_path;
            let sha1 = x.downloads.artifact.sha1.clone();
            let path = Path::new(&config.game_dir)
                .join("libraries")
                .join(artifact_path);
            let save_dir = path
                .parent()
                .unwrap()
                .to_string_lossy()
                .as_ref()
                .to_string();
            let file_name = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .as_ref()
                .to_string();
            InstallSingleDescriptor {
                url,
                sha1,
                save_dir,
                file_name,
                r#type: InstallType::Library,
            }
        })
        .collect();
    Ok(descripts)
}

fn get_client_descript(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<InstallSingleDescriptor> {
    let json_client = &version_json["downloads"]["client"];
    let url = json_client["url"].as_str().unwrap().to_string();
    let url = url.replace_domain(&config.mirror.client);
    let sha1 = json_client["sha1"].as_str().unwrap().to_string();
    let save_dir = Path::new(&config.game_dir)
        .join("versions/")
        .join(config.game_version.clone() + "/")
        .to_string_lossy()
        .as_ref()
        .to_string();
    let file_name = config.game_version.clone() + ".jar";
    Ok(InstallSingleDescriptor {
        url,
        sha1,
        save_dir,
        file_name,
        r#type: InstallType::Client,
    })
}

fn get_assets_descript(config: &RuntimeConfig, asset_json: AssetJson) -> InstallDescriptors {
    asset_json
        .objects
        .into_iter()
        .map(|x| {
            let url = config.mirror.assets.clone() + &x.1.hash[0..2] + "/" + &x.1.hash;
            let sha1 = x.1.hash.clone();
            let save_dir = Path::new(&config.game_dir)
                .join("assets/")
                .join("objects/")
                .join(&x.1.hash[0..2])
                .to_string_lossy()
                .as_ref()
                .to_string();
            let file_name = x.1.hash.clone();
            InstallSingleDescriptor {
                url,
                sha1,
                save_dir,
                file_name,
                r#type: InstallType::Asset,
            }
        })
        .collect()
}

fn install_asset_index_and_get_assets_descript(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<InstallDescriptors> {
    let ass: AssetIndex = serde_json::from_value(version_json["assetIndex"].clone())?;
    let url = ass.url.replace_domain(&config.mirror.version_manifest);
    let asset_index_dir = config.game_dir.clone() + "assets/indexes/";
    let asset_index_file = asset_index_dir.clone() + &ass.id + ".json";

    info!("get {}", &url);
    let client = reqwest::blocking::Client::new();
    for _ in 0..=3 {
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
            let descripts = get_assets_descript(config, datajson);
            return Ok(descripts);
        };
        error!("get assets json fail, then retry");
    }
    return Err(anyhow::anyhow!("can't get assets json"));
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
