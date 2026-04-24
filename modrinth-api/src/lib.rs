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

/// Hash values for file integrity verification.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hashes {
    /// SHA1 hash.
    pub sha1: String,
    /// SHA512 hash.
    pub sha512: String,
}

/// A downloadable file for a specific version.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionFile {
    /// File name.
    pub filename: String,
    /// Hash information for verification.
    pub hashes: Hashes,
    /// Download URL.
    pub url: String,
}

/// A specific version of a Minecraft project.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Version {
    /// Display name (e.g., "1.0.0").
    pub name: String,
    /// Version number string.
    pub version_number: String,
    /// Supported Minecraft game versions.
    pub game_versions: Vec<String>,
    /// Release type (e.g., "release", "beta", "alpha").
    pub version_type: String,
    /// Supported mod loaders (e.g., "fabric", "forge").
    pub loaders: Vec<String>,
    /// Downloadable files.
    pub files: Vec<VersionFile>,
}

impl Version {
    /// Checks if this version supports a specific Minecraft game version.
    ///
    /// Returns true if the specified game version is in the supported versions list.
    ///
    /// # Example
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
    /// Returns true if the specified mod loader is in the supported loaders list.
    ///
    /// # Example
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

/// Fetches version information from Modrinth.
#[derive(Debug, Deserialize, Serialize)]
pub struct Versions {}

impl Versions {
    /// Fetches versions for a project by slug (blocking).
    ///
    /// Retrieves all available versions for the specified project slug.
    /// Uses up to 5 retry attempts for failed requests.
    ///
    /// # Example
    /// ```
    /// use modrinth_api::Versions;
    /// let versions = Versions::fetch_blocking("fabric-api").unwrap();
    /// assert!(versions.len() > 0);
    /// ```
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails after retries, the response
    /// cannot be parsed, or the project slug does not exist.
    pub fn fetch_blocking(slug: &str) -> Result<Vec<Version>> {
        let client = Client::new();
        let mut err_detail = None;
        for _ in 0..5 {
            let url = reqwest::Url::parse("https://api.modrinth.com/v2/project/")
                .and_then(|base| base.join(&(slug.to_owned() + "/version")))
                .map_err(|e| anyhow::anyhow!("invalid slug: {e}"))?;
            let init = client
                .get(url)
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

    /// Fetches versions for a project by slug (async).
    ///
    /// Retrieves all available versions for the specified project slug.
    /// Uses up to 5 retry attempts for failed requests.
    ///
    /// # Example
    /// ```
    /// use modrinth_api::Versions;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let versions = Versions::fetch("fabric-api").await.unwrap();
    ///     assert!(versions.len() > 0);
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails after retries, the response
    /// cannot be parsed, or the project slug does not exist.
    pub async fn fetch(slug: &str) -> Result<Vec<Version>> {
        let url = reqwest::Url::parse("https://api.modrinth.com/v2/project/")
            .and_then(|base| base.join(&(slug.to_owned() + "/version")))
            .map_err(|e| anyhow::anyhow!("invalid slug: {e}"))?;
        let client = reqwest::Client::new();
        let mut err_detail = None;
        for _ in 0..5 {
            let init = client
                .get(url.clone())
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

/// A search result from Modrinth.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hit {
    /// Unique project identifier.
    pub project_id: String,
    /// Project type (e.g., "mod", "modpack", "resourcepack").
    pub project_type: String,
    /// URL-friendly identifier.
    pub slug: String,
    /// Author username.
    pub author: String,
    /// Display title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Display categories.
    pub display_categories: Option<Vec<String>>,
    /// Version IDs.
    pub versions: Vec<String>,
    /// Follower count.
    pub follows: i32,
    /// Creation date (ISO 8601).
    pub date_created: String,
    /// Latest version ID.
    pub latest_version: Option<String>,
    /// License identifier.
    pub license: String,
    /// Gallery image URLs.
    pub gallery: Option<Vec<String>>,
    /// Featured gallery image URL.
    pub featured_gallery: Option<String>,
}

impl Hit {
    /// Checks if this project is a mod.
    ///
    /// # Example
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

    /// Returns the project slug (name).
    ///
    /// # Example
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

    /// Checks if this project supports a specific version ID.
    ///
    /// Note: This checks against version IDs stored in the project,
    /// not game version strings. Use the `Versions` module for
    /// accurate game version compatibility checking.
    ///
    /// # Example
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

/// Paginated search results from Modrinth.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Projects {
    /// Matching projects.
    pub hits: Vec<Hit>,
    /// Page offset.
    pub offset: i32,
    /// Maximum results per page.
    pub limit: i32,
    /// Total matching results.
    pub total_hits: i32,
}

impl Projects {
    /// Searches for projects matching the query (blocking).
    ///
    /// Searches Modrinth for projects matching the query string.
    /// Uses up to 5 retry attempts for failed requests.
    /// The limit parameter must be <= 100 (defaults to 10).
    ///
    /// # Example
    /// ```
    /// use modrinth_api::Projects;
    /// let projects = Projects::fetch_blocking("fabric-api", Some(10)).unwrap();
    /// assert!(projects.hits.len() > 0);
    /// ```
    ///
    /// # Errors
    /// Returns an error if the limit exceeds 100, the HTTP request fails
    /// after retries, or the response cannot be parsed.
    pub fn fetch_blocking(query: &str, limit: Option<usize>) -> Result<Projects> {
        let client = Client::new();
        let mut err_detail = None;
        if limit.is_some_and(|lim| lim > 100) {
            return Err(anyhow::anyhow!("limit must be <= 100, got: {limit:?}"));
        }
        for _ in 0..5 {
            let init = client
                .get("https://api.modrinth.com/v2/search")
                .query(&[
                    ("query", query),
                    ("limit", &limit.unwrap_or(10).to_string()),
                ])
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

    /// Searches for projects matching the query (async).
    ///
    /// Searches Modrinth for projects matching the query string.
    /// Uses up to 5 retry attempts for failed requests.
    /// The limit parameter must be <= 100 (defaults to 10).
    ///
    /// # Example
    /// ```
    /// use modrinth_api::Projects;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let projects = Projects::fetch("fabric-api", Some(10)).await.unwrap();
    ///     assert!(projects.hits.len() > 0);
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns an error if the limit exceeds 100, the HTTP request fails
    /// after retries, or the response cannot be parsed.
    pub async fn fetch(query: &str, limit: Option<usize>) -> Result<Projects> {
        let client = reqwest::Client::new();
        let mut err_detail = None;
        if limit.is_some_and(|lim| lim > 100) {
            return Err(anyhow::anyhow!("limit must be <= 100, got: {limit:?}"));
        }
        for _ in 0..5 {
            let init = client
                .get("https://api.modrinth.com/v2/search")
                .query(&[
                    ("query", query),
                    ("limit", &limit.unwrap_or(10).to_string()),
                ])
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
