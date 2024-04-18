use crate::config::{
    AssetIndex, AssetJson, InstallType, RuntimeConfig, VersionJsonLibraries, VersionManifestJson,
    VersionType,
};
use log::error;
use regex::Regex;
use reqwest::header;
use sha1::{Digest, Sha1};
use std::sync::{Arc, Mutex};
use std::{
    cmp::Ordering,
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    thread,
};

const MAX_THREAD: usize = 24;

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

trait Sha1Compare {
    fn sha1_cmp(&self, sha1code: &str) -> Ordering;
}

trait DomainReplacer<T> {
    fn replace_domain(&self, domain: &str) -> T;
}

trait PathExist {
    fn path_exists(&self) -> bool;
}

trait FileInstall {
    fn install(&self, task_len: usize, task_done: &Arc<Mutex<usize>>) -> anyhow::Result<()>;
}

trait Installer {
    fn install(self) -> anyhow::Result<()>;
}

impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

impl<T> Sha1Compare for T
where
    T: AsRef<[u8]>,
{
    fn sha1_cmp(&self, sha1code: &str) -> Ordering {
        let mut hasher = Sha1::new();
        hasher.update(self);
        let sha1 = hasher.finalize();
        hex::encode(sha1).cmp(&sha1code.into())
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

struct InstallTask {
    pub url: String,
    pub sha1: String,
    pub save_file: PathBuf,
    pub r#type: InstallType,
}

type TaskPool = VecDeque<InstallTask>;

fn fetch_bytes_with_timeout(url: &String, sha1: &str) -> anyhow::Result<bytes::Bytes> {
    let client = reqwest::blocking::Client::new();
    for _ in 0..5 {
        let send = client
            .get(url)
            .header(header::USER_AGENT, "mc_launcher")
            .send();
        let data = send.and_then(|x| x.bytes());
        if let Ok(_data) = data {
            if _data.sha1_cmp(sha1).is_eq() {
                return Ok(_data);
            }
        };
        error!("install fail, then retry");
        thread::sleep(std::time::Duration::from_millis(5));
    }
    Err(anyhow::anyhow!("download {url} fail"))
}

impl FileInstall for InstallTask {
    fn install(&self, task_len: usize, task_done: &Arc<Mutex<usize>>) -> anyhow::Result<()> {
        if !(self.save_file.path_exists()
            && fs::read(&self.save_file)
                .unwrap()
                .sha1_cmp(&self.sha1)
                .is_eq())
        {
            let data = fetch_bytes_with_timeout(&self.url, &self.sha1)?;
            fs::create_dir_all(self.save_file.parent().unwrap()).unwrap();
            fs::write(&self.save_file, data).unwrap();
        }
        let mut task_done = task_done.lock().unwrap();
        *task_done += 1;
        match &self.r#type {
            InstallType::Asset => {
                println!("{}/{} Asset {} installed", task_done, task_len, self.sha1)
            }
            InstallType::Library => println!(
                "{}/{} library {:?} installed",
                task_done,
                task_len,
                self.save_file.file_name().unwrap()
            ),
            InstallType::Client => println!("{}/{} client installed", task_done, task_len),
        }
        Ok(())
    }
}

impl<T> Installer for VecDeque<T>
where
    T: FileInstall + std::marker::Send + 'static + std::marker::Sync,
{
    fn install(self) -> anyhow::Result<()> {
        let task_len = self.len();
        let task_done = Arc::new(Mutex::new(0usize));
        let descripts = Arc::new(Mutex::new(self));
        let mut handles = vec![];
        for _ in 0..MAX_THREAD {
            let descripts_share = Arc::clone(&descripts);
            let task_done_share = Arc::clone(&task_done);
            let thr = thread::spawn(move || loop {
                let descs;
                if let Some(desc) = descripts_share.lock().unwrap().pop_back() {
                    descs = desc;
                } else {
                    return;
                }
                if let Err(e) = descs.install(task_len, &task_done_share) {
                    error!("{:#?}", e);
                    error!("Please rebuild to get Miecraft completely!");
                    panic!();
                }
            });
            handles.push(thr);
        }
        for handle in handles {
            handle.join().unwrap();
        }
        Ok(())
    }
}

pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    // install version.json then write it in version dir
    let version_json = get_version_json(config)?;
    let version_json_file = Path::new(&config.game_dir)
        .join("versions")
        .join(&config.game_version)
        .join(config.game_version.clone() + ".json");
    let native_dir = Path::new(&config.game_dir).join("natives");
    fs::create_dir_all(native_dir).unwrap_or(());
    fs::create_dir_all(version_json_file.parent().unwrap()).unwrap_or(());
    fs::write(
        version_json_file,
        serde_json::to_string_pretty(&version_json)?,
    )?;
    let asset_index = install_asset_index(config, &version_json)?;

    let mut descripts = TaskPool::new();

    descripts.append(&mut assets_installtask(config, asset_index));
    descripts.append(&mut libraries_installtask(config, &version_json)?);
    descripts.push_back(client_installtask(config, &version_json)?);
    descripts.install()?;

    Ok(())
}

fn libraries_installtask(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<TaskPool> {
    let libraries: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone())?;
    let descripts: TaskPool = libraries
        .iter()
        .filter(|obj| {
            let objs = &obj.rules.clone();
            if let Some(_objs) = objs {
                let flag = _objs
                    .iter()
                    .find(|rules| rules.os.clone().unwrap_or_default()["name"] == OS);
                obj.downloads.classifiers.is_none() && flag.clone().is_some()
            } else {
                obj.downloads.classifiers.is_none()
            }
        })
        .map(|x| {
            let artifact_path = x.downloads.artifact.path.clone();
            InstallTask {
                url: config.mirror.libraries.clone() + &artifact_path,
                sha1: x.downloads.artifact.sha1.clone(),
                save_file: Path::new(&config.game_dir)
                    .join("libraries")
                    .join(&artifact_path),
                r#type: InstallType::Library,
            }
        })
        .collect();
    Ok(descripts)
}

fn client_installtask(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<InstallTask> {
    let json_client = &version_json["downloads"]["client"];
    Ok(InstallTask {
        url: json_client["url"]
            .as_str()
            .unwrap()
            .to_string()
            .replace_domain(&config.mirror.client),
        sha1: json_client["sha1"].as_str().unwrap().to_string(),
        save_file: Path::new(&config.game_dir)
            .join("versions")
            .join(&config.game_version)
            .join(config.game_version.clone() + ".jar"),
        r#type: InstallType::Client,
    })
}

fn assets_installtask(config: &RuntimeConfig, asset_json: AssetJson) -> TaskPool {
    asset_json
        .objects
        .into_iter()
        .map(|x| InstallTask {
            url: config.mirror.assets.clone() + &x.1.hash[0..2] + "/" + &x.1.hash,
            sha1: x.1.hash.clone(),
            save_file: Path::new(&config.game_dir)
                .join("assets")
                .join("objects")
                .join(&x.1.hash[0..2])
                .join(x.1.hash.clone()),
            r#type: InstallType::Asset,
        })
        .collect()
}

fn install_asset_index(
    config: &RuntimeConfig,
    version_json: &serde_json::Value,
) -> anyhow::Result<AssetJson> {
    let asset_index: AssetIndex = serde_json::from_value(version_json["assetIndex"].clone())?;
    let url = asset_index
        .url
        .replace_domain(&config.mirror.version_manifest);
    let asset_index_file = Path::new(&config.game_dir)
        .join("assets")
        .join("indexes")
        .join(asset_index.id.clone() + ".json");

    let client = reqwest::blocking::Client::new();
    let data = client
        .get(url)
        .header(header::USER_AGENT, "mc_launcher")
        .send()?
        .text()?;
    if data.sha1_cmp(&asset_index.sha1).is_eq() {
        fs::create_dir_all(asset_index_file.parent().unwrap())?;
        fs::write(asset_index_file, &data)?;
        let datajson: AssetJson = serde_json::from_str(data.as_ref())?;
        return Ok(datajson);
    };
    Err(anyhow::anyhow!("can't get assets json"))
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
    let data = client
        .get(url)
        .header(header::USER_AGENT, "mc_launcher")
        .send()?
        .text()?;

    let data: serde_json::Value = serde_json::from_str(data.as_str())?;
    Ok(data)
}

impl VersionManifestJson {
    pub fn new(config: &RuntimeConfig) -> anyhow::Result<VersionManifestJson> {
        let mut url = config.mirror.version_manifest.clone();
        url += "mc/game/version_manifest.json";
        let client = reqwest::blocking::Client::new();
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
        user_name: "no_name".into(),
        user_type: "offline".into(),
        user_uuid: "...".into(),
        game_dir: "somepath".into(),
        game_version: "1.20.4".into(),
        java_path: "/usr/bin/java".into(),
        mirror: crate::config::MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".into(),
            assets: "...".into(),
            client: "...".into(),
            libraries: "...".into(),
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
        user_name: "no_name".into(),
        user_uuid: "...".into(),
        user_type: "offline".into(),
        game_dir: "somepath".into(),
        game_version: "1.20.4".into(),
        java_path: "/usr/bin/java".into(),
        mirror: crate::config::MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".into(),
            assets: "...".into(),
            client: "...".into(),
            libraries: "...".into(),
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
        user_name: "no_name".into(),
        user_type: "offline".into(),
        user_uuid: "...".into(),
        game_dir: "somepath".into(),
        game_version: "1.20.4".into(),
        java_path: "/usr/bin/java".into(),
        mirror: crate::config::MCMirror {
            version_manifest: "https://bmclapi2.bangbang93.com/".into(),
            assets: "...".into(),
            client: "...".into(),
            libraries: "...".into(),
        },
    };
    let version_json = get_version_json(&config).unwrap();
    let _: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone()).unwrap();
}
