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
//! let mod_profile = /* fetch mod loader profile */;
//!

use super::{DomainReplacer, Sha1Compare};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const OS: &str = "windows";

#[cfg(target_os = "linux")]
const OS: &str = "linux";

#[cfg(target_os = "macos")]
const OS: &str = "osx";

/// Represents the type of Minecraft version.
///
/// Used for filtering versions from the version manifest.
///
/// # Variants
///
/// * `All` - Include all versions (both release and snapshot)
/// * `Release` - Include only release versions
/// * `Snapshot` - Include only snapshot versions
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
///
/// // Get only releases
/// let releases = manifest.list(&VersionType::Release);
///
/// // Get only snapshots
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

/// Represents a library artifact download information.
///
/// Contains information about a library file that can be downloaded,
/// including its path, SHA1 hash, size, and download URL.
///
/// # Fields
///
/// * `path` - The relative path where the library should be stored
/// * `sha1` - Optional SHA1 hash for integrity verification
/// * `size` - Optional file size in bytes
/// * `url` - The URL where the library can be downloaded
///
/// # Path Format
///
/// The path follows the standard Maven directory structure:
/// `groupId/path/artifactId/version/artifactId-version.jar`
///
/// # Example
///
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Artifact {
    pub path: String,
    pub sha1: Option<String>,
    pub size: Option<i32>,
    pub url: String,
}

/// Represents download information for a library.
///
/// Contains the main artifact download and optional classifier downloads
/// for platform-specific libraries (natives).
///
/// # Fields
///
/// * `artifact` - The main library artifact download information
/// * `classifiers` - Optional map of platform-specific artifacts (e.g., natives)
///
/// # Classifiers
///
/// Classifiers are used for platform-specific library variants:
/// - `natives-windows` - Windows native libraries
/// - `natives-linux` - Linux native libraries
/// - `natives-osx` - macOS native libraries
///
/// # Example
///
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LibDownloads {
    pub artifact: Artifact,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

/// Represents a rule that determines when a library should be included.
///
/// Library rules are used to conditionally include libraries based on
/// operating system or other conditions.
///
/// # Fields
///
/// * `action` - Either "allow" or "disallow"
/// * `os` - Optional operating system filter
///
/// # Rule Evaluation
///
/// Rules are evaluated in order:
/// - If action is "allow" and OS matches, library is included
/// - If action is "disallow" and OS matches, library is excluded
/// - If no rules match, default behavior is to include the library
///
/// # Example
///
/// ```
/// use mc_api::official::Rules;
/// use std::collections::HashMap;
///
/// // Include library only on Windows
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
    pub action: String,
    pub os: Option<HashMap<String, String>>,
}

/// Represents a library dependency in the version JSON.
///
/// Contains comprehensive information about a library including download
/// information, platform-specific variants, and inclusion rules.
///
/// # Fields
///
/// * `downloads` - Download information for the library
/// * `name` - The Maven coordinate name (groupId:artifactId:version)
/// * `natives` - Optional map of platform-specific library names
/// * `rules` - Optional list of inclusion rules
///
/// # Platform Filtering
///
/// Libraries can be filtered based on the current platform using:
/// - `is_target_lib()` - Checks if library should be included for current OS
/// - `is_target_native()` - Checks if library is a native library for current OS
///
/// # Example
///
/// ```
/// use mc_api::official::Library;
///
/// // Check if library is needed for current platform
/// if library.is_target_lib() {
///     println!("Library {} is needed", library.name);
/// }
///
/// // Check if library is a native library
/// if library.is_target_native() {
///     println!("Library {} is a native library", library.name);
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Library {
    pub downloads: LibDownloads,
    pub name: String,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rules>>,
}

impl Library {
    /// Determines if this library should be included for the current platform.
    ///
    /// This method evaluates the library's rules to determine if it should be
    /// included in the classpath for the current operating system.
    ///
    /// # Rule Evaluation
    ///
    /// The method checks:
    /// - If there are no rules, the library is included (no classifiers)
    /// - If there are rules, finds a rule that applies to the current OS
    /// - The library is included if a matching rule exists and has no classifiers
    ///
    /// # Platform Detection
    ///
    /// The current platform is detected at compile time:
    /// - Windows → `OS` constant is `"windows"`
    /// - Linux → `OS` constant is `"linux"`
    /// - macOS → `OS` constant is `"osx"`
    ///
    /// # Returns
    ///
    /// Returns `true` if the library should be included, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(manifest, "1.20.4", manifest_mirror)?;
    ///
    /// // Get only libraries needed for current platform
    /// let target_libs: Vec<_> = version.libraries.iter()
    ///     .filter(|lib| lib.is_target_lib())
    ///     .map(|lib| lib.name.clone())
    ///     .collect();
    ///
    /// println!(\"Libraries needed: {}\", target_libs.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a library rule has invalid OS configuration (this is
    /// unlikely with well-formed version JSON files).
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
    /// This method checks if the library has platform-specific (native) variants
    /// and if one exists for the current operating system.
    ///
    /// # Native Libraries
    ///
    /// Native libraries contain compiled code specific to an operating system:
    /// - Windows DLLs (`.dll` files)
    /// - Linux shared objects (`.so` files)
    /// - macOS dynamic libraries (`.dylib` files)
    ///
    /// # Returns
    ///
    /// Returns `true` if the library has a native variant for the current platform,
    /// `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(manifest, "1.20.4", manifest_mirror)?;
    ///
    /// // Get only native libraries for current platform
    /// let native_libs: Vec<_> = version.libraries.iter()
    ///     .filter(|lib| lib.is_target_native())
    ///     .map(|lib| lib.name.clone())
    ///     .collect();
    ///
    /// println!(\"Native libraries needed: {}\", native_libs.len());
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

/// Represents a version entry in the version manifest.
///
/// Contains basic information about a Minecraft version including its type,
/// URL, and timestamps.
///
/// # Fields
///
/// * `id` - The version identifier (e.g., "1.20.4", "23w14a")
/// * `r#type` - The type of version ("release" or "snapshot")
/// * `url` - The URL to download the version JSON file
/// * `time` - When this version was published
/// * `release_time` - When this version was originally released
///
/// # Example
///
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
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde[rename = "releaseTime"]]
    pub release_time: String,
}

/// Represents the latest release and snapshot versions.
///
/// Contains the version IDs of the most recent stable release
/// and the latest snapshot build.
///
/// # Fields
///
/// * `release` - The latest stable release version ID
/// * `snapshot` - The latest snapshot version ID
///
/// # Example
///
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
    pub release: String,
    pub snapshot: String,
}

/// Represents the Minecraft version manifest.
///
/// The version manifest is a central index that contains information about
/// all available Minecraft versions, including the latest releases and snapshots.
///
/// # Fields
///
/// * `latest` - Information about the latest release and snapshot
/// * `versions` - List of all available versions
///
/// # Usage
///
/// The manifest is typically fetched from the official API or a mirror:
/// - Official: `https://launchermeta.mojang.com/mc/game/version_manifest.json`
/// - BMCLAPI: `https://bmclapi2.bangbang93.com/mc/game/version_manifest.json`
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionManifest {
    pub latest: LatestVersion,
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
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
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

/// Represents the asset index information from a version JSON.
///
/// Contains information about the assets index file that maps asset
/// hashes to download URLs.
///
/// # Fields
///
/// * `total_size` - Total size of all assets in bytes
/// * `id` - The asset index ID (typically matches the Minecraft version)
/// * `url` - The URL to download the assets index JSON file
/// * `sha1` - The SHA1 hash of the assets index file for verification
/// * `size` - The size of the assets index file in bytes
///
/// # Asset Index File
///
/// The assets index file is typically located at:
/// `assets/indexes/{id}.json`
///
/// # Example
///
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
    #[serde[rename = "totalSize"]]
    pub total_size: usize,
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: usize,
}

/// Represents an individual asset entry in the assets index.
///
/// Contains the hash and size of a single asset file.
///
/// # Fields
///
/// * `hash` - The SHA1 hash of the asset file (used to construct the download URL)
/// * `size` - The size of the asset file in bytes
///
/// # Asset URL Construction
///
/// The download URL is constructed from the hash:
/// `https://resources.download.minecraft.net/{hash[0:2]}/{hash}`
///
/// # Example
///
/// ```
/// use mc_api::official::Asset;
///
/// let asset = Asset {
///     hash: "1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b".to_string(),
///     size: 12345,
/// };
///
/// // Construct download URL
/// let url = format!(
///     "https://resources.download.minecraft.net/{}/{}",
///     &asset.hash[0..2],
///     asset.hash
/// );
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    pub hash: String,
    pub size: usize,
}

/// Represents the assets index JSON file.
///
/// This structure contains the complete mapping of asset names to their
/// hash and size information, retrieved from the Minecraft asset servers.
///
/// # File Location
///
/// This file is typically stored at: `assets/indexes/{id}.json`
/// where `{id}` is the Minecraft version (e.g., "1.20.4").
///
/// # Fields
///
/// * `objects` - A map of asset names to their hash and size information
///
/// # Example
///
/// ```no_run
/// use mc_api::official::Assets;
/// use std::path::PathBuf;
///
/// let assets = Assets { /* ... */ };
///
/// // Install the assets index file
/// let path = PathBuf::from("./assets/indexes/1.20.4.json");
/// assets.install(&path);
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Assets {
    pub objects: HashMap<String, Asset>,
}

impl Assets {
    /// Fetches the assets index from the specified mirror.
    ///
    /// This method downloads the assets index JSON file from the Minecraft
    /// asset servers or a mirror, verifying the SHA1 hash.
    ///
    /// # Parameters
    ///
    /// * `asset_index` - The `AssetIndex` information containing the URL and hash
    /// * `mirror` - The base URL of the mirror server for asset downloads
    ///
    /// # Returns
    ///
    /// Returns a populated `Assets` structure with the asset mappings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - SHA1 hash verification fails
    /// - Server returns non-200 status code
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version, Assets};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let assets_mirror = "https://bmclapi2.bangbang93.com/";
    ///
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// // Fetch the assets index
    /// let assets = Assets::fetch(&version.asset_index, assets_mirror)?;
    ///
    /// println!("Total assets: {}", assets.objects.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # SHA1 Verification
    ///
    /// The downloaded file is verified against the SHA1 hash provided
    /// in the `AssetIndex` structure to ensure integrity.
    pub fn fetch(asset_index: &AssetIndex, mirror: &str) -> anyhow::Result<Self> {
        let url = asset_index.url.replace_domain(mirror);
        let client = reqwest::blocking::Client::new();
        let sha1 = &asset_index.sha1;
        let data = fetch!(client, url, sha1, text)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Installs the assets index JSON file to the specified path.
    ///
    /// This method writes the assets index to a file, creating parent
    /// directories as needed.
    ///
    /// # Parameters
    ///
    /// * `file` - The path where the assets index should be saved
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Parent directory creation fails
    /// - File writing fails
    /// - JSON serialization fails
    ///
    /// # Example
    ///
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
    /// // Install assets index
    /// let asset_path = PathBuf::from("./assets/indexes/1.20.4.json");
    /// assets.install(&asset_path);
    ///
    /// println!("Assets index installed to: {:?}", asset_path);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # File Format
    ///
    /// The file is written as pretty-printed JSON for human readability.
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
}

/// Represents game and JVM arguments for launching Minecraft.
///
/// Contains the argument lists that should be passed to the game and JVM
/// when launching Minecraft.
///
/// # Fields
///
/// * `game` - Arguments to pass to the Minecraft game process
/// * `jvm` - Arguments to pass to the Java virtual machine
///
/// # Argument Format
///
/// Arguments can be either simple strings or complex structures
/// containing conditional logic based on rules.
#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

/// Represents a complete Minecraft version JSON file.
///
/// This structure contains all the information needed to launch a specific
/// version of Minecraft, including libraries, arguments, assets, and more.
///
/// # File Location
///
/// Version JSON files are typically located at:
/// `versions/{version}/{version}.json`
///
/// # Fields
///
/// * `arguments` - Game and JVM launch arguments
/// * `asset_index` - Information about the assets index
/// * `assets` - The type of assets ("legacy" or "standard")
/// * `compliance_level` - The compliance level of the version
/// * `downloads` - Download information for client, server, etc.
/// * `id` - The version identifier
/// * `java_version` - Information about the required Java version
/// * `libraries` - List of required library dependencies
/// * `logging` - Logging configuration
/// * `main_class` - The main class to launch
/// * `minimum_launcher_version` - Minimum launcher version required
/// * `release_time` - When this version was originally released
/// * `time` - When this version was last updated
/// * `r#type` - The type of version ("release" or "snapshot")
///
/// # Example
///
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
    pub arguments: Arguments,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
    pub assets: String,
    #[serde(rename = "complianceLevel")]
    pub compliance_level: usize,
    pub downloads: serde_json::Value,
    pub id: String,
    #[serde(rename = "javaVersion")]
    pub java_version: serde_json::Value,
    pub libraries: Libraries,
    pub logging: serde_json::Value,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minimumLauncherVersion")]
    pub minimum_launcher_version: usize,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub time: String,
    pub r#type: String,
}

/// Trait for merging mod loader profiles with official Minecraft versions.
///
/// This trait provides a standardized interface for integrating mod loader
/// configurations (like Fabric, Forge, etc.) with official Minecraft version
/// JSON files.
///
/// # Purpose
///
/// Mod loaders like Fabric provide their own versions of version JSON files that
/// extend the official Minecraft version with additional libraries, arguments,
/// and configuration changes. This trait allows these profiles to be merged
/// with official versions.
///
/// # Required Methods
///
/// * `official_libraries()` - Returns mod loader-specific libraries
/// * `main_class()` - Returns the mod loader's main class
/// * `arguments_game()` - Returns game arguments to merge
/// * `arguments_jvm()` - Returns JVM arguments to merge
///
/// # Example Implementation
///
/// ```
/// use mc_api::official::MergeVersion;
///
/// struct CustomModProfile {
///     // ... fields
/// }
///
/// impl MergeVersion for CustomModProfile {
///     fn official_libraries(&self) -> Option<Vec<mc_api::official::Library>> {
///         Some(vec![]) // Return custom libraries
///     }
///
///     fn main_class(&self) -> Option<String> {
///         Some("com.example.CustomModLoader".to_string())
///     }
///
///     fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
///         None // No additional game arguments
///     }
///
///     fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
///         Some(vec![]) // Additional JVM arguments
///     }
/// }
/// ```
///
/// # Usage
///
/// ```no_run
/// use mc_api::official::{VersionManifest, Version, MergeVersion};
///
/// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
/// let manifest = VersionManifest::fetch(manifest_mirror)?;
/// let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
/// let mod_profile = /* fetch mod profile */;
///
/// // Merge mod profile into official version
/// version.merge(&mod_profile);
///
/// // The version now includes mod loader libraries and arguments
/// # Ok::<(), anyhow::Error>(())
/// ```
pub trait MergeVersion {
    /// Returns mod loader-specific libraries compatible with the official format.
    ///
    /// This method should return the libraries that the mod loader requires,
   /// converted to the official library format.
    ///
    /// # Returns
    ///
    /// Returns `Some(Vec<Library>)` with mod loader libraries, or `None` if
    /// there are no additional libraries.
    fn official_libraries(&self) -> Option<Vec<Library>>;

    /// Returns the mod loader's main class.
    ///
    /// This method should return the main class that should be used instead
    /// of the official Minecraft main class.
    ///
    /// # Returns
    ///
    /// Returns `Some(String)` with the main class name, or `None` to use
    /// the official main class.
    fn main_class(&self) -> Option<String>;

    /// Returns game arguments to merge with the official version.
    ///
    /// This method should return any additional game arguments that the
    /// mod loader requires.
    ///
    /// # Returns
    ///
    /// Returns `Some(Vec<Value>)` with additional game arguments, or `None`
    /// if there are no additional arguments.
    fn arguments_game(&self) -> Option<Vec<serde_json::Value>>;

    /// Returns JVM arguments to merge with the official version.
    ///
    /// This method should return any additional JVM arguments that the
    /// mod loader requires.
    ///
    /// # Returns
    ///
    /// Returns `Some(Vec<Value>)` with additional JVM arguments, or `None`
    /// if there are no additional arguments.
    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>>;
}

impl Version {
    /// Fetches a Minecraft version JSON from the version manifest.
    ///
    /// This method retrieves the detailed version information for a specific
    /// Minecraft version from the specified mirror.
    ///
    /// # Parameters
    ///
    /// * `manifest` - The version manifest containing version URLs
    /// * `version` - The version ID to fetch (e.g., \"1.20.4\", \"23w14a\")
    /// * `mirror` - The base URL of the mirror server for version downloads
    ///
    /// # Returns
    ///
    /// Returns a populated `Version` structure with all version information.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - Server returns non-200 status code
    /// - Version not found in manifest
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    ///
    /// // Fetch version 1.20.4
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// println!("Version: {}", version.id);
    /// println!("Type: {}", version.r#type);
    /// println!("Main class: {}", version.main_class);
    /// println!("Libraries: {}", version.libraries.len());
    /// println!("Java version: {}", version.java_version);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn fetch(manifest: &VersionManifest, version: &str, mirror: &str) -> anyhow::Result<Self> {
        let url = manifest.url(version).replace_domain(mirror);
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }

    /// Installs the version JSON file to the specified path.
    ///
    /// This method writes the version information to a file, creating parent
    /// directories as needed.
    ///
    /// # Parameters
    ///
    /// * `file` - The path where the version JSON should be saved
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Parent directory creation fails
    /// - File writing fails
    /// - JSON serialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version};
    /// use std::path::PathBuf;
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// // Install version JSON
    /// let version_path = PathBuf::from("./versions/1.20.4/1.20.4.json");
    /// version.install(&version_path);
    ///
    /// println!("Version JSON installed to: {:?}", version_path);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # File Format
    ///
    /// The file is written as pretty-printed JSON for human readability.
    pub fn install<P>(&self, file: &P)
    where
        P: AsRef<Path>,
    {
        let text = serde_json::to_string_pretty(self).unwrap();
        fs::create_dir_all(file.as_ref().parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    /// Merges a mod loader profile into this version.
    ///
    /// This method combines the official version information with a mod loader's
    /// profile, adding mod loader libraries, replacing the main class, and merging
    /// arguments as needed.
    ///
    /// # Parameters
    ///
    /// * `other` - A type implementing `MergeVersion` (e.g., Fabric profile)
    ///
    /// # Behavior
    ///
    /// The merge operation:
    /// - Appends mod loader libraries to the existing library list
    /// - Replaces the main class if the mod loader provides one
    /// - Appends mod loader game arguments to existing game arguments
    /// - Appends mod loader JVM arguments to existing JVM arguments
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::official::{VersionManifest, Version, MergeVersion};
    /// use mc_api::fabric::Profile;
    ///
    /// let manifest_mirror = "https://bmclapi2.bangbang93.com/";
    /// let fabric_mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    ///
    /// let manifest = VersionManifest::fetch(manifest_mirror)?;
    /// let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
    ///
    /// // Fetch Fabric profile
    /// let profile = Profile::fetch(fabric_mirror, "1.20.4", "0.15.10")?;
    ///
    /// // Merge Fabric profile into version
    /// version.merge(&profile);
    ///
    /// // Version now includes Fabric libraries and arguments
    /// println!("Total libraries: {}", version.libraries.len());
    /// println!("Main class: {}", version.main_class);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Common Mod Loaders
    ///
    /// This trait is commonly used with:
    /// - Fabric: `mc_api::fabric::Profile`
    /// - Forge: Similar profiles (not yet implemented)
    /// - Quilt: Similar profiles (not yet implemented)
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
