use crate::{
    api::official::{Assets, Version, VersionManifest},
    config::RuntimeConfig,
};
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use regex::Regex;
use reqwest::header;
use sha1::{Digest, Sha1};
use std::{
    cmp::Ordering,
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
};

const MAX_THREAD: usize = 24;

trait Sha1Compare {
    fn sha1_cmp(&self, sha1code: &str) -> Ordering;
}

trait DomainReplacer<T> {
    fn replace_domain(&self, domain: &str) -> T;
}

trait PathExist {
    fn path_exists(&self) -> bool;
}

pub trait FileInstall {
    fn install(&self, bar: &ProgressBar) -> anyhow::Result<()>;
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

#[derive(Debug,Default,Clone,PartialEq)]
pub enum InstallType {
    #[default]
    Asset,
    Library,
    Client,
}

#[derive(Debug,Default,Clone,PartialEq)]
pub struct InstallTask {
    pub url: String,
    pub sha1: String,
    pub save_file: PathBuf,
    pub r#type: InstallType,
}

#[derive(Debug,Default,Clone)]
pub struct TaskPool<T>
where
    T: FileInstall + std::marker::Send + 'static + std::marker::Sync + Clone,
{
    pub pool: Arc<Mutex<VecDeque<T>>>,
}

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

impl<T> TaskPool<T>
where
    T: FileInstall + std::marker::Send + 'static + std::marker::Sync + Clone,
{
    ///Constructs a new TaskPool
    ///# Panics
    ///This function might panic when called if the lock is already held by
    ///the current thread
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask};
    ///let pool:TaskPool<InstallTask> = TaskPool::new();
    ///```
    pub fn new() -> Self {
        TaskPool {
            pool: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    ///Returns the number of task in the Pool
    ///# Panics
    ///This function might panic when called if the lock is already held by
    ///the current thread
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask, InstallType};
    ///let mut pool = TaskPool::new();
    ///let task = InstallTask::default();
    ///assert_eq!(pool.len(), 0);
    ///pool.push_back(task);
    ///assert_eq!(pool.len(), 1);
    ///```
    pub fn len(&self) -> usize {
        self.pool.lock().unwrap().len()
    }

    ///Returns `true` if the Pool is empty.
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask};
    ///let mut pool = TaskPool::new();
    ///let task = InstallTask::default();
    ///assert!(pool.is_empty());
    ///pool.push_back(task);
    ///assert!(!pool.is_empty());
    ///```
    pub fn is_empty(&self) -> bool {
        self.pool.lock().unwrap().is_empty()
    }

    ///Removes the last task from the Pool and returns it, or `None` if
    ///it is empty
    ///# Panics
    ///This function might panic when called if the lock is already held by
    ///the current thread
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask, InstallType};
    ///use std::path::Path;
    ///let mut pool = TaskPool::new();
    ///let task = InstallTask::default();
    ///assert_eq!(pool.pop_back(), None);
    ///pool.push_back(task.clone());
    ///assert_eq!(pool.pop_back(), Some(task));
    pub fn pop_back(&self) -> Option<T> {
        self.pool.lock().unwrap().pop_back()
    }

    ///Appends an task to the back of the Pool
    ///# Panics
    ///This function might panic when called if the lock is already held by
    ///the current thread
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask, InstallType};
    ///use std::path::Path;
    ///let mut pool = TaskPool::new();
    ///let task = InstallTask::default();
    ///assert_eq!(pool.len(), 0);
    ///pool.push_back(task);
    ///assert_eq!(pool.len(), 1);
    ///```
    pub fn push_back(&self, value: T) {
        self.pool.lock().unwrap().push_back(value)
    }

    ///Moves all the tasks of `other` into `self`, leaving `other` empty.
    ///# Notice
    ///The `other` must be a &mut VecDeque<T> type
    ///# Panics
    ///Panics if the new number of elements in self overflows a `usize`
    ///This function might panic when called if the lock is already held by
    ///the current thread
    ///# Examples
    ///```
    ///use launcher::install::{TaskPool, InstallTask, InstallType};
    ///use std::collections::VecDeque;
    ///use std::path::Path;
    ///let mut pool1 = TaskPool::new();
    ///let mut p = VecDeque::new();
    ///let task = InstallTask::default();
    ///p.push_back(task.clone());
    ///pool1.push_back(task.clone());
    ///pool1.append(&mut p);
    ///assert_eq!(pool1.len(), 2);
    ///```
    pub fn append(&self, other: &mut VecDeque<T>) {
        self.pool.lock().unwrap().append(other);
    }

    //Execute all install task.
    //# Error
    //Return Error when install fail 5 times
    pub fn install(self) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        let bar = ProgressBar::new(self.len() as u64);
        bar.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        let mut handles = vec![];
        for _ in 0..MAX_THREAD {
            let tasks_share = self.clone();
            let bar_share = bar.clone();
            let tx_share = tx.clone();
            let thr = thread::spawn(move || loop {
                if let Some(task) = tasks_share.pop_back() {
                    tx_share.send(task.install(&bar_share)).unwrap();
                } else {
                    return;
                }
            });
            handles.push(thr);
        }
        drop(tx);
        for received in rx {
            received?;
        }
        for handle in handles {
            handle.join().unwrap();
        }
        Ok(())
    }
}

pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    let manifest = VersionManifest::fetch(&config.mirror.version_manifest)?;
    let version = Version::fetch(
        manifest,
        &config.game_version,
        &config.mirror.version_manifest,
    )?;
    let version_json_file = Path::new(&config.game_dir)
        .join("versions")
        .join(&config.game_version)
        .join(config.game_version.clone() + ".json");
    version.install(&version_json_file);
    let native_dir = Path::new(&config.game_dir).join("natives");
    fs::create_dir_all(native_dir).unwrap_or(());

    let tasks = TaskPool::new();

    let game_dir = &config.game_dir;
    let game_version = &config.game_version;
    let asset_index_file = Path::new(game_dir)
        .join("assets")
        .join("indexes")
        .join(version.asset_index.id.clone() + ".json");
    let assets = Assets::fetch(&version.asset_index, &config.mirror.version_manifest)?;
    assets.install(&asset_index_file);

    tasks.append(&mut assets_installtask(
        game_dir,
        &config.mirror.assets,
        &assets,
    ));
    tasks.append(&mut libraries_installtask(
        game_dir,
        &config.mirror.libraries,
        &version,
    )?);
    tasks.push_back(client_installtask(
        game_dir,
        game_version,
        &config.mirror.client,
        &version,
    )?);
    tasks.install()?;

    Ok(())
}

fn libraries_installtask(
    game_dir: &str,
    libraries_mirror: &str,
    version_json: &Version,
) -> anyhow::Result<VecDeque<InstallTask>> {
    let libraries = &version_json.libraries;
    Ok(libraries
        .iter()
        .filter(|obj| obj.is_target_lib())
        .map(|x| {
            let artifact_path = x.downloads.artifact.path.clone();
            InstallTask {
                url: libraries_mirror.to_owned() + &artifact_path,
                sha1: x.downloads.artifact.sha1.clone(),
                save_file: Path::new(game_dir).join("libraries").join(&artifact_path),
                r#type: InstallType::Library,
            }
        })
        .collect())
}

fn client_installtask(
    game_dir: &str,
    game_version: &str,
    client_mirror: &str,
    version_json: &Version,
) -> anyhow::Result<InstallTask> {
    let json_client = &version_json.downloads["client"];
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

fn assets_installtask(
    game_dir: &str,
    assets_mirror: &str,
    asset_json: &Assets,
) -> VecDeque<InstallTask> {
    asset_json
        .objects
        .clone()
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
