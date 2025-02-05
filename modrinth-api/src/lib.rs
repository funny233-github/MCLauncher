use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionFile {
    pub filename: String,
    pub hashes: Hashes,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Version {
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Versions {}

impl Versions {
    /// Fetches the list of versions for a project with the given slug.
    /// # Examples
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
                    "https://api.modrinth.com/v2/project/{}/version",
                    slug
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));
            let send = if let Ok(_send) = init.send() {
                _send
            } else {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };
            match send.json() {
                Ok(_json) => return Ok(_json),
                Err(e) => {
                    err_detail = Some(e);
                    continue;
                }
            };
        }

        Err(anyhow::anyhow!(
            "modrinth Versions fetch timeout!\ndetail:{:#?}",
            err_detail
        ))
    }

    pub async fn fetch(slug: &str) -> Result<Vec<Version>> {
        let client = reqwest::Client::new();
        let mut err_detail = None;
        for _ in 0..5 {
            let init = client
                .get(format!(
                    "https://api.modrinth.com/v2/project/{}/version",
                    slug
                ))
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(Duration::from_secs(100));
            let send = if let Ok(_send) = init.send().await {
                _send
            } else {
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            };
            match send.json().await {
                Ok(_json) => return Ok(_json),
                Err(e) => {
                    err_detail = Some(e);
                    continue;
                }
            };
        }

        Err(anyhow::anyhow!(
            "modrinth Versions fetch timeout!\ndetail:{:#?}",
            err_detail
        ))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hit {
    pub project_id: String,
    pub author: String,
    pub display_categories: Option<Vec<String>>,
    pub versions: Vec<String>,
    pub follows: i32,
    pub date_created: String,
    pub latest_version: Option<String>,
    pub license: String,
    pub gallery: Option<Vec<String>>,
    pub featured_gallery: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Projects {
    pub hits: Vec<Hit>,
    pub offset: i32,
    pub limit: i32,
    pub total_hits: i32,
}

impl Projects {
    /// Fetches the list of Projects for a project with the given slug and limit.
    /// # Examples
    /// ```
    /// use modrinth_api::Projects;
    /// let projects = Projects::fetch_blocking("fabric-api", Some(10)).unwrap();
    /// assert!(projects.hits.len() > 0);
    /// ```
    pub fn fetch_blocking(query: &str, limit: Option<usize>) -> Result<Projects> {
        let client = Client::new();
        let mut err_detail = None;
        if limit.is_some_and(|lim| lim > 100) {
            return Err(anyhow::anyhow!(
                "limit must < 100, the limit is {:?}",
                limit
            ));
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

            let send = if let Ok(_send) = init.send() {
                _send
            } else {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };

            match send.json() {
                Ok(_json) => return Ok(_json),
                Err(e) => {
                    err_detail = Some(e);
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!(
            "modrinth Projects fetch timeout!\ndetail:{:#?}",
            &err_detail
        ))
    }
}
