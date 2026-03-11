use crate::fetcher::{FetcherBuilder, FetcherResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Version {
    pub version: Vec<String>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Versioning {
    pub latest: String,
    pub release: String,
    pub versions: Version,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Loader {
    #[serde(rename = "groupId")]
    pub group_id: String,
    #[serde(rename = "artifactId")]
    pub artifact_id: String,
    pub versioning: Versioning,
}

impl Loader {
    /// # Errors
    /// TODO complete docs
    pub fn fetch(mirror: &str) -> Result<Loader> {
        let url = format!("{mirror}/maven-metadata.xml");
        let res: FetcherResult<Loader> = FetcherBuilder::fetch(&url).xml().execute()?;
        res.xml()
    }
}

pub struct Installer {
    pub installer: Vec<u8>,
}

impl Installer {
    /// # Errors
    /// TODO complete docs
    pub fn fetch(mirror: &str, version: &str) -> Result<Installer> {
        let url = format!("{mirror}/{version}/neoforge-{version}-installer.jar");
        let res: FetcherResult<Vec<u8>> = FetcherBuilder::fetch(&url).byte().execute()?;
        Ok(Installer {
            installer: res.byte()?,
        })
    }

    /// # Errors
    /// # Panics
    /// TODO complete docs
    pub fn extract(&self, path: &str) -> Result<()> {
        let mut archive = ZipArchive::new(Cursor::new(self.installer.clone()))?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_file() {
                let path = format!("{path}/{}", entry.name());
                let entry_path = Path::new(&path);
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                fs::create_dir_all(
                    entry_path
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("take parent failed"))?,
                )?;
                fs::write(entry_path, buf)?;
            }
        }

        Ok(())
    }
}
