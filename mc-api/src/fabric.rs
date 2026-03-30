//! Fabric Loader API Module
//!
//! This module provides functionality for interacting with the Fabric mod loader API,
//! including fetching version information, loader versions, yarn mappings, and
//! installation profiles.
//!
//! # API Endpoints
//!
//! The Fabric Meta API provides the following endpoints:
//!
//! - `/v2/versions/game` - List all supported Minecraft versions
//! - `/v2/versions/yarn` - List all Yarn mapping versions
//! - `/v2/versions/loader` - List all Fabric loader versions
//! - `/v2/versions/intermediary` - List all intermediary versions
//! - `/v2/versions` - Full database with all version information
//! - `/v2/versions/loader/{game_version}/{loader_version}/profile/json` - Fabric profile JSON
//!
//! # Mirror Support
//!
//! The library supports using mirror servers for the Fabric Meta API:
//!
//! - Official: `https://meta.fabricmc.net/`
//! - BMCLAPI: `https://bmclapi2.bangbang93.com/fabric-meta/`
//!
//! # Usage Example
//!
//! ```no_run
//! use mc_api::fabric::{Versions, Profile};
//!
//! // Fetch all Fabric metadata
//! let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
//! let versions = Versions::fetch(mirror)?;
//!
//! // Get the latest stable loader
//! let latest_loader = versions.loader.iter().find(|l| l.stable).unwrap();
//! println!("Latest loader: {}", latest_loader.version);
//!
//! // Fetch a specific Fabric profile
//! let profile = Profile::fetch(mirror, "1.20.6", "0.15.10")?;
//! println!("Main class: {}", profile.main_class);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Version Types
//!
//! The Fabric API tracks several types of versions:
//!
//! - **Game Versions**: Minecraft versions supported by Fabric
//! - **Yarn Mappings**: Yarn mapping versions for deobfuscation
//! - **Loader Versions**: Fabric loader versions
//! - **Intermediary**: Intermediary mapping versions
//! - **Installer**: Fabric installer versions
//!
//! # Profile Integration
//!
//! Fabric profiles can be merged with official Minecraft versions to create
//! complete modded game installations:
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version};
//! use mc_api::fabric::Profile;
//!
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let fabric_mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
//!
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//! let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//! let profile = Profile::fetch(fabric_mirror, "1.20.4", "0.15.10")?;
//!
//! // Merge Fabric profile into official version
//! version.merge(&profile);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # For more information
//!
//! See the [Fabric Meta API documentation](https://github.com/FabricMC/fabric-meta)

use super::official;
use crate::fetcher::FetcherBuilder;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Represents a supported Minecraft game version for Fabric.
#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    /// Version string (e.g., "1.20.6").
    pub version: String,
    /// Whether this version is considered stable.
    pub stable: bool,
}

impl Game {
    /// Fetches all supported game versions for Fabric.
    ///
    /// Returns a vector of all Minecraft versions that the Fabric mod loader supports.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror (e.g., the official
    /// meta.fabricmc.net or BMCLAPI's mirror).
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Game;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let games = Game::fetch(mirror)?;
    ///
    /// for game in &games {
    ///     let status = if game.stable { "stable" } else { "unstable" };
    ///     println!("{} ({})", game.version, status);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// or the server returns a non-success status code.
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/game";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Yarn mapping version for Fabric.
#[derive(Debug, Serialize, Deserialize)]
pub struct Yarn {
    /// Minecraft version this mapping targets.
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    /// Separator used in mapping names.
    pub separator: String,
    /// Build number of this mapping.
    pub build: i32,
    /// Maven coordinates for downloading.
    pub maven: String,
    /// Version string of this mapping.
    pub version: String,
    /// Whether this version is considered stable.
    pub stable: bool,
}

impl Yarn {
    /// Fetches all Yarn mapping versions.
    ///
    /// Returns a vector of all Yarn mapping versions available from the specified mirror.
    /// Stability is determined by the associated Minecraft version's stability.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Yarn;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let yarns = Yarn::fetch(mirror)?;
    ///
    /// println!("Available Yarn mappings:");
    /// for yarn in &yarns {
    ///     let status = if yarn.stable { "stable" } else { "unstable" };
    ///     println!("  {} for Minecraft {} ({})", yarn.version, yarn.game_version, status);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// or the server returns a non-success status code.
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/yarn";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Fabric loader version.
#[derive(Debug, Serialize, Deserialize)]
pub struct Loader {
    /// Separator used in version strings.
    pub separator: String,
    /// Build number of this loader.
    pub build: i32,
    /// Maven coordinates for downloading.
    pub maven: String,
    /// Version string of this loader.
    pub version: String,
    /// Whether this version is considered stable.
    pub stable: bool,
}

impl Loader {
    /// Fetches all Fabric loader versions.
    ///
    /// Returns a vector of all Fabric loader versions available from the specified mirror.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Loader;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let loaders = Loader::fetch(mirror)?;
    ///
    /// println!("Available loaders:");
    /// for loader in &loaders {
    ///     let status = if loader.stable { "stable" } else { "unstable" };
    ///     println!("  {} (build {}) - {}", loader.version, loader.build, status);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// or the server returns a non-success status code.
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/loader";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents an intermediary mapping version for Fabric.
#[derive(Debug, Serialize, Deserialize)]
pub struct Intermediary {
    /// Maven coordinates for downloading.
    pub maven: String,
    /// Version string of this intermediary.
    pub version: String,
    /// Whether this version is considered stable.
    pub stable: bool,
}

impl Intermediary {
    /// Fetches all intermediary mapping versions.
    ///
    /// Returns a vector of all intermediary mapping versions available from the specified mirror.
    /// Stability is determined by the associated Minecraft version's stability.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Intermediary;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let intermediaries = Intermediary::fetch(mirror)?;
    ///
    /// println!("Available intermediaries:");
    /// for intermediary in &intermediaries {
    ///     let status = if intermediary.stable { "stable" } else { "unstable" };
    ///     println!("  {} ({})", intermediary.version, status);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// or the server returns a non-success status code.
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/intermediary";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Fabric installer version.
#[derive(Debug, Serialize, Deserialize)]
pub struct Installer {
    /// URL to download this installer.
    pub url: String,
    /// Maven coordinates.
    pub maven: String,
    /// Version string of this installer.
    pub version: String,
    /// Whether this version is considered stable.
    pub stable: bool,
}

/// Complete Fabric metadata database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Versions {
    /// All supported Minecraft game versions.
    pub game: Vec<Game>,
    /// All Yarn mapping versions.
    pub mappings: Vec<Yarn>,
    /// All intermediary mapping versions.
    pub intermediary: Vec<Intermediary>,
    /// All Fabric loader versions.
    pub loader: Vec<Loader>,
    /// All Fabric installer versions.
    pub installer: Vec<Installer>,
}

impl Versions {
    /// Fetches the complete Fabric metadata database.
    ///
    /// Returns all available version information from the specified mirror,
    /// including game versions, mappings, intermediaries, loaders, and installers.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Versions;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let versions = Versions::fetch(mirror)?;
    ///
    /// // Find compatible components for a Minecraft version
    /// let mc_version = "1.20.6";
    /// let stable_loader = versions.loader.iter()
    ///     .filter(|l| l.stable)
    ///     .last()
    ///     .unwrap();
    /// println!("Stable loader for {}: {}", mc_version, stable_loader.version);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// or the server returns a non-success status code.
    pub fn fetch(mirror: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned() + "v2/versions";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Game and JVM arguments for Fabric.
#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    /// Arguments to pass to the Minecraft game process.
    pub game: Vec<serde_json::Value>,
    /// Arguments to pass to the Java virtual machine.
    pub jvm: Vec<serde_json::Value>,
}

/// Library dependency from a Fabric profile.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Library {
    name: String,
    url: String,
    md5: Option<String>,
    sha1: Option<String>,
    sha256: Option<String>,
    sha512: Option<String>,
    size: Option<i32>,
}

impl From<Library> for official::Library {
    fn from(lib: Library) -> Self {
        let artifact = official::Artifact {
            path: to_path(&lib.name),
            sha1: lib.sha1,
            size: lib.size,
            url: lib.url,
        };
        let downloads = official::LibDownloads {
            artifact,
            classifiers: None,
        };
        official::Library {
            downloads,
            name: lib.name,
            natives: None,
            rules: None,
        }
    }
}

/// Converts a Maven coordinate name to a file path.
///
/// Transforms a Maven coordinate string (e.g., `group:artifact:version`) into the
/// corresponding file path used in Minecraft's library directory structure.
fn to_path(name: &str) -> String {
    let mut name: VecDeque<&str> = name.split(':').collect();
    let version = &name.pop_back().unwrap();
    let file = &name.pop_back().unwrap();
    let mut res = String::new();
    for i in name {
        res += i.replace('.', "/").as_ref();
        res += "/";
    }
    format!("{res}{file}/{version}/{file}-{version}.jar")
}

#[test]
fn test_name_to_path() {
    let name = "net.fabricmc:sponge-mixin:0.13.3+mixin.0.8.5".to_owned();
    let ans = "net/fabricmc/sponge-mixin/0.13.3+mixin.0.8.5/sponge-mixin-0.13.3+mixin.0.8.5.jar"
        .to_owned();
    assert_eq!(to_path(&name), ans);
}

/// Fabric loader profile JSON for the standard Minecraft launcher.
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    /// Profile ID (e.g., "fabric-loader-0.15.10-1.20.6").
    pub id: String,
    /// Minecraft version this profile inherits from.
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: String,
    /// Release timestamp.
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    /// Last update timestamp.
    pub time: String,
    /// Profile type (typically "release" or "snapshot").
    pub r#type: String,
    /// Main class to launch.
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// Game and JVM arguments.
    pub arguments: Arguments,
    /// Required library dependencies.
    pub libraries: Vec<Library>,
}

impl Profile {
    /// Fetches a Fabric loader profile for a specific game and loader version.
    ///
    /// Returns the JSON profile that should be used in the standard Minecraft launcher
    /// for launching with Fabric. Spaces in version strings are URL-encoded as `%20`.
    /// The mirror URL should be the base URL of a Fabric Meta API mirror.
    ///
    /// # Example
    /// ```no_run
    /// use mc_api::fabric::Profile;
    ///
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
    /// let game_version = "1.20.6-rc1";
    /// let loader_version = "0.15.10";
    ///
    /// let profile = Profile::fetch(mirror, game_version, loader_version)?;
    ///
    /// println!("Fetched profile for Fabric {} on Minecraft {}",
    ///     loader_version, game_version);
    /// println!("Main class: {}", profile.main_class);
    /// println!("Libraries: {}", profile.libraries.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the response cannot be parsed as JSON,
    /// the server returns a non-success status code, or an invalid game or loader version is specified.
    pub fn fetch(mirror: &str, game_version: &str, loader_version: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned()
            + "v2/versions/loader/"
            + game_version.replace(' ', "%20").as_ref()
            + "/"
            + loader_version.replace(' ', "%20").as_ref()
            + "/profile/json";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Implementation of `official::MergeVersion` for `Profile`.
impl official::MergeVersion for Profile {
    fn official_libraries(&self) -> Option<Vec<official::Library>> {
        Some(self.libraries.iter().map(|x| x.clone().into()).collect())
    }

    fn main_class(&self) -> Option<String> {
        Some(self.main_class.clone())
    }

    fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
        None
    }

    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
        Some(self.arguments.jvm.clone())
    }
}
