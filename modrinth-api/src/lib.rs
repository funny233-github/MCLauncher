use reqwest::blocking::Client;
use reqwest::Result;
use serde::{Deserialize, Serialize};

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
        client
            .get(format!(
                "https://api.modrinth.com/v2/project/{}/version",
                slug
            ))
            .send()?
            .json()
    }
    pub async fn fetch(slug: &str) -> Result<Vec<Version>> {
        let client = reqwest::Client::new();
        client
            .get(format!(
                "https://api.modrinth.com/v2/project/{}/version",
                slug
            ))
            .send()
            .await?
            .json()
            .await
    }
}
