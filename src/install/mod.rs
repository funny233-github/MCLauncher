use crate::config::{MCLoader, RuntimeConfig};
use installer::{InstallTask, TaskPool};
use mc_api::{
    fabric::{Loader, Profile},
    official::{Artifact, Assets, Version, VersionManifest},
};
use regex::Regex;
use std::{
    borrow::Cow,
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

trait DomainReplacer<T> {
    fn replace_domain(&self, domain: &str) -> T;
}

impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum InstallType {
    #[default]
    Asset,
    Library,
    Client,
    Mods,
}

pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    let version = fetch_version(config)?;

    let version_json_file = Path::new(&config.game_dir)
        .join("versions")
        .join(&config.game_version)
        .join(config.game_version.clone() + ".json");
    version.install(&version_json_file);

    let native_dir = Path::new(&config.game_dir).join("natives");
    fs::create_dir_all(native_dir).unwrap_or(());

    install_dependencies(config, &version)?;
    Ok(())
}

fn fetch_version(config: &RuntimeConfig) -> anyhow::Result<Version> {
    println!("fetching version manifest...");
    let manifest = VersionManifest::fetch(&config.mirror.version_manifest)?;

    if !manifest
        .versions
        .iter()
        .any(|x| x.id == config.game_version)
    {
        return Err(anyhow::anyhow!(
            "Cant' find the minecraft version {}",
            &config.game_version
        ));
    }

    println!("fetching version...");
    let mut version = Version::fetch(
        manifest,
        &config.game_version,
        &config.mirror.version_manifest,
    )?;
    if let MCLoader::Fabric(v) = &config.loader {
        println!("fetching fabric loaders version...");
        let loaders = Loader::fetch(&config.mirror.fabric_meta)?;
        if !loaders.iter().any(|x| &x.version == v) {
            return Err(anyhow::anyhow!("Cant' find the loader version {}", v));
        }
        println!("fetching fabric profile...");
        let game_version = Cow::from(&config.game_version);
        let loader_version = Cow::from(v);
        let profile = Profile::fetch(&config.mirror.fabric_meta, game_version, loader_version)?;
        version.merge(profile)
    }
    Ok(version)
}

fn install_dependencies(config: &RuntimeConfig, version: &Version) -> anyhow::Result<()> {
    let asset_index_file = Path::new(&config.game_dir)
        .join("assets")
        .join("indexes")
        .join(version.asset_index.id.clone() + ".json");
    println!("fetching assets/libraries/natives...");
    let assets = Assets::fetch(&version.asset_index, &config.mirror.version_manifest)?;
    assets.install(&asset_index_file);
    let mut tasks = assets_installtask(&config.game_dir, &config.mirror.assets, &assets);
    tasks.append(&mut libraries_installtask(
        &config.game_dir,
        &config.mirror.libraries,
        &config.mirror.fabric_maven,
        version,
    )?);
    tasks.push_back(client_installtask(
        &config.game_dir,
        &config.game_version,
        &config.mirror.client,
        version,
    )?);
    tasks.append(&mut native_installtask(
        &config.game_dir,
        &config.mirror.libraries,
        version,
    )?);
    TaskPool::from(tasks).install()?;
    println!("extracting natives ...");
    native_extract(&config.game_dir, version);
    Ok(())
}

fn libraries_installtask(
    game_dir: &str,
    libraries_mirror: &str,
    fabric_maven_mirror: &str,
    version_json: &Version,
) -> anyhow::Result<VecDeque<InstallTask>> {
    let libraries = &version_json.libraries;
    Ok(libraries
        .iter()
        .filter(|obj| obj.is_target_lib())
        .map(|x| {
            let artifact = &x.downloads.artifact;
            let path = &artifact.path;
            let mirror = if artifact.url == "https://maven.fabricmc.net/" {
                fabric_maven_mirror
            } else {
                libraries_mirror
            };
            let save_file = Path::new(game_dir).join("libraries").join(path);
            InstallTask {
                url: mirror.to_owned() + path,
                sha1: x.downloads.artifact.sha1.clone(),
                message: format!("library {:?} installed", save_file.file_name().unwrap()),
                save_file,
            }
        })
        .collect())
}

#[test]
fn test_libraries_installtask() {
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let libraries_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let fabric_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let version_json = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    let tasks =
        libraries_installtask(game_dir, libraries_mirror, fabric_mirror, &version_json).unwrap();
    assert!(tasks.len() > 0);
}

fn native_installtask(
    game_dir: &str,
    mirror: &str,
    version_json: &Version,
) -> anyhow::Result<VecDeque<InstallTask>> {
    let libraries = &version_json.libraries;
    Ok(libraries
        .iter()
        .filter(|obj| obj.is_target_native())
        .map(|x| {
            let key = x.natives.as_ref().unwrap().get(OS).unwrap();
            let artifact: &Artifact = x.downloads.classifiers.as_ref().unwrap().get(key).unwrap();
            let path = &artifact.path;
            let save_file = Path::new(game_dir).join("libraries").join(path);
            InstallTask {
                url: mirror.to_owned() + path,
                sha1: artifact.sha1.clone(),
                message: format!("library {:?} installed", save_file.file_name().unwrap()),
                save_file,
            }
        })
        .collect())
}

#[test]
fn test_native_installtask() {
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let libraries_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let version_json = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    let tasks = native_installtask(game_dir, libraries_mirror, &version_json).unwrap();
    assert!(tasks.len() > 0);
}

fn native_extract(game_dir: &str, version_json: &Version) {
    let libraries = &version_json.libraries;
    for lib in libraries {
        if lib.is_target_native() {
            let key = lib.natives.as_ref().unwrap().get(OS).unwrap();
            let artifact: &Artifact = lib
                .downloads
                .classifiers
                .as_ref()
                .unwrap()
                .get(key)
                .unwrap();
            let file_path = Path::new(game_dir).join("libraries").join(&artifact.path);
            extract(game_dir, file_path);
        }
    }
}

fn extract(game_dir: &str, path: PathBuf) {
    let jar_file = fs::File::open(path).unwrap();
    let mut zip = ZipArchive::new(jar_file).unwrap();
    let regex = Regex::new(r"\S+.so$").unwrap();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        if !entry.is_dir() && regex.captures(entry.name()).is_some() {
            let file_path = format!("{}natives/{}", game_dir, entry.name());
            let file_path = Path::new(&file_path);
            fs::create_dir_all(file_path.parent().unwrap()).unwrap();
            let mut output = fs::File::create(file_path).unwrap();
            std::io::copy(&mut entry, &mut output).unwrap();
        }
    }
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
        sha1: Some(json_client["sha1"].as_str().unwrap().to_string()),
        save_file: Path::new(game_dir)
            .join("versions")
            .join(game_version)
            .join(game_version.to_owned() + ".jar"),
        message: "client installed".to_string(),
    })
}

#[test]
fn test_client_installtask() {
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let game_version = "1.16.5";
    let client_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    let task = client_installtask(game_dir, game_version, client_mirror, &version_json);
    assert!(task.is_ok());
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
        .map(|x| {
            let sha1 = Some(x.1.hash.clone());
            InstallTask {
                url: assets_mirror.to_owned() + &x.1.hash[0..2] + "/" + &x.1.hash,
                save_file: Path::new(game_dir)
                    .join("assets")
                    .join("objects")
                    .join(&x.1.hash[0..2])
                    .join(x.1.hash.clone()),
                message: format!("Asset {} installed", sha1.as_ref().unwrap()),
                sha1,
            }
        })
        .collect()
}

#[test]
fn test_assets_installtask() {
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let assets_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = Version::fetch(manifest, "1.16.5", manifest_mirror).unwrap();
    let assets_json = Assets::fetch(&version_json.asset_index, assets_mirror).unwrap();
    let task = assets_installtask(game_dir, assets_mirror, &assets_json);
    assert!(task.len() > 0);
}
