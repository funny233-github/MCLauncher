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
                .timeout(Duration::from_secs(10));
            let send = if let Ok(_send) = init.send() {
                _send
            } else {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };
            if let Ok(_json) = send.json() {
                return Ok(_json);
            } else {
                continue;
            }
        }

        Err(anyhow::anyhow!("modrinth Versions fetch timeout!"))
    }

    pub async fn fetch(slug: &str) -> Result<Vec<Version>> {
        let client = reqwest::Client::new();
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
                .timeout(Duration::from_secs(10));
            let send = if let Ok(_send) = init.send().await {
                _send
            } else {
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            };
            if let Ok(_json) = send.json().await {
                return Ok(_json);
            } else {
                continue;
            }
        }

        Err(anyhow::anyhow!("modrinth Versions fetch timeout!"))
    }
}
