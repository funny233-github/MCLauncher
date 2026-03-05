use crate::fetcher::{FetcherBuilder, FetcherResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Versioning {
    latest: String,
    release: String,
    versions: Vec<String>,
    #[serde(rename = "last_updated")]
    last_updated: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Loader {
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "artifactId")]
    artifact_id: String,
    versioning: Versioning,
}

impl Loader {
    /// # Errors
    /// # Panics
    /// TODO complete docs
    pub fn fetch() -> Result<Loader> {
        let url = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
        let res: FetcherResult<Loader> = FetcherBuilder::fetch(url).xml().execute()?;
        res.xml()
    }
}
