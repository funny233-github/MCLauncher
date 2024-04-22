use crate::config::{
    AssetIndex, AssetJson, InstallType, RuntimeConfig, VersionJsonLibraries,
};
use crate::api::official::VersionManifest;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, warn};
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
    fn install(&self, bar: &ProgressBar) -> anyhow::Result<()>;
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

fn fetch_bytes(url: &String, sha1: &str) -> anyhow::Result<bytes::Bytes> {
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
        warn!("install fail, then retry");
        thread::sleep(std::time::Duration::from_secs(3));
    }
    Err(anyhow::anyhow!("download {url} fail"))
}

impl FileInstall for InstallTask {
    fn install(&self, bar: &ProgressBar) -> anyhow::Result<()> {
        if !(self.save_file.path_exists()
            && fs::read(&self.save_file)
                .unwrap()
                .sha1_cmp(&self.sha1)
                .is_eq())
        {
            let data = fetch_bytes(&self.url, &self.sha1)?;
            fs::create_dir_all(self.save_file.parent().unwrap()).unwrap();
            fs::write(&self.save_file, data).unwrap();
            thread::sleep(std::time::Duration::from_secs(1));
        }
        bar.inc(1);
        match &self.r#type {
            InstallType::Asset => bar.set_message(format!("Asset {} installed", self.sha1)),
            InstallType::Library => bar.set_message(format!(
                "library {:?} installed",
                self.save_file.file_name().unwrap()
            )),
            InstallType::Client => bar.set_message("client installed"),
        }
        Ok(())
    }
}

impl<T> Installer for VecDeque<T>
where
    T: FileInstall + std::marker::Send + 'static + std::marker::Sync,
{
    fn install(self) -> anyhow::Result<()> {
        let bar = ProgressBar::new(self.len() as u64);
        bar.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        let descripts = Arc::new(Mutex::new(self));
        let mut handles = vec![];
        for _ in 0..MAX_THREAD {
            let descripts_share = Arc::clone(&descripts);
            let bar_share = bar.clone();
            let thr = thread::spawn(move || loop {
                let descs;
                if let Some(desc) = descripts_share.lock().unwrap().pop_back() {
                    descs = desc;
                } else {
                    return;
                }
                if let Err(e) = descs.install(&bar_share) {
                    error!("{:#?}", e);
                    error!("Please reinstall to get Miecraft completely!");
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
    let version_json = version_json(&config.game_version, &config.mirror.version_manifest)?;
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

    let mut descripts = TaskPool::new();

    let game_dir = &config.game_dir;
    let game_version = &config.game_version;

    let asset_index =
        install_asset_index(game_dir, &config.mirror.version_manifest, &version_json)?;
    descripts.append(&mut assets_installtask(
        game_dir,
        &config.mirror.assets,
        asset_index,
    ));
    descripts.append(&mut libraries_installtask(
        game_dir,
        &config.mirror.libraries,
        &version_json,
    )?);
    descripts.push_back(client_installtask(
        game_dir,
        game_version,
        &config.mirror.client,
        &version_json,
    )?);
    descripts.install()?;

    Ok(())
}

fn libraries_installtask(
    game_dir: &str,
    libraries_mirror: &str,
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
                url: libraries_mirror.to_owned() + &artifact_path,
                sha1: x.downloads.artifact.sha1.clone(),
                save_file: Path::new(game_dir).join("libraries").join(&artifact_path),
                r#type: InstallType::Library,
            }
        })
        .collect();
    Ok(descripts)
}

fn client_installtask(
    game_dir: &str,
    game_version: &str,
    client_mirror: &str,
    version_json: &serde_json::Value,
) -> anyhow::Result<InstallTask> {
    let json_client = &version_json["downloads"]["client"];
    Ok(InstallTask {
        url: json_client["url"]
            .as_str()
            .unwrap()
            .to_string()
            .replace_domain(client_mirror),
        sha1: json_client["sha1"].as_str().unwrap().to_string(),
        save_file: Path::new(game_dir)
            .join("versions")
            .join(game_version)
            .join(game_version.to_owned() + ".jar"),
        r#type: InstallType::Client,
    })
}

fn assets_installtask(game_dir: &str, assets_mirror: &str, asset_json: AssetJson) -> TaskPool {
    asset_json
        .objects
        .into_iter()
        .map(|x| InstallTask {
            url: assets_mirror.to_owned() + &x.1.hash[0..2] + "/" + &x.1.hash,
            sha1: x.1.hash.clone(),
            save_file: Path::new(game_dir)
                .join("assets")
                .join("objects")
                .join(&x.1.hash[0..2])
                .join(x.1.hash.clone()),
            r#type: InstallType::Asset,
        })
        .collect()
}

fn install_asset_index(
    game_dir: &str,
    version_manifest_mirror: &str,
    version_json: &serde_json::Value,
) -> anyhow::Result<AssetJson> {
    let asset_index: AssetIndex = serde_json::from_value(version_json["assetIndex"].clone())?;
    let url = asset_index.url.replace_domain(version_manifest_mirror);
    let asset_index_file = Path::new(game_dir)
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

pub fn version_json(
    game_version: &str,
    version_manifest_mirror: &str,
) -> anyhow::Result<serde_json::Value> {
    let url = VersionManifest::fetch(version_manifest_mirror)?.url(game_version);
    let url = url.replace_domain(version_manifest_mirror);

    let client = reqwest::blocking::Client::new();
    let data = client
        .get(url)
        .header(header::USER_AGENT, "mc_launcher")
        .send()?
        .text()?;

    let data: serde_json::Value = serde_json::from_str(data.as_str())?;
    Ok(data)
}

#[test]
fn test_get_version_json() {
    let game_version = "1.20.4";
    let version_manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let _ = version_json(game_version, version_manifest_mirror).unwrap();
}

#[test]
fn test_get_version_json_libraries() {
    let game_version = "1.20.4";
    let version_manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = version_json(game_version, version_manifest_mirror).unwrap();
    let _: VersionJsonLibraries =
        serde_json::from_value(version_json["libraries"].clone()).unwrap();
}
