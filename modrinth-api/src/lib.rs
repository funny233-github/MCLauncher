//! # `modrinth_api`
//!
//! A Rust library for interacting with the Modrinth API v2.
//!
//! This library provides a simple and efficient way to search for Minecraft mods
//! and retrieve version information from Modrinth, the largest Minecraft mod repository.
//!
//! ## Features
//!
//! - Search for projects by query string
//! - Fetch version information for any project
//! - Check version compatibility with game versions and loaders
//! - Both synchronous and asynchronous API support
//! - Automatic retry mechanism for failed requests
//!
//! ## Example
//!
//! ```no_run
//! use modrinth_api::{Projects, Versions};
//!
//! fn main() -> anyhow::Result<()> {
//!     // Search for mods
//!     let projects = Projects::fetch_blocking("fabric-api", Some(5))?;
//!     for hit in &projects.hits {
//!         println!("{}: {}", hit.title, hit.description);
//!     }
//!
//!     // Get versions for a specific project
//!     let versions = Versions::fetch_blocking("fabric-api")?;
//!     for version in &versions {
//!         println!("Version {}: {} files", version.name, version.files.len());
//!     }
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Represents file hash information for a version file.
///
/// This structure contains the SHA1 and SHA512 hashes of a file,
/// which are used for file integrity verification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hashes {
    /// The SHA1 hash of the file
    pub sha1: String,
    /// The SHA512 hash of the file
    pub sha512: String,
}

/// Represents a file associated with a specific version of a project.
///
/// Each version may contain multiple files (e.g., different downloads for
/// different environments or additional resources).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionFile {
    /// The filename of the file
    pub filename: String,
    /// Hash information for file integrity verification
    pub hashes: Hashes,
    /// The URL from which the file can be downloaded
    pub url: String,
}

/// Represents a specific version of a Minecraft mod or project.
///
/// A version contains all the information about a particular release of a mod,
/// including which game versions and mod loaders it supports, as well as
/// the actual files that can be downloaded.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Version {
    /// The display name of the version (e.g., "1.0.0")
    pub name: String,
    /// The version number string
    pub version_number: String,
    /// List of Minecraft game versions this version supports
    pub game_versions: Vec<String>,
    /// The type of version (e.g., "release", "beta", "alpha")
    pub version_type: String,
    /// List of mod loaders this version supports (e.g., "fabric", "forge")
    pub loaders: Vec<String>,
    /// List of downloadable files for this version
    pub files: Vec<VersionFile>,
}

impl Version {
    /// Checks if this version supports a specific Minecraft game version.
    ///
    /// # Arguments
    ///
    /// * `game_version` - The game version to check (e.g., "1.20.4")
    ///
    /// # Returns
    ///
    /// `true` if the version supports the specified game version, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use modrinth_api::Version;
    /// # let version = Version {
    /// #     name: String::from("1.0.0"),
    /// #     version_number: String::from("1.0.0"),
    /// #     game_versions: vec![String::from("1.20.4"), String::from("1.20.5")],
    /// #     version_type: String::from("release"),
    /// #     loaders: vec![String::from("fabric")],
    /// #     files: vec![],
    /// # };
    /// assert!(version.is_support_game_version("1.20.4"));
    /// assert!(!version.is_support_game_version("1.19.2"));
    /// ```
    #[must_use]
    pub fn is_support_game_version(&self, game_version: &str) -> bool {
        self.game_versions.iter().any(|v| v == game_version)
    }

    /// Checks if this version supports a specific mod loader.
    ///
    /// # Arguments
    ///
    /// * `game_loader` - The mod loader to check (e.g., "fabric", "forge")
    ///
    /// # Returns
    ///
    /// `true` if the version supports the specified loader, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use modrinth_api::Version;
    /// # let version = Version {
    /// #     name: String::from("1.0.0"),
    /// #     version_number: String::from("1.0.0"),
    /// #     game_versions: vec![String::from("1.20.4")],
    /// #     version_type: String::from("release"),
    /// #     loaders: vec![String::from("fabric"), String::from("quilt")],
    /// #     files: vec![],
    /// # };
    /// assert!(version.is_support_loader("fabric"));
    /// assert!(!version.is_support_loader("forge"));
    /// ```
    #[must_use]
    pub fn is_support_loader(&self, game_loader: &str) -> bool {
        self.loaders.iter().any(|l| l == game_loader)
    }
}

/// Provides methods to fetch version information from Modrinth.
///
/// This struct acts as a namespace for version-related operations,
/// offering both synchronous and asynchronous methods to retrieve
/// version data for projects.
#[derive(Debug, Deserialize, Serialize)]
pub struct Versions {}

impl Versions {
    /// Fetches the list of versions for a project with the given slug.
    ///
    /// This is a synchronous blocking operation that will wait for the HTTP request
    /// to complete before returning. The method implements an automatic retry mechanism
    /// with up to 5 attempts.
    ///
    /// # Arguments
    ///
    /// * `slug` - The project slug (unique identifier) on Modrinth
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a vector of `Version` objects if successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails after 5 retry attempts
    /// - The response cannot be parsed as JSON
    /// - The project slug does not exist
    /// - Network connectivity issues occur
    ///
    /// # Examples
    ///
    /// ```
    /// use modrinth_api::Versions;
    /// let versions = Versions::fetch_blocking("fabric-api").unwrap();
    /// assert!(versions.len() > 0);
    /// ```
    pub fn fetch_blocking(slug: &str) -> Result<Vec<Version>> {
        let client = Client::new();
        let mut err_detail = None;
        for _ in 0..5 {
            let init = client
                .get(format!(
                    "https://api.modrinth.com/v2/project/{slug}/version"
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));
            let Ok(send) = init.send() else {
                std::thread::sleep(Duration::from_secs(10));
                continue;
            };
            match send.json() {
                Ok(json) => return Ok(json),
                Err(e) => {
                    err_detail = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "modrinth Versions fetch timeout!\ndetail:{err_detail:#?}",
        ))
    }

    /// Fetches the list of versions for a project with the given slug.
    ///
    /// This is an asynchronous non-blocking operation. The method implements an
    /// automatic retry mechanism with up to 5 attempts. Must be awaited in an
    /// async context.
    ///
    /// # Arguments
    ///
    /// * `slug` - The project slug (unique identifier) on Modrinth
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a vector of `Version` objects if successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails after 5 retry attempts
    /// - The response cannot be parsed as JSON
    /// - The project slug does not exist
    /// - Network connectivity issues occur
    ///
    /// # Examples
    ///
    /// ```
    /// use modrinth_api::Versions;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let versions = Versions::fetch("fabric-api").await.unwrap();
    ///     assert!(versions.len() > 0);
    /// }
    /// ```
    pub async fn fetch(slug: &str) -> Result<Vec<Version>> {
        let client = reqwest::Client::new();
        let mut err_detail = None;
        for _ in 0..5 {
            let init = client
                .get(format!(
                    "https://api.modrinth.com/v2/project/{slug}/version"
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));
            let Ok(send) = init.send().await else {
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            };

            match send.json().await {
                Ok(json) => return Ok(json),
                Err(e) => {
                    err_detail = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "modrinth Versions fetch timeout!\ndetail:{err_detail:#?}",
        ))
    }
}

/// Represents a single search result hit from Modrinth.
///
/// This structure contains metadata about a project that appears in search results,
/// including its title, description, author, version information, and more.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hit {
    /// The unique project identifier
    pub project_id: String,
    /// The type of project (e.g., "mod", "modpack", "resourcepack")
    pub project_type: String,
    /// The project slug (URL-friendly identifier)
    pub slug: String,
    /// The username of the project author
    pub author: String,
    /// The display title of the project
    pub title: String,
    /// A short description of the project
    pub description: String,
    /// Categories the project is displayed in
    pub display_categories: Option<Vec<String>>,
    /// List of version IDs for this project
    pub versions: Vec<String>,
    /// Number of users following this project
    pub follows: i32,
    /// ISO 8601 date string when the project was created
    pub date_created: String,
    /// The ID of the latest version
    pub latest_version: Option<String>,
    /// The license identifier for the project
    pub license: String,
    /// List of gallery image URLs
    pub gallery: Option<Vec<String>>,
    /// URL of the featured gallery image
    pub featured_gallery: Option<String>,
}

impl Hit {
    /// Checks if this project is a mod.
    ///
    /// # Returns
    ///
    /// `true` if the project type is "mod", `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use modrinth_api::Hit;
    /// # let mut hit = Hit {
    /// #     project_id: String::from("test"),
    /// #     project_type: String::from("mod"),
    /// #     slug: String::from("test-mod"),
    /// #     author: String::from("test"),
    /// #     title: String::from("Test Mod"),
    /// #     description: String::from("A test mod"),
    /// #     display_categories: None,
    /// #     versions: vec![],
    /// #     follows: 0,
    /// #     date_created: String::from("2024-01-01"),
    /// #     latest_version: None,
    /// #     license: String::from("MIT"),
    /// #     gallery: None,
    /// #     featured_gallery: None,
    /// # };
    /// assert!(hit.is_mod());
    /// hit.project_type = String::from("modpack");
    /// assert!(!hit.is_mod());
    /// ```
    #[must_use]
    pub fn is_mod(&self) -> bool {
        self.project_type == "mod"
    }

    /// Returns the slug (name) of the project.
    ///
    /// # Returns
    ///
    /// A cloned copy of the project's slug string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use modrinth_api::Hit;
    /// # let hit = Hit {
    /// #     project_id: String::from("test"),
    /// #     project_type: String::from("mod"),
    /// #     slug: String::from("fabric-api"),
    /// #     author: String::from("test"),
    /// #     title: String::from("Test Mod"),
    /// #     description: String::from("A test mod"),
    /// #     display_categories: None,
    /// #     versions: vec![],
    /// #     follows: 0,
    /// #     date_created: String::from("2024-01-01"),
    /// #     latest_version: None,
    /// #     license: String::from("MIT"),
    /// #     gallery: None,
    /// #     featured_gallery: None,
    /// # };
    /// assert_eq!(hit.name(), "fabric-api");
    /// ```
    #[must_use]
    pub fn name(&self) -> String {
        self.slug.clone()
    }

    /// Checks if this project supports a specific game version.
    ///
    /// Note: This checks against the version IDs stored in the project,
    /// not the actual version data. For more accurate checking,
    /// use the `Versions` module to fetch full version information.
    ///
    /// # Arguments
    ///
    /// * `version` - The version ID to check
    ///
    /// # Returns
    ///
    /// `true` if the version ID exists in the project's versions list,
    /// `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use modrinth_api::Hit;
    /// # let hit = Hit {
    /// #     project_id: String::from("test"),
    /// #     project_type: String::from("mod"),
    /// #     slug: String::from("test-mod"),
    /// #     author: String::from("test"),
    /// #     title: String::from("Test Mod"),
    /// #     description: String::from("A test mod"),
    /// #     display_categories: None,
    /// #     versions: vec![String::from("abc123"), String::from("def456")],
    /// #     follows: 0,
    /// #     date_created: String::from("2024-01-01"),
    /// #     latest_version: None,
    /// #     license: String::from("MIT"),
    /// #     gallery: None,
    /// #     featured_gallery: None,
    /// # };
    /// assert!(hit.is_support_game_version("abc123"));
    /// assert!(!hit.is_support_game_version("xyz789"));
    /// ```
    #[must_use]
    pub fn is_support_game_version(&self, version: &str) -> bool {
        self.versions.iter().any(|v| v == version)
    }
}

/// Represents a paginated list of search results from Modrinth.
///
/// This structure contains the search results along with pagination
/// information, allowing clients to navigate through large result sets.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Projects {
    /// The list of matching projects
    pub hits: Vec<Hit>,
    /// The offset of the current page
    pub offset: i32,
    /// The maximum number of results per page
    pub limit: i32,
    /// The total number of matching results
    pub total_hits: i32,
}

impl Projects {
    /// Searches for projects matching the given query.
    ///
    /// This is a synchronous blocking operation that searches Modrinth's database
    /// for projects matching the provided query string. The method implements an
    /// automatic retry mechanism with up to 5 attempts.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string (can be a project name, keyword, etc.)
    /// * `limit` - Optional maximum number of results to return (must be <= 100).
    ///   If `None`, defaults to 10.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a `Projects` object with the search results.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `limit` parameter is greater than 100
    /// - The HTTP request fails after 5 retry attempts
    /// - The response cannot be parsed as JSON
    /// - Network connectivity issues occur
    ///
    /// # Examples
    ///
    /// ```
    /// use modrinth_api::Projects;
    /// let projects = Projects::fetch_blocking("fabric-api", Some(10)).unwrap();
    /// assert!(projects.hits.len() > 0);
    /// ```
    pub fn fetch_blocking(query: &str, limit: Option<usize>) -> Result<Projects> {
        let client = Client::new();
        let mut err_detail = None;
        if limit.is_some_and(|lim| lim > 100) {
            return Err(anyhow::anyhow!("limit must < 100, the limit is {limit:?}",));
        }
        for _ in 0..5 {
            let init = client
                .get(format!(
                    "https://api.modrinth.com/v2/search?query={}&limit={}",
                    query,
                    limit.unwrap_or(10)
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));

            let Ok(send) = init.send() else {
                std::thread::sleep(Duration::from_secs(10));
                continue;
            };

            match send.json() {
                Ok(json) => return Ok(json),
                Err(e) => {
                    err_detail = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "modrinth Projects fetch timeout!\ndetail:{:#?}",
            &err_detail
        ))
    }

    /// Searches for projects matching the given query.
    ///
    /// This is an asynchronous non-blocking operation. The method implements an
    /// automatic retry mechanism with up to 5 attempts. Must be awaited in an
    /// async context.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string (can be a project name, keyword, etc.)
    /// * `limit` - Optional maximum number of results to return (must be <= 100).
    ///   If `None`, defaults to 10.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a `Projects` object with the search results.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `limit` parameter is greater than 100
    /// - The HTTP request fails after 5 retry attempts
    /// - The response cannot be parsed as JSON
    /// - Network connectivity issues occur
    ///
    /// # Examples
    ///
    /// ```
    /// use modrinth_api::Projects;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let projects = Projects::fetch("fabric-api", Some(10)).await.unwrap();
    ///     assert!(projects.hits.len() > 0);
    /// }
    /// ```
    pub async fn fetch(query: &str, limit: Option<usize>) -> Result<Projects> {
        let client = reqwest::Client::new();
        let mut err_detail = None;
        if limit.is_some_and(|lim| lim > 100) {
            return Err(anyhow::anyhow!("limit must < 100, the limit is {limit:?}",));
        }
        for _ in 0..5 {
            let init = client
                .get(format!(
                    "https://api.modrinth.com/v2/search?query={}&limit={}",
                    query,
                    limit.unwrap_or(10)
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));

            let Ok(send) = init.send().await else {
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            };

            match send.json().await {
                Ok(json) => return Ok(json),
                Err(e) => {
                    err_detail = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "modrinth Projects fetch timeout!\ndetail:{:#?}",
            &err_detail
        ))
    }
}
