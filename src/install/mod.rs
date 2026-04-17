//! Minecraft game files and libraries installation module.
//!
//! Downloads and installs Minecraft game files including version manifests,
//! libraries, assets, native libraries, and optional mod loaders.
//!
//! # Supported Loaders
//!
//! | Loader | Installer | Description |
//! |--------|-----------|-------------|
//! | Vanilla | [`VanillaInstaller`] | Unmodded Minecraft |
//! | Fabric | [`FabricInstaller`] | Fabric mod loader |
//! | `NeoForge` | [`NeoforgeInstaller`] | `NeoForge` mod loader with installer processors |
//!
//! # Installation Workflow
//!
//! 1. Determine the loader type from the runtime configuration
//! 2. Fetch and merge the loader profile with the base Minecraft version JSON
//! 3. Download all dependencies (assets, libraries, client JAR, natives)
//! 4. Execute loader-specific post-install steps (e.g., `NeoForge` processors)
//!
//! # Example
//! ```no_run
//! use gluon::install::install_mc;
//! use gluon::config::RuntimeConfig;
//! use gluon::config::MCLoader;
//! use gluon::config::MCMirror;
//!
//! let config = RuntimeConfig {
//!     max_memory_size: 1000000,
//!     window_weight: 100,
//!     window_height: 100,
//!     game_dir: "/path/to/game".to_string(),
//!     game_version: "1.21.1".to_string(),
//!     java_path: "/path/to/java".to_string(),
//!     vanilla: "1.21.1".to_string(),
//!     loader: MCLoader::None,
//!     mirror: MCMirror::official_mirror(),
//!     mods: None,
//! };
//!
//! install_mc(&config).expect("Installation failed");
//! ```

use crate::config::{ConfigHandler, MCLoader};
use installer::{InstallTask, TaskPool};
use mc_api::official::{Artifact, Assets, Version};
use regex::Regex;
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

mod fabric;
mod mavencoord;
mod mc_installer;
mod neoforge;
mod vanilla;

use fabric::FabricInstaller;
use mc_installer::MCInstaller;
use neoforge::NeoforgeInstaller;
use vanilla::VanillaInstaller;

/// Operating system identifier set at compile time.
#[cfg(target_os = "windows")]
const OS: &str = "windows";

/// Operating system identifier set at compile time.
#[cfg(target_os = "linux")]
const OS: &str = "linux";

/// Operating system identifier set at compile time.
#[cfg(target_os = "macos")]
const OS: &str = "osx";

/// Trait for replacing download domains in URLs.
///
/// Supports alternative download mirrors by replacing the base domain in URLs
/// while preserving the path structure.
trait DomainReplacer<T> {
    /// Replaces the domain part of the URL with a new domain.
    ///
    /// # Errors
    /// - `anyhow::Error` if URL doesn't match the expected pattern
    fn replace_domain(&self, domain: &str) -> anyhow::Result<T>;
}

impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> anyhow::Result<String> {
        let regex = Regex::new(r"(?<replace>https://\S+?/)")?;
        let replace = regex
            .captures(self.as_str())
            .ok_or_else(|| anyhow::anyhow!("Cant' find the replace string"))?;
        Ok(self.replace(&replace["replace"], domain))
    }
}

/// Represents the type of installation task.
///
/// Used to categorize different types of Minecraft game files during
/// the installation process.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum InstallType {
    /// Game assets (sounds, textures, models, etc.)
    #[default]
    Asset,
    /// Java libraries and dependencies
    Library,
    /// Minecraft client JAR file
    Client,
    /// Mod files (not currently used in core installation)
    Mods,
}

/// Installs Minecraft game files and libraries.
///
/// Handles the complete installation process including downloading version manifests,
/// libraries, assets, client JAR, and native libraries. Supports vanilla, Fabric,
/// and `NeoForge` mod loaders.
///
/// # Example
/// ```no_run
/// use gluon::install::install_mc;
/// use gluon::config::RuntimeConfig;
/// use gluon::config::MCMirror;
/// use gluon::config::MCLoader;
///
/// let config = RuntimeConfig {
///     max_memory_size: 1000,
///     window_weight: 100,
///     window_height: 100,
///     game_dir: "/path/to/game".to_string(),
///     game_version: "1.21.1".to_string(),
///     java_path: "/path/to/java".to_string(),
///     vanilla: "1.21.1".to_string(),
///     loader: MCLoader::None,
///     mirror: MCMirror::official_mirror(),
///     mods: None,
/// };
///
/// if let Err(e) = install_mc(&config) {
///     eprintln!("Installation failed: {}", e);
/// }
/// ```
///
/// # Errors
/// - `anyhow::Error` if version manifest cannot be downloaded or parsed
/// - `anyhow::Error` if the specified Minecraft version is not found
/// - `anyhow::Error` if libraries cannot be downloaded
/// - `anyhow::Error` if assets cannot be downloaded
/// - `anyhow::Error` if native libraries cannot be extracted
/// - `anyhow::Error` if file system operations fail
/// - `anyhow::Error` if network errors occur during download
pub fn install_mc(config: &ConfigHandler) -> anyhow::Result<()> {
    match config.config().loader {
        MCLoader::None => VanillaInstaller::install(config)?,
        MCLoader::Fabric(_) => FabricInstaller::install(config)?,
        MCLoader::Neoforge(_) => NeoforgeInstaller::install(config)?,
    }
    Ok(())
}

/// Installs all game dependencies for a specific version.
///
/// Orchestrates the installation of all required game files: assets, libraries,
/// client JAR, and native libraries. Creates download tasks for each file type
/// and executes them concurrently.
///
/// # Errors
/// - `anyhow::Error` if asset index cannot be fetched
/// - `anyhow::Error` if download task creation fails
/// - `anyhow::Error` if download execution fails
/// - `anyhow::Error` if native library extraction fails
fn install_dependencies(config: &ConfigHandler, version: &Version) -> anyhow::Result<()> {
    let game_dir = config.get_absolute_game_dir()?;
    let asset_index_file = Path::new(&game_dir)
        .join("assets")
        .join("indexes")
        .join(version.asset_index.id.clone() + ".json");
    println!("fetching assets/libraries/natives...");
    let assets = Assets::fetch(
        &version.asset_index,
        &config.config().mirror.version_manifest,
    )?;
    assets.install(&asset_index_file);
    let mut tasks = assets_installtask(&game_dir, &config.config().mirror.assets, &assets)?;
    tasks.append(&mut libraries_installtask(
        &game_dir,
        &config.config().mirror.libraries,
        &config.config().mirror.fabric_maven,
        version,
    )?);
    tasks.push_back(client_installtask(
        &game_dir,
        &config.config().game_version,
        &config.config().mirror.client,
        version,
    )?);
    tasks.append(&mut native_installtask(
        &game_dir,
        &config.config().mirror.libraries,
        version,
    )?);
    TaskPool::from(tasks).install();
    println!("extracting natives ...");
    native_extract(&game_dir, version)?;
    Ok(())
}

/// Creates download tasks for Java library dependencies.
///
/// Filters the version's library list to include only libraries that are
/// compatible with the current platform and OS, then creates download tasks
/// for each library. Fabric libraries are downloaded from the Fabric Maven mirror.
///
/// # Errors
/// - `anyhow::Error` if any library's path cannot be constructed or extracted
fn libraries_installtask(
    game_dir: &str,
    libraries_mirror: &str,
    fabric_maven_mirror: &str,
    version_json: &Version,
) -> anyhow::Result<VecDeque<InstallTask>> {
    let libraries = &version_json.libraries;
    libraries
        .iter()
        .filter(|obj| obj.is_target_lib())
        .map(|x| {
            let artifact = &x.downloads.artifact;
            let path = &artifact.path;
            let fabric_domain = "https://maven.fabricmc.net/";
            let vanilla_domain = "https://libraries.minecraft.net";
            let url = if artifact.url.starts_with(vanilla_domain) {
                libraries_mirror.to_string() + path
            } else if artifact.url.starts_with(fabric_domain) {
                fabric_maven_mirror.to_string() + path
            } else {
                artifact.url.clone()
            };

            let save_file = Path::new(game_dir).join("libraries").join(path);
            Ok(InstallTask {
                url,
                sha1: x.downloads.artifact.sha1.clone(),
                message: format!(
                    "library {} installed",
                    save_file
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("take file name failed"))?
                        .display()
                ),
                save_file,
            })
        })
        .collect()
}

/// Verifies that library install tasks are correctly generated for a
/// specific Minecraft version.
#[test]
fn test_libraries_installtask() {
    use mc_api::official::VersionManifest;
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let libraries_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let fabric_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let version_json = Version::fetch(&manifest, "1.16.5", manifest_mirror).unwrap();
    let tasks =
        libraries_installtask(game_dir, libraries_mirror, fabric_mirror, &version_json).unwrap();
    assert!(!tasks.is_empty());
}

/// Creates download tasks for native libraries.
///
/// Native libraries are platform-specific JAR files containing compiled
/// code (e.g., .so files on Linux, .dll files on Windows, .dylib on macOS).
/// These need to be downloaded and then extracted. Uses the `OS` constant
/// which is set at compile time based on the target platform.
///
/// # Errors
/// - `anyhow::Error` if no native classifiers are defined for the library
/// - `anyhow::Error` if the current OS is not supported by the library
/// - `anyhow::Error` if the path cannot be constructed
fn native_installtask(
    game_dir: &str,
    mirror: &str,
    version_json: &Version,
) -> anyhow::Result<VecDeque<InstallTask>> {
    let libraries = &version_json.libraries;
    libraries
        .iter()
        .filter(|obj| obj.is_target_native())
        .map(|x| {
            let key = x
                .natives
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("take natives failed"))?
                .get(OS)
                .ok_or_else(|| {
                    anyhow::anyhow!("take {OS} natives failed, there is no natives for this os")
                })?;
            let artifact: &Artifact = x
                .downloads
                .classifiers
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("take classifiers failed"))?
                .get(key)
                .ok_or_else(|| anyhow::anyhow!("take {key} natives failed"))?;
            let path = &artifact.path;
            let save_file = Path::new(game_dir).join("libraries").join(path);
            Ok(InstallTask {
                url: mirror.to_owned() + path,
                sha1: artifact.sha1.clone(),
                message: format!(
                    "library {} installed",
                    save_file
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("take file name failed"))?
                        .display()
                ),
                save_file,
            })
        })
        .collect()
}

/// Verifies that native library install tasks are correctly generated for
/// the current platform.
#[test]
fn test_native_installtask() {
    use mc_api::official::VersionManifest;
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let libraries_mirror = "https://bmclapi2.bangbang93.com/maven/";
    let version_json = Version::fetch(&manifest, "1.16.5", manifest_mirror).unwrap();
    let tasks = native_installtask(game_dir, libraries_mirror, &version_json).unwrap();
    assert!(!tasks.is_empty());
}

/// Extracts native libraries from downloaded JAR files.
///
/// Extracts platform-specific native libraries (e.g., .so files)
/// from their JAR containers and places them in the game's natives directory.
///
/// # Errors
/// - `anyhow::Error` if no native classifiers are defined for a library
/// - `anyhow::Error` if the current OS is not supported
/// - `anyhow::Error` if the JAR file cannot be opened
/// - `anyhow::Error` if file extraction fails
fn native_extract(game_dir: &str, version_json: &Version) -> anyhow::Result<()> {
    let libraries = &version_json.libraries;
    libraries
        .iter()
        .filter(|lib| lib.is_target_native())
        .try_for_each(|lib| {
            let key = lib
                .natives
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("take natives failed"))?
                .get(OS)
                .ok_or_else(|| anyhow::anyhow!("take {OS} natives failed"))?;
            let artifact: &Artifact = lib
                .downloads
                .classifiers
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("take classifiers failed"))?
                .get(key)
                .ok_or_else(|| anyhow::anyhow!("take {key} natives failed"))?;
            let file_path = Path::new(game_dir).join("libraries").join(&artifact.path);
            extract(game_dir, file_path)?;
            Ok(())
        })
}

/// Extracts native files from a JAR archive.
///
/// Extracts platform-specific native files (e.g., .so files on Linux) from a
/// JAR file to the game's natives directory. The regex pattern currently only
/// matches .so files (Linux). This should be made platform-aware for Windows (.dll)
/// and macOS (.dylib) support.
///
/// # Errors
/// - `anyhow::Error` if the JAR file cannot be opened
/// - `anyhow::Error` if a directory path cannot be extracted
/// - `anyhow::Error` if file creation fails
fn extract(game_dir: &str, path: PathBuf) -> anyhow::Result<()> {
    let jar_file = fs::File::open(path)?;
    let mut zip = ZipArchive::new(jar_file)?;
    let regex = Regex::new(r"\S+.so$")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if !entry.is_dir() && regex.captures(entry.name()).is_some() {
            let file_path = format!("{}natives/{}", game_dir, entry.name());
            let file_path = Path::new(&file_path);
            fs::create_dir_all(
                file_path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("take parent failed"))?,
            )?;
            let mut output = fs::File::create(file_path)?;
            std::io::copy(&mut entry, &mut output)?;
        }
    }
    Ok(())
}

/// Creates a download task for the Minecraft client JAR.
///
/// The client JAR is the main executable file for Minecraft. Constructs the
/// download task including URL replacement for mirror support. The client JAR is
/// saved to: `{game_dir}/versions/{game_version}/{game_version}.jar`
///
/// # Errors
/// - `anyhow::Error` if the client URL cannot be extracted from version metadata
/// - `anyhow::Error` if domain replacement fails
/// - `anyhow::Error` if the SHA-1 checksum cannot be extracted
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
            .map(|str| str.to_string().replace_domain(client_mirror))
            .ok_or_else(|| anyhow::anyhow!("take url failed"))??,
        sha1: Some(
            json_client["sha1"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("take sha1 failed"))?
                .to_string(),
        ),
        save_file: Path::new(game_dir)
            .join("versions")
            .join(game_version)
            .join(game_version.to_owned() + ".jar"),
        message: "client installed".to_string(),
    })
}

/// Verifies that the client install task is correctly generated with proper
/// URL replacement and file path construction.
#[test]
fn test_client_installtask() {
    use mc_api::official::VersionManifest;
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let game_version = "1.16.5";
    let client_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = Version::fetch(&manifest, "1.16.5", manifest_mirror).unwrap();
    let task = client_installtask(game_dir, game_version, client_mirror, &version_json);
    assert!(task.is_ok());
}

/// Creates download tasks for all game assets.
///
/// Assets include textures, sounds, models, and other game resources.
/// Creates a download task for each asset file based on its SHA-1 hash.
/// Assets are stored in a content-addressable structure based on their SHA-1 hash:
/// `{game_dir}/assets/objects/{first_two_chars_of_hash}/{full_hash}`.
///
/// # Errors
/// - `anyhow::Error` if an asset's SHA-1 hash cannot be extracted
fn assets_installtask(
    game_dir: &str,
    assets_mirror: &str,
    asset_json: &Assets,
) -> anyhow::Result<VecDeque<InstallTask>> {
    asset_json
        .objects
        .clone()
        .into_iter()
        .map(|x| {
            let sha1 = Some(x.1.hash.clone());
            Ok(InstallTask {
                url: assets_mirror.to_owned() + &x.1.hash[0..2] + "/" + &x.1.hash,
                save_file: Path::new(game_dir)
                    .join("assets")
                    .join("objects")
                    .join(&x.1.hash[0..2])
                    .join(x.1.hash.clone()),
                message: format!(
                    "Asset {} installed",
                    sha1.as_ref()
                        .ok_or_else(|| anyhow::anyhow!("take sha1 failed"))?
                ),
                sha1,
            })
        })
        .collect()
}

/// Verifies that asset install tasks are correctly generated for all
/// assets in the version's asset index.
#[test]
fn test_assets_installtask() {
    use mc_api::official::VersionManifest;
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let assets_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = Version::fetch(&manifest, "1.16.5", manifest_mirror).unwrap();
    let assets_json = Assets::fetch(&version_json.asset_index, assets_mirror).unwrap();
    let task = assets_installtask(game_dir, assets_mirror, &assets_json);
    assert!(!task.unwrap().is_empty());
}
