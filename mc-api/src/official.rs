//! Minecraft Official API Module
//!
//! This module provides functionality for interacting with Minecraft's official
//! version manifest API and version JSON files. It supports fetching version lists,
//! downloading version metadata, managing game assets, and integrating with mod loaders.
//!
//! # API Overview
//!
//! The official Minecraft API consists of two main components:
//!
//! - **Version Manifest**: A central index of all available Minecraft versions
//! - **Version JSON**: Detailed metadata for each specific version
//!
//! # Version Manifest
//!
//! The version manifest (`version_manifest.json`) provides:
//! - List of all available versions (release and snapshot)
//! - Latest release and snapshot information
//! - URLs to download each version's JSON file
//!
//! # Version JSON
//!
//! Each version's JSON file contains:
//! - Game and JVM launch arguments
//! - Library dependencies with platform rules
//! - Asset index information
//! - Download links for client, server, and assets
//! - Java version requirements
//!
//! # Platform Support
//!
//! The library automatically handles platform-specific library filtering:
//!
//! - **Windows**: Filters for Windows-native libraries
//! - **Linux**: Filters for Linux-native libraries
//! - **macOS**: Filters for macOS-native libraries (detected as "osx")
//!
//! # Usage Example
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version};
//!
//! // Fetch version manifest
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//!
//! // Get latest release
//! let latest_release = &manifest.latest.release;
//! println!("Latest release: {}", latest_release);
//!
//! // Fetch version details
//! let version = Version::fetch(&manifest, latest_release, manifest_mirror)?;
//! println!("Main class: {}", version.main_class);
//! println!("Libraries: {}", version.libraries.len());
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Mirror Support
//!
//! The library supports mirror servers for improved download speed and reliability:
//!
//! - **Official**: `https://launchermeta.mojang.com/`
//! - **BMCLAPI**: `https://bmclapi2.bangbang93.com/`
//! - **Other mirrors**: Any compatible Minecraft API mirror
//!
//! # Asset Management
//!
//! Assets (sounds, textures, etc.) are managed through the asset index system:
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version, Assets};
//! use std::path::PathBuf;
//!
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let assets_mirror = "https://bmclapi2.bangbang93.com/";
//!
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//! let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//!
//! // Fetch asset index
//! let assets = Assets::fetch(&version.asset_index, assets_mirror)?;
//!
//! // Install assets.json
//! let asset_path = PathBuf::from("./assets/indexes/1.20.4.json");
//! assets.install(&asset_path);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Version Merging
//!
//! Integrate mod loader profiles with official versions:
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version, MergeVersion};
//!
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//!
//! let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//! # Ok::<(), anyhow::Error>(())
//!

use super::DomainReplacer;
use crate::fetcher::FetcherBuilder;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

/// Represents the type of Minecraft version used for filtering from the version manifest.
///
/// # Example
/// ```no_run
/// use mc_api::official::{VersionManifest, VersionType};
///
/// let mirror = "https://bmclapi2.bangbang93.com/";
/// let manifest = VersionManifest::fetch(mirror)?;
///
/// let all = manifest.list(&VersionType::All);
/// let releases = manifest.list(&VersionType::Release);
/// let snapshots = manifest.list(&VersionType::Snapshot);
///
/// println!("Total versions: {}", all.len());
/// println!("Releases: {}", releases.len());
/// println!("Snapshots: {}", snapshots.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug)]
pub enum VersionType {
    All,
    Release,
    Snapshot,
}

/// Contains download information for a library file.
///
/// The path follows the standard Maven directory structure:
/// `groupId/path/artifactId/version/artifactId-version.jar`
///
/// # Example
/// ```
/// use mc_api::official::Artifact;
///
/// let artifact = Artifact {
///     path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar".to_string(),
///     sha1: Some("abc123...".to_string()),
///     size: Some(1234567),
///     url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar".to_string(),
/// };
/// ```
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Artifact {
    /// Relative storage path following Maven structure.
    pub path: String,
    /// SHA1 hash for integrity verification.
    pub sha1: Option<String>,
    /// File size in bytes.
    pub size: Option<i32>,
    /// Download URL for the library.
    pub url: String,
}

/// Contains download information for a library including optional platform-specific classifiers.
///
/// Classifiers are used for platform-specific library variants:
/// - `natives-windows` - Windows native libraries
/// - `natives-linux` - Linux native libraries
/// - `natives-osx` - macOS native libraries
///
/// # Example
/// ```
/// use mc_api::official::{LibDownloads, Artifact};
///
/// let downloads = LibDownloads {
///     artifact: Artifact {
///         path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar".to_string(),
///         sha1: Some("abc123...".to_string()),
///         size: Some(1234567),
///         url: "https://example.com/lwjgl.jar".to_string(),
///     },
///     classifiers: None,
/// };
/// ```
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct LibDownloads {
    /// Main library artifact.
    pub artifact: Artifact,
    /// Map of platform-specific artifacts (e.g., natives).
    pub classifiers: Option<HashMap<String, Artifact>>,
}

/// Determines when a library should be included based on OS or other conditions.
///
/// Rules are evaluated in order: if action is "allow" and OS matches, the library
/// is included; if action is "disallow" and OS matches, the library is excluded.
/// If no rules match, the default behavior is to include the library.
///
/// # Example
/// ```
/// use mc_api::official::Rules;
/// use std::collections::HashMap;
///
/// let rule = Rules {
///     action: "allow".to_string(),
///     os: Some({
///         let mut map = HashMap::new();
///         map.insert("name".to_string(), "windows".to_string());
///         map
///     }),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rules {
    /// Either "allow" or "disallow".
    pub action: String,
    /// Optional operating system filter.
    pub os: Option<HashMap<String, String>>,
}

/// Contains comprehensive information about a library dependency including downloads, platform variants, and rules.
///
/// Libraries can be filtered for the current platform using:
/// - `is_target_lib()` - Checks if library should be included for current OS
/// - `is_target_native()` - Checks if library is a native library for current OS
///
/// # Example
/// ```
/// use mc_api::official::Library;
///
/// let library = Library::default();
///
/// if library.is_target_lib() {
///     println!("Library {} is needed", library.name);
/// }
///
/// if library.is_target_native() {
///     println!("Library {} is a native library", library.name);
/// }
/// ```
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Library {
    /// Download information for the library.
    pub downloads: LibDownloads,
    /// Maven coordinate name (groupId:artifactId:version).
    pub name: String,
    /// Map of platform-specific library names.
    pub natives: Option<HashMap<String, String>>,
    /// List of inclusion rules.
    pub rules: Option<Vec<Rules>>,
}

impl Library {
    /// Determines if this library should be included for the current platform.
    ///
    /// Evaluates the library's rules: if no rules exist, the library is included
    /// (no classifiers). If rules exist, finds a rule that applies to the current OS.
    /// The library is included if a matching rule exists and has no classifiers.
    ///
    /// Platform is detected at compile time:
    /// - Windows → `OS` = `"windows"`
    /// - Linux → `OS` = `"linux"`
    /// - macOS → `OS` = `"osx"`
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// let target_libs: Vec<_> = version.libraries.iter()
    ///     .filter(|lib| lib.is_target_lib())
    ///     .map(|lib| lib.name.clone())
    ///     .collect();
    ///
    /// println!("Libraries needed: {}", target_libs.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Panics
    /// Panics if a library rule has invalid OS configuration.
    #[must_use]
    pub fn is_target_lib(&self) -> bool {
        if let Some(rule) = &self.rules {
            let is_for_current_os = rule
                .iter()
                .find(|x| x.os.is_none() || x.os.as_ref().map(|x| x["name"] == OS).unwrap());
            self.downloads.classifiers.is_none() && is_for_current_os.is_some()
        } else {
            self.downloads.classifiers.is_none()
        }
    }

    /// Determines if this library contains a native variant for the current platform.
    ///
    /// Native libraries contain compiled code specific to an operating system:
    /// - Windows DLLs (`.dll` files)
    /// - Linux shared objects (`.so` files)
    /// - macOS dynamic libraries (`.dylib` files)
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// let native_libs: Vec<_> = version.libraries.iter()
    ///     .filter(|lib| lib.is_target_native())
    ///     .map(|lib| lib.name.clone())
    ///     .collect();
    ///
    /// println!("Native libraries needed: {}", native_libs.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    #[must_use]
    pub fn is_target_native(&self) -> bool {
        self.natives.as_ref().and_then(|x| x.get(OS)).is_some()
    }
}

/// Type alias for a vector of libraries.
///
/// This alias is used throughout the library to represent collections
/// of library dependencies.
pub type Libraries = Vec<Library>;

/// Basic information about a Minecraft version from the version manifest.
///
/// # Example
/// ```
/// use mc_api::official::Versions;
///
/// let version = Versions {
///     id: "1.20.4".to_string(),
///     r#type: "release".to_string(),
///     url: "https://launchermeta.mojang.com/v1/packages/1.20.4/1.20.4.json".to_string(),
///     time: "2023-06-30T08:00:00+00:00".to_string(),
///     release_time: "2023-06-30T08:00:00+00:00".to_string(),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Versions {
    /// Version identifier (e.g., "1.20.4", "23w14a").
    pub id: String,
    /// Version type ("release" or "snapshot").
    pub r#type: String,
    /// URL to download the version JSON file.
    pub url: String,
    /// When this version was published.
    pub time: String,
    /// When this version was originally released.
    #[serde[rename = "releaseTime"]]
    pub release_time: String,
}

/// Contains the version IDs of the most recent stable release and latest snapshot.
///
/// # Example
/// ```
/// use mc_api::official::LatestVersion;
///
/// let latest = LatestVersion {
///     release: "1.20.4".to_string(),
///     snapshot: "23w14a".to_string(),
/// };
///
/// println!("Latest release: {}", latest.release);
/// println!("Latest snapshot: {}", latest.snapshot);
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LatestVersion {
    /// Latest stable release version ID.
    pub release: String,
    /// Latest snapshot version ID.
    pub snapshot: String,
}

/// Central index containing information about all available Minecraft versions.
///
/// The manifest is typically fetched from:
/// - Official: `https://launchermeta.mojang.com/mc/game/version_manifest.json`
/// - BMCLAPI: `https://bmclapi2.bangbang93.com/mc/game/version_manifest.json`
///
/// # Example
/// ```no_run
/// use mc_api::official::VersionManifest;
///
/// let mirror = "https://bmclapi2.bangbang93.com/";
/// let manifest = VersionManifest::fetch(mirror)?;
///
/// println!("Latest release: {}", manifest.latest.release);
/// println!("Latest snapshot: {}", manifest.latest.snapshot);
/// println!("Total versions: {}", manifest.versions.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionManifest {
    /// Latest release and snapshot information.
    pub latest: LatestVersion,
    /// List of all available versions.
    pub versions: Vec<Versions>,
}

impl VersionManifest {
    /// Fetches the Minecraft version manifest from a mirror.
    ///
    /// This method retrieves the central index of all available Minecraft
    /// versions from the specified mirror server.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Minecraft API mirror
    ///
    /// # Returns
    ///
    /// Returns a `VersionManifest` containing version information.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - Server returns non-200 status code
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::VersionManifest;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(mirror)?;
    ///
    /// println!("Latest release: {}", manifest.latest.release);
    /// println!("Latest snapshot: {}", manifest.latest.snapshot);
    /// println!("Total versions: {}", manifest.versions.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Common Mirrors
    ///
    /// - Official: `https://launchermeta.mojang.com/`
    /// - BMCLAPI: `https://bmclapi2.bangbang93.com/`
    /// - Other compatible mirrors
    pub fn fetch(mirror: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned() + "mc/game/version_manifest.json";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }

    /// Filters and returns a list of version IDs based on the specified type.
    ///
    /// This method filters the versions from the manifest and returns
    /// only the version IDs that match the specified type.
    ///
    /// # Parameters
    ///
    /// * `version_type` - The type of versions to filter (All, Release, or Snapshot)
    ///
    /// # Returns
    ///
    /// Returns a vector of version IDs as strings.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, VersionType};
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(mirror)?;
    ///
    /// // Get all versions
    /// let all = manifest.list(&VersionType::All);
    /// println!("Total versions: {}", all.len());
    ///
    /// // Get only releases
    /// let releases = manifest.list(&VersionType::Release);
    /// println!("Releases: {}", releases.len());
    ///
    /// // Get only snapshots
    /// let snapshots = manifest.list(&VersionType::Snapshot);
    /// println!("Snapshots: {}", snapshots.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Filtering Behavior
    ///
    /// - `All`: Returns all version IDs
    /// - `Release`: Returns only versions with type "release"
    /// - `Snapshot`: Returns only versions with type "snapshot"
    #[must_use]
    pub fn list(&self, version_type: &VersionType) -> Vec<String> {
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

    /// Returns the download URL for a specific version.
    ///
    /// This method looks up the version in the manifest and returns
    /// the URL to download its JSON file.
    ///
    /// # Parameters
    ///
    /// * `version` - The version ID to look up (e.g., "1.20.4", "23w14a")
    ///
    /// # Returns
    ///
    /// Returns the URL to download the version's JSON file.
    ///
    /// # Panics
    ///
    /// Panics if the specified version is not found in the manifest.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::VersionManifest;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(mirror)?;
    ///
    /// let url = manifest.url("1.20.4");
    /// println!("Version JSON URL: {}", url);
    ///
    /// // This will panic for non-existent versions
    /// // let url = manifest.url("999.999.999");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Notes
    ///
    /// The URL returned is from the official manifest. To use a mirror,
    /// you'll need to replace the domain using the `DomainReplacer` trait.
    #[must_use]
    pub fn url(&self, version: &str) -> String {
        self.versions
            .iter()
            .find(|x| x.id == version)
            .unwrap()
            .url
            .clone()
    }
}

/// Information about the assets index file that maps asset hashes to download URLs.
///
/// The assets index file is typically located at:
/// `assets/indexes/{id}.json`
///
/// # Example
/// ```
/// use mc_api::official::AssetIndex;
///
/// let asset_index = AssetIndex {
///     total_size: 1234567890,
///     id: "1.20.4".to_string(),
///     url: "https://launchermeta.mojang.com/v1/1.20.4/1.20.4.json".to_string(),
///     sha1: "abc123...".to_string(),
///     size: 123456,
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct AssetIndex {
    #[serde(rename = "totalSize")]
    pub total_size: usize,
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: usize,
}

/// Contains the hash and size of a single asset file.
///
/// The download URL is constructed from the hash:
/// `https://resources.download.minecraft.net/{hash[0:2]}/{hash}`
///
/// # Example
/// ```
/// use mc_api::official::Asset;
///
/// let asset = Asset {
///     hash: "1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b".to_string(),
///     size: 12345,
/// };
///
/// let url = format!(
///     "https://resources.download.minecraft.net/{}/{}",
///     &asset.hash[0..2],
///     asset.hash
/// );
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    /// SHA1 hash of the asset file (used to construct the download URL).
    pub hash: String,
    /// Size of the asset file in bytes.
    pub size: usize,
}

/// Complete mapping of asset names to their hash and size information.
///
/// This file is typically stored at: `assets/indexes/{id}.json`
/// where `{id}` is the Minecraft version (e.g., "1.20.4").
///
/// # Example
/// ```no_run
/// use mc_api::official::Assets;
/// use std::path::PathBuf;
///
/// let assets = Assets::default();
///
/// let path = PathBuf::from("./assets/indexes/1.20.4.json");
/// assets.install(&path);
/// ```
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Assets {
    /// Map of asset names to their hash and size information.
    pub objects: HashMap<String, Asset>,
}

impl Assets {
    /// Downloads the assets index JSON file from the Minecraft asset servers or a mirror,
    /// verifying the SHA1 hash. The downloaded file is verified against the SHA1 hash
    /// provided in the `AssetIndex` structure to ensure integrity.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version, Assets};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let assets_mirror = "https://bmclapi2.bangbang93.com/";
    ///
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    /// let assets = Assets::fetch(&version.asset_index, assets_mirror)?;
    ///
    /// println!("Total assets: {}", assets.objects.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - SHA1 hash verification fails
    /// - Server returns non-200 status code
    pub fn fetch(asset_index: &AssetIndex, mirror: &str) -> anyhow::Result<Self> {
        let url = asset_index.url.replace_domain(mirror);
        let sha1 = &asset_index.sha1;
        FetcherBuilder::fetch(&url).sha1(sha1).execute()?.json()
    }

    /// Writes the assets index to a file, creating parent directories as needed.
    /// The file is written as pretty-printed JSON for human readability.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version, Assets};
    /// use std::path::PathBuf;
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let assets_mirror = "https://bmclapi2.bangbang93.com/";
    ///
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    /// let assets = Assets::fetch(&version.asset_index, assets_mirror)?;
    ///
    /// let asset_path = PathBuf::from("./assets/indexes/1.20.4.json");
    /// assets.install(&asset_path);
    ///
    /// println!("Assets index installed to: {:?}", asset_path);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Panics
    /// Panics if:
    /// - Parent directory creation fails
    /// - File writing fails
    /// - JSON serialization fails
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
}

/// Game and JVM arguments for launching Minecraft.
///
/// Arguments can be either simple strings or complex structures containing
/// conditional logic based on rules.
#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    /// Arguments to pass to the Minecraft game process.
    pub game: Vec<serde_json::Value>,
    /// Arguments to pass to the Java virtual machine.
    pub jvm: Vec<serde_json::Value>,
}

/// Complete information needed to launch a specific Minecraft version.
///
/// Version JSON files are typically located at: `versions/{version}/{version}.json`
///
/// # Example
/// ```no_run
/// use mc_api::official::{VersionManifest, Version};
///
/// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
/// let manifest = VersionManifest::fetch(manifest_mirror)?;
/// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
///
/// println!("Version: {}", version.id);
/// println!("Type: {}", version.r#type);
/// println!("Main class: {}", version.main_class);
/// println!("Libraries: {}", version.libraries.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    /// Game and JVM launch arguments.
    pub arguments: Arguments,
    /// Information about the assets index.
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
    /// The type of assets ("legacy" or "standard").
    pub assets: String,
    /// The compliance level of the version.
    #[serde(rename = "complianceLevel")]
    pub compliance_level: usize,
    /// Download information for client, server, etc.
    pub downloads: serde_json::Value,
    /// The version identifier.
    pub id: String,
    /// Information about the required Java version.
    #[serde(rename = "javaVersion")]
    pub java_version: serde_json::Value,
    /// List of required library dependencies.
    pub libraries: Libraries,
    /// Logging configuration.
    pub logging: serde_json::Value,
    /// The main class to launch.
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// Minimum launcher version required.
    #[serde(rename = "minimumLauncherVersion")]
    pub minimum_launcher_version: usize,
    /// When this version was originally released.
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    /// When this version was last updated.
    pub time: String,
    /// The type of version ("release" or "snapshot").
    pub r#type: String,
}

/// Trait for merging mod loader profiles with official Minecraft versions.
///
/// Mod loaders like Fabric provide their own versions of version JSON files that
/// extend the official Minecraft version with additional libraries, arguments,
/// and configuration changes. This trait allows these profiles to be merged
/// with official versions.
///
/// # Example Implementation
/// ```
/// use mc_api::official::MergeVersion;
///
/// struct CustomModProfile {
///     // ... fields
/// }
///
/// impl MergeVersion for CustomModProfile {
///     fn official_libraries(&self) -> Option<Vec<mc_api::official::Library>> {
///         Some(vec![])
///     }
///
///     fn main_class(&self) -> Option<String> {
///         Some("com.example.CustomModLoader".to_string())
///     }
///
///     fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
///         None
///     }
///
///     fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
///         Some(vec![])
///     }
/// }
/// ```
///
/// # Usage
/// ```no_run
/// use mc_api::official::{VersionManifest, Version, MergeVersion};
/// use mc_api::fabric::Profile;
///
/// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
/// let fabric_mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
///
/// let manifest = VersionManifest::fetch(manifest_mirror)?;
/// let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
/// let mod_profile = Profile::fetch(fabric_mirror,"1.20.4","0.15.10")?;
///
/// version.merge(&mod_profile);
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// Common mod loaders: Fabric, Forge, Quilt
pub trait MergeVersion {
    /// Returns the libraries that the mod loader requires, converted to the official library format.
    /// Returns `Some(Vec<Library>)` with mod loader libraries, or `None` if there are no additional libraries.
    fn official_libraries(&self) -> Option<Vec<Library>>;

    /// Returns the main class that should be used instead of the official Minecraft main class.
    /// Returns `Some(String)` with the main class name, or `None` to use the official main class.
    fn main_class(&self) -> Option<String>;

    /// Returns any additional game arguments that the mod loader requires.
    /// Returns `Some(Vec<Value>)` with additional game arguments, or `None` if there are no additional arguments.
    fn arguments_game(&self) -> Option<Vec<serde_json::Value>>;

    /// Returns any additional JVM arguments that the mod loader requires.
    /// Returns `Some(Vec<Value>)` with additional JVM arguments, or `None` if there are no additional arguments.
    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>>;
}

impl Version {
    /// Retrieves the detailed version information for a specific Minecraft version from the specified mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// println!("Version: {}", version.id);
    /// println!("Type: {}", version.r#type);
    /// println!("Main class: {}", version.main_class);
    /// println!("Libraries: {}", version.libraries.len());
    /// println!("Java version: {}", version.java_version);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - Server returns non-200 status code
    /// - Version not found in manifest
    pub fn fetch(manifest: &VersionManifest, version: &str, mirror: &str) -> anyhow::Result<Self> {
        let url = manifest.url(version).replace_domain(mirror);
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }

    /// Writes the version information to a file, creating parent directories as needed.
    /// The file is written as pretty-printed JSON for human readability.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    /// use std::path::PathBuf;
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// let version_path = PathBuf::from("./versions/1.20.4/1.20.4.json");
    /// version.install(&version_path);
    ///
    /// println!("Version JSON installed to: {:?}", version_path);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Panics
    /// Panics if:
    /// - Parent directory creation fails
    /// - File writing fails
    /// - JSON serialization fails
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    /// Combines the official version information with a mod loader's profile.
    ///
    /// The merge operation:
    /// - Appends mod loader libraries to the existing library list
    /// - Replaces the main class if the mod loader provides one
    /// - Appends mod loader game arguments to existing game arguments
    /// - Appends mod loader JVM arguments to existing JVM arguments
    ///
    /// Common mod loaders: Fabric, Forge, Quilt
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version, MergeVersion};
    /// use mc_api::fabric::Profile;
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let fabric_mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    ///
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    /// let profile = Profile::fetch(fabric_mirror, "1.20.4", "0.15.10")?;
    ///
    /// version.merge(&profile);
    ///
    /// println!("Total libraries: {}", version.libraries.len());
    /// println!("Main class: {}", version.main_class);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn merge<T>(&mut self, other: &T)
    where
        T: MergeVersion,
    {
        if let Some(mut libs) = other.official_libraries() {
            self.libraries.append(&mut libs);
        }
        if let Some(main_class) = other.main_class() {
            self.main_class = main_class;
        }
        if let Some(mut arguments_game) = other.arguments_game() {
            self.arguments.game.append(&mut arguments_game);
        }
        if let Some(mut arguments_jvm) = other.arguments_jvm() {
            self.arguments.jvm.append(&mut arguments_jvm);
        }
    }
}
