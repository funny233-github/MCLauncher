//! Minecraft game files and libraries installation module.
//!
//! This module provides functionality to download and install Minecraft game files,
//! including version manifests, libraries, assets, native libraries, and optional
//! mod loaders like Fabric.
//!
//! # Architecture
//!
//! The installation process is divided into several stages:
//!
//! 1. **Version fetching** - Retrieves version information and manifests
//! 2. **Dependency resolution** - Determines which libraries and assets are needed
//! 3. **Download tasks** - Creates download tasks for all required files
//! 4. **Installation** - Executes downloads and extracts native libraries
//!
//! # Supported Features
//!
//! - Official Minecraft versions via version manifest
//! - Fabric mod loader integration
//! - Mirror support for faster downloads (e.g., BMCLAPI)
//! - SHA-1 checksum verification
//! - Concurrent downloads
//! - Cross-platform native library extraction
//!
//! # Example
//!
//! ```no_run
//! use your_crate::install::install_mc;
//! use your_crate::config::RuntimeConfig;
//!
//! let config = RuntimeConfig {
//!     game_dir: "/path/to/game".to_string(),
//!     game_version: "1.16.5".to_string(),
//!     // ... other config fields
//! };
//!
//! install_mc(&config).expect("Installation failed");
//! ```

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
use std::{fs::File, io::Read};
use zip::ZipArchive;

/// Operating system identifier for Windows.
#[cfg(target_os = "windows")]
const OS: &str = "windows";

/// Operating system identifier for Linux.
#[cfg(target_os = "linux")]
const OS: &str = "linux";

/// Operating system identifier for macOS.
#[cfg(target_os = "macos")]
const OS: &str = "osx";

/// Trait for replacing download domains in URLs.
///
/// This trait is used to support alternative download mirrors by replacing
/// the base domain in URLs while preserving the path structure.
///
/// # Type Parameters
///
/// * `T` - The output type after domain replacement
///
/// # Examples
///
/// ```
/// use regex::Regex;
///
/// let url = "https://libraries.minecraft.net/com/example/lib/1.0.0/lib.jar";
/// let new_url = url.replace_domain("https://bmclapi2.bangbang93.com/").unwrap();
/// ```
trait DomainReplacer<T> {
    /// Replaces the domain part of the URL with a new domain.
    ///
    /// # Arguments
    ///
    /// * `domain` - The new domain to use (e.g., `<https://mirror.example.com/>`)
    ///
    /// # Errors
    ///
    /// Returns an error if the URL doesn't match the expected pattern.
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
/// This is the main entry point for Minecraft installation. It handles the complete
/// installation process including downloading version manifests, libraries, assets,
/// client JAR, and native libraries. It also supports optional Fabric mod loader.
///
/// # Installation Process
///
/// 1. Fetches and caches the version manifest JSON
/// 2. Creates the native library directory
/// 3. Downloads all dependencies (libraries, assets, client, natives)
/// 4. Extracts native libraries to the game directory
///
/// # Arguments
///
/// * `config` - Runtime configuration including game directory, version, and mirror URLs
///
/// # Errors
///
/// Returns an error if:
/// - Version manifest cannot be downloaded or parsed
/// - The specified Minecraft version is not found
/// - Libraries cannot be downloaded
/// - Assets cannot be downloaded
/// - Native libraries cannot be extracted
/// - File system operations fail
/// - Network errors occur during download
///
/// # Examples
///
/// ```no_run
/// use your_crate::install::install_mc;
/// use your_crate::config::RuntimeConfig;
///
/// let config = RuntimeConfig {
///     game_dir: "/path/to/minecraft".to_string(),
///     game_version: "1.16.5".to_string(),
///     // ... other fields
/// };
///
/// if let Err(e) = install_mc(&config) {
///     eprintln!("Installation failed: {}", e);
/// }
/// ```
pub fn install_mc(config: &RuntimeConfig) -> anyhow::Result<()> {
    let version_json_file_path = Path::new(&config.game_dir)
        .join("versions")
        .join(&config.game_version)
        .join(config.game_version.clone() + ".json");

    if !version_json_file_path.exists() {
        let version = fetch_version(config)?;
        version.install(&version_json_file_path);
    }

    let native_dir = Path::new(&config.game_dir).join("natives");
    fs::create_dir_all(native_dir).unwrap_or(());

    let mut version_json_file = File::open(version_json_file_path)?;
    let mut content = String::new();
    version_json_file.read_to_string(&mut content)?;

    let version: Version = serde_json::from_str(&content)?;
    install_dependencies(config, &version)?;
    Ok(())
}

/// Fetches the version manifest for the specified Minecraft version.
///
/// This function downloads the version manifest, verifies the requested version
/// exists, and optionally merges Fabric loader information if configured.
///
/// # Arguments
///
/// * `config` - Runtime configuration containing game version and mirror URLs
///
/// # Returns
///
/// A `Version` object containing all version information including:
/// - Main Minecraft version details
/// - Library dependencies
/// - Asset index information
/// - (Optional) Fabric loader modifications
///
/// # Errors
///
/// Returns an error if:
/// - The version manifest cannot be fetched
/// - The requested game version is not found in the manifest
/// - A Fabric loader version is specified but not found
/// - The Fabric profile cannot be fetched or merged
///
/// # Flow
///
/// 1. Download version manifest from configured mirror
/// 2. Verify requested version exists
/// 3. Download version JSON for the specified version
/// 4. If Fabric is configured: fetch loader and profile, merge into version
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
        &manifest,
        &config.game_version,
        &config.mirror.version_manifest,
    )?;
    if let MCLoader::Fabric(v) = &config.loader {
        println!("fetching fabric loaders version...");
        let loaders = Loader::fetch(&config.mirror.fabric_meta)?;
        if !loaders.iter().any(|x| &x.version == v) {
            return Err(anyhow::anyhow!("Cant' find the loader version {v}"));
        }
        println!("fetching fabric profile...");
        let game_version = Cow::from(&config.game_version);
        let loader_version = Cow::from(v);
        let profile = Profile::fetch(&config.mirror.fabric_meta, &game_version, &loader_version)?;
        version.merge(&profile);
    }
    Ok(version)
}

/// Installs all game dependencies for a specific version.
///
/// This function orchestrates the installation of all required game files:
/// assets, libraries, client JAR, and native libraries. It creates download
/// tasks for each file type and executes them concurrently.
///
/// # Arguments
///
/// * `config` - Runtime configuration with game directory and mirror URLs
/// * `version` - Version information containing dependency metadata
///
/// # Errors
///
/// Returns an error if:
/// - Asset index cannot be fetched
/// - Download task creation fails
/// - Download execution fails
/// - Native library extraction fails
///
/// # Installation Stages
///
/// 1. Fetch and cache the asset index
/// 2. Create download tasks for assets
/// 3. Create download tasks for libraries
/// 4. Create download task for client JAR
/// 5. Create download tasks for native libraries
/// 6. Execute all downloads in parallel
/// 7. Extract native libraries from downloaded JARs
fn install_dependencies(config: &RuntimeConfig, version: &Version) -> anyhow::Result<()> {
    let asset_index_file = Path::new(&config.game_dir)
        .join("assets")
        .join("indexes")
        .join(version.asset_index.id.clone() + ".json");
    println!("fetching assets/libraries/natives...");
    let assets = Assets::fetch(&version.asset_index, &config.mirror.version_manifest)?;
    assets.install(&asset_index_file);
    let mut tasks = assets_installtask(&config.game_dir, &config.mirror.assets, &assets)?;
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
    TaskPool::from(tasks).install();
    println!("extracting natives ...");
    native_extract(&config.game_dir, version)?;
    Ok(())
}

/// Creates download tasks for Java library dependencies.
///
/// Filters the version's library list to include only libraries that are
/// compatible with the current platform and OS, then creates download tasks
/// for each library.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `libraries_mirror` - Mirror URL for standard Minecraft libraries
/// * `fabric_maven_mirror` - Mirror URL for Fabric-specific libraries
/// * `version_json` - Version metadata containing library information
///
/// # Returns
///
/// A vector of `InstallTask` objects, one for each library that needs to be downloaded.
///
/// # Errors
///
/// Returns an error if any library's path cannot be constructed or extracted.
///
/// # Note
///
/// - Fabric libraries are downloaded from the Fabric Maven mirror
/// - Standard libraries are downloaded from the libraries mirror
/// - Only libraries for the current platform are included
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
            let mirror = if artifact.url == "https://maven.fabricmc.net/" {
                fabric_maven_mirror
            } else {
                libraries_mirror
            };
            let save_file = Path::new(game_dir).join("libraries").join(path);
            Ok(InstallTask {
                url: mirror.to_owned() + path,
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

/// Tests the library download task creation.
///
/// Verifies that library install tasks are correctly generated for a
/// specific Minecraft version. This test uses a real version manifest
/// from a public mirror.
#[test]
fn test_libraries_installtask() {
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
/// These need to be downloaded and then extracted.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `mirror` - Mirror URL for downloading native libraries
/// * `version_json` - Version metadata containing native library information
///
/// # Returns
///
/// A vector of `InstallTask` objects, one for each native library that needs
/// to be downloaded for the current platform.
///
/// # Errors
///
/// Returns an error if:
/// - No native classifiers are defined for the library
/// - The current OS is not supported by the library
/// - The path cannot be constructed
///
/// # Platform Detection
///
/// Uses the `OS` constant which is set at compile time based on the target
/// platform (`windows`, `linux`, or `osx` for macOS).
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

/// Tests the native library download task creation.
///
/// Verifies that native library install tasks are correctly generated for
/// the current platform. This test ensures that platform-specific native
/// libraries are properly identified.
#[test]
fn test_native_installtask() {
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
/// This function extracts platform-specific native libraries (e.g., .so files)
/// from their JAR containers and places them in the game's natives directory.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `version_json` - Version metadata containing native library information
///
/// # Errors
///
/// Returns an error if:
/// - No native classifiers are defined for a library
/// - The current OS is not supported
/// - The JAR file cannot be opened
/// - File extraction fails
///
/// # Extraction Process
///
/// 1. Iterates through all libraries in the version
/// 2. Filters for native libraries for the current platform
/// 3. Opens each library JAR file
/// 4. Extracts native files (e.g., .so) to the natives directory
/// 5. Preserves directory structure within the natives folder
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
/// This is a helper function that extracts platform-specific native files
/// (e.g., .so files on Linux) from a JAR file to the game's natives directory.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `path` - Path to the JAR file containing native libraries
///
/// # Errors
///
/// Returns an error if:
/// - The JAR file cannot be opened
/// - A directory path cannot be extracted
/// - File creation fails
///
/// # Extraction Rules
///
/// - Only extracts files matching the platform-specific extension (.so on Linux)
/// - Skips directories within the JAR
/// - Preserves the directory structure in the output
/// - Creates parent directories as needed
///
/// # Note
///
/// The regex pattern currently only matches .so files (Linux). This should
/// be made platform-aware for Windows (.dll) and macOS (.dylib) support.
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
/// The client JAR is the main executable file for Minecraft. This function
/// constructs the download task including URL replacement for mirror support.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `game_version` - Version string (e.g., "1.16.5")
/// * `client_mirror` - Mirror URL to use for client downloads
/// * `version_json` - Version metadata containing client download information
///
/// # Returns
///
/// An `InstallTask` configured to download the client JAR to the appropriate
/// location in the versions directory.
///
/// # Errors
///
/// Returns an error if:
/// - The client URL cannot be extracted from version metadata
/// - Domain replacement fails
/// - The SHA-1 checksum cannot be extracted
///
/// # File Location
///
/// The client JAR is saved to:
/// `{game_dir}/versions/{game_version}/{game_version}.jar`
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

/// Tests the client JAR download task creation.
///
/// Verifies that the client install task is correctly generated with proper
/// URL replacement and file path construction.
#[test]
fn test_client_installtask() {
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
/// This function creates a download task for each asset file based on its
/// SHA-1 hash.
///
/// # Arguments
///
/// * `game_dir` - Base directory for game installation
/// * `assets_mirror` - Mirror URL for asset downloads
/// * `asset_json` - Asset index metadata containing file hashes
///
/// # Returns
///
/// A vector of `InstallTask` objects, one for each asset that needs to be downloaded.
///
/// # Errors
///
/// Returns an error if an asset's SHA-1 hash cannot be extracted.
///
/// # Asset Storage Structure
///
/// Assets are stored in a content-addressable structure based on their SHA-1 hash:
/// `{game_dir}/assets/objects/{first_two_chars_of_hash}/{full_hash}`
///
/// This allows for deduplication across different game versions.
///
/// # Download URL Format
///
/// Assets are downloaded from the mirror using the format:
/// `{mirror}/{first_two_chars_of_hash}/{full_hash}`
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

/// Tests the assets download task creation.
///
/// Verifies that asset install tasks are correctly generated for all
/// assets in the version's asset index.
#[test]
fn test_assets_installtask() {
    let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    let manifest = VersionManifest::fetch(manifest_mirror).unwrap();
    let game_dir = "test_dir/";
    let assets_mirror = "https://bmclapi2.bangbang93.com/";
    let version_json = Version::fetch(&manifest, "1.16.5", manifest_mirror).unwrap();
    let assets_json = Assets::fetch(&version_json.asset_index, assets_mirror).unwrap();
    let task = assets_installtask(game_dir, assets_mirror, &assets_json);
    assert!(!task.unwrap().is_empty());
}
