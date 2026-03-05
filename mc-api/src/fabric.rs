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
///
/// Contains information about Minecraft versions that are supported by the
/// Fabric mod loader, including stability status.
///
/// # Fields
///
/// * `version` - The Minecraft version string (e.g., "1.20.6")
/// * `stable` - Whether this version is considered stable
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Game;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let games = Game::fetch(mirror)?;
///
/// // Find the latest stable version
/// let latest_stable = games.iter().filter(|g| g.stable).last().unwrap();
/// println!("Latest stable version: {}", latest_stable.version);
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    pub version: String,
    pub stable: bool,
}

impl Game {
    /// Fetches all supported game versions for Fabric.
    ///
    /// Retrieves a list of all Minecraft versions that are supported by the
    /// Fabric mod loader from the specified mirror.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    ///
    /// # Returns
    ///
    /// Returns a vector of `Game` structs representing supported versions.
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
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/game";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Yarn mapping version for Fabric.
///
/// Contains information about Yarn mappings used for deobfuscating Minecraft
/// code, including the game version, separator, build number, and stability.
///
/// # Fields
///
/// * `game_version` - The Minecraft version this mapping targets
/// * `separator` - The separator used in mapping names
/// * `build` - The build number of this mapping
/// * `maven` - The Maven coordinates for downloading this mapping
/// * `version` - The version string of this mapping
/// * `stable` - Whether this version is considered stable
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Yarn;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let yarns = Yarn::fetch(mirror)?;
///
/// // Find Yarn mappings for a specific version
/// let mc_1_20_yarn = yarns.iter()
///     .filter(|y| y.game_version == "1.20" && y.stable)
///     .next()
///     .unwrap();
/// println!("Yarn for 1.20: {}", mc_1_20_yarn.version);
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Yarn {
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    pub separator: String,
    pub build: i32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

impl Yarn {
    /// Fetches all Yarn mapping versions.
    ///
    /// Retrieves a list of all Yarn mapping versions available from the
    /// specified mirror. Stability is based on the Minecraft version.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    ///
    /// # Returns
    ///
    /// Returns a vector of `Yarn` structs representing available mappings.
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
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/yarn";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Fabric loader version.
///
/// Contains information about Fabric loader versions, including the separator,
/// build number, Maven coordinates, and stability.
///
/// # Fields
///
/// * `separator` - The separator used in version strings
/// * `build` - The build number of this loader
/// * `maven` - The Maven coordinates for downloading this loader
/// * `version` - The version string of this loader
/// * `stable` - Whether this version is considered stable
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Loader;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let loaders = Loader::fetch(mirror)?;
///
/// // Find the latest stable loader
/// let latest_stable = loaders.iter().filter(|l| l.stable).last().unwrap();
/// println!("Latest stable loader: {} (build {})", latest_stable.version, latest_stable.build);
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Loader {
    pub separator: String,
    pub build: i32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

impl Loader {
    /// Fetches all Fabric loader versions.
    ///
    /// Retrieves a list of all Fabric loader versions available from the
    /// specified mirror.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    ///
    /// # Returns
    ///
    /// Returns a vector of `Loader` structs representing available versions.
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
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/loader";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents an intermediary mapping version for Fabric.
///
/// Contains information about intermediary mapping versions, which are used
/// for deobfuscation between obfuscated and deobfuscated code. Stability is based
/// on the Minecraft version.
///
/// # Fields
///
/// * `maven` - The Maven coordinates for downloading this intermediary
/// * `version` - The version string of this intermediary
/// * `stable` - Whether this version is considered stable
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Intermediary;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let intermediaries = Intermediary::fetch(mirror)?;
///
/// // Find the latest stable intermediary
/// let latest_stable = intermediaries.iter().filter(|i| i.stable).last().unwrap();
/// println!("Latest stable intermediary: {}", latest_stable.version);
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Intermediary {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

impl Intermediary {
    /// Fetches all intermediary mapping versions.
    ///
    /// Retrieves a list of all intermediary mapping versions available from
    /// the specified mirror. Stability is based on the Minecraft version.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    ///
    /// # Returns
    ///
    /// Returns a vector of `Intermediary` structs representing available versions.
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
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "v2/versions/intermediary";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents a Fabric installer version.
///
/// Contains information about Fabric installer versions, including the download URL,
/// Maven coordinates, and stability.
///
/// # Fields
///
/// * `url` - The URL to download this installer
/// * `maven` - The Maven coordinates for this installer
/// * `version` - The version string of this installer
/// * `stable` - Whether this version is considered stable
#[derive(Debug, Serialize, Deserialize)]
pub struct Installer {
    pub url: String,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

/// Complete Fabric metadata database.
///
/// Contains all available version information for Fabric, including game versions,
/// mappings, intermediaries, loaders, and installers.
///
/// # Fields
///
/// * `game` - All supported Minecraft game versions
/// * `mappings` - All Yarn mapping versions
/// * `intermediary` - All intermediary mapping versions
/// * `loader` - All Fabric loader versions
/// * `installer` - All Fabric installer versions
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Versions;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let versions = Versions::fetch(mirror)?;
///
/// println!("Supported game versions: {}", versions.game.len());
/// println!("Available loaders: {}", versions.loader.len());
/// println!("Latest stable loader: {}", versions.loader.iter()
///     .filter(|l| l.stable).last().map(|l| &l.version).unwrap());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Versions {
    /// Lists all of the supported game versions.
    pub game: Vec<Game>,
    /// Lists all of the compatible game versions for yarn.
    pub mappings: Vec<Yarn>,
    /// Lists all of the intermediary versions, stable is based of the Minecraft version.
    pub intermediary: Vec<Intermediary>,
    /// Lists all of the loader versions.
    pub loader: Vec<Loader>,
    /// Lists all of the installer.
    pub installer: Vec<Installer>,
}

impl Versions {
    /// Fetches the complete Fabric metadata database.
    ///
    /// Retrieves all available version information from the specified mirror,
    /// including game versions, mappings, intermediaries, loaders, and installers.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    ///
    /// # Returns
    ///
    /// Returns a `Versions` struct containing all metadata.
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
    pub fn fetch(mirror: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned() + "v2/versions";
        FetcherBuilder::fetch(&url).json().execute()?.json()
    }
}

/// Represents game and JVM arguments for Fabric.
///
/// Contains the argument lists that should be passed to the game and JVM
/// when launching Minecraft with Fabric.
///
/// # Fields
///
/// * `game` - Arguments to pass to the Minecraft game process
/// * `jvm` - Arguments to pass to the Java virtual machine
#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

/// Represents a library dependency from a Fabric profile.
///
/// Contains information about a library required by Fabric, including its name,
/// download URL, and various hash values for integrity verification.
///
/// # Fields
///
/// * `name` - The Maven coordinate name of the library (private)
/// * `url` - The base URL for downloading the library
/// * `md5` - Optional MD5 hash for verification
/// * `sha1` - Optional SHA1 hash for verification
/// * `sha256` - Optional SHA256 hash for verification
/// * `sha512` - Optional SHA512 hash for verification
/// * `size` - Optional file size in bytes
///
/// # Conversion
///
/// This struct can be converted to `official::Library` using the `From` trait.
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
/// This helper function transforms a Maven coordinate string (e.g., `group:artifact:version`)
/// into the corresponding file path used in Minecraft's library directory structure.
///
/// # Parameters
///
/// * `name` - The Maven coordinate name (format: `groupId:artifactId:version`)
///
/// # Returns
///
/// Returns the file path in the format: `groupId/path/artifactId/version/artifactId-version.jar`
///
/// # Format Details
///
/// The transformation follows these rules:
/// 1. Split the name by `:` into components
/// 2. The last component is the version
/// 3. The second-to-last component is the artifact ID
/// 4. All preceding components form the group ID
/// 5. Replace `.` with `/` in the group ID
/// 6. Construct the path: `groupId/artifactId/version/artifactId-version.jar`
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

/// Represents a Fabric loader profile JSON for the standard Minecraft launcher.
///
/// This structure contains all the information needed to launch Minecraft with
/// the Fabric mod loader, including game arguments, JVM arguments, and library dependencies.
/// It's designed to be compatible with the standard Minecraft launcher format.
///
/// # Fields
///
/// * `id` - The profile ID (e.g., "fabric-loader-0.15.10-1.20.6")
/// * `inherits_from` - The Minecraft version this profile inherits from
/// * `release_time` - When this profile was released
/// * `time` - When this profile was last updated
/// * `r#type` - The type of profile (typically "release" or "snapshot")
/// * `main_class` - The main class to launch
/// * `arguments` - Game and JVM arguments
/// * `libraries` - Required library dependencies
///
/// # Version String Encoding
///
/// Some characters in version strings are URL-encoded:
/// - Space (` `) becomes `%20`
/// - For example: `1.14 Pre-Release 5` becomes `1.14%20Pre-Release%205`
///
/// # Example
///
/// ```no_run
/// use mc_api::fabric::Profile;
///
/// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
/// let game_version = "1.20.6";
/// let loader_version = "0.15.10";
///
/// let profile = Profile::fetch(mirror, game_version, loader_version)?;
///
/// println!("Profile ID: {}", profile.id);
/// println!("Main class: {}", profile.main_class);
/// println!("Libraries: {}", profile.libraries.len());
/// println!("JVM arguments: {}", profile.arguments.jvm.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Integration with Official Versions
///
/// Fabric profiles implement the `official::MergeVersion` trait, allowing them to be
/// merged with official Minecraft versions for complete modded game installations.
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub time: String,
    pub r#type: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Arguments,
    pub libraries: Vec<Library>,
}

impl Profile {
    /// Fetches a Fabric loader profile for a specific game and loader version.
    ///
    /// This method retrieves the JSON profile that should be used in the standard
    /// Minecraft launcher for launching with Fabric.
    ///
    /// # Parameters
    ///
    /// * `mirror` - The base URL of the Fabric Meta API mirror
    /// * `game_version` - The Minecraft version (e.g., "1.20.6", "1.14 Pre-Release 5")
    /// * `loader_version` - The Fabric loader version (e.g., "0.15.10", "0.14.24")
    ///
    /// # URL Encoding
    ///
    /// Spaces in version strings are URL-encoded as `%20`:
    /// - `"1.14 Pre-Release 5"` becomes `"1.14%20Pre-Release%205"`
    ///
    /// # Returns
    ///
    /// Returns a `Profile` struct containing all the profile information.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid JSON response
    /// - Server returns non-200 status code
    /// - Invalid game or loader version specified
    ///
    /// # Example
    ///
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
    /// # Common Loader Versions
    ///
    /// Common Fabric loader versions include:
    /// - `0.15.10` - Latest stable (as of writing)
    /// - `0.14.24` - Previous stable series
    /// - `0.15.11` - Development versions
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
///
/// This allows Fabric profiles to be merged with official Minecraft versions,
/// creating complete modded game installations.
impl official::MergeVersion for Profile {
    /// Returns the Fabric-specific libraries in a format compatible with official versions.
    ///
    /// This method converts Fabric's `Library` structs to `official::Library` structs,
    /// enabling them to be used alongside official Minecraft libraries.
    ///
    /// # Returns
    ///
    /// Returns a vector of `official::Library` structs representing Fabric's dependencies.
    fn official_libraries(&self) -> Option<Vec<official::Library>> {
        Some(self.libraries.iter().map(|x| x.clone().into()).collect())
    }

    /// Returns the main class for Fabric.
    ///
    /// Fabric uses a custom main class to handle mod loading.
    ///
    /// # Returns
    ///
    /// Returns the Fabric main class name.
    fn main_class(&self) -> Option<String> {
        Some(self.main_class.clone())
    }

    /// Returns game arguments (always `None` for Fabric).
    ///
    /// Fabric handles game arguments internally and doesn't need to merge them.
    ///
    /// # Returns
    ///
    /// Always returns `None`.
    fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
        None
    }

    /// Returns JVM arguments for Fabric.
    ///
    /// Fabric provides custom JVM arguments for mod loading.
    ///
    /// # Returns
    ///
    /// Returns a vector of JVM argument values.
    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
        Some(self.arguments.jvm.clone())
    }
}
