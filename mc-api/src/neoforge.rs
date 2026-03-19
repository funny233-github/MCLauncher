use crate::fetcher::{FetcherBuilder, FetcherResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

/// Available versions for the `NeoForge` loader.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Version {
    /// List of available version strings.
    pub version: Vec<String>,
}

/// Version metadata for the `NeoForge` loader.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Versioning {
    /// The latest version identifier.
    pub latest: String,
    /// The latest release version identifier.
    pub release: String,
    /// Available versions.
    pub versions: Version,
    /// Timestamp of the last metadata update.
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
}

/// Maven metadata describing available `NeoForge` loader versions.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Loader {
    /// Maven group ID.
    #[serde(rename = "groupId")]
    pub group_id: String,
    /// Maven artifact ID.
    #[serde(rename = "artifactId")]
    pub artifact_id: String,
    /// Version information.
    pub versioning: Versioning,
}

impl Loader {
    /// Fetches `NeoForge` loader metadata from a Maven mirror.
    ///
    /// The mirror URL should point to the `NeoForge` Maven repository. The function
    /// retrieves the `maven-metadata.xml` file and parses it into a `Loader` struct.
    ///
    /// # Example
    /// ```
    /// # use mc_api::neoforge::Loader;
    /// let loader = Loader::fetch("https://maven.neoforged.net/releases/net/neoforged/neoforge").unwrap();
    /// println!("Latest version: {}", loader.versioning.latest);
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails, the XML is malformed, or parsing fails.
    pub fn fetch(mirror: &str) -> Result<Loader> {
        let url = format!("{mirror}/maven-metadata.xml");
        let res: FetcherResult<Loader> = FetcherBuilder::fetch(&url).xml().execute()?;
        res.xml()
    }
}

/// A `NeoForge` installer JAR file fetched from a Maven repository.
pub struct Installer {
    /// Raw bytes of the installer JAR.
    pub installer: Vec<u8>,
}

impl Installer {
    /// Fetches a specific `NeoForge` installer version from a Maven mirror.
    ///
    /// The mirror URL should point to the `NeoForge` Maven repository. The function
    /// downloads the installer JAR for the specified version.
    ///
    /// # Example
    /// ```
    /// # use mc_api::neoforge::Installer;
    /// let installer = Installer::fetch("https://maven.neoforged.net/releases/net/neoforged/neoforge", "21.0.0-beta").unwrap();
    /// println!("Downloaded {} bytes", installer.installer.len());
    /// ```
    ///
    /// # Errors
    /// Returns an error if the network request fails or the file cannot be downloaded.
    pub fn fetch(mirror: &str, version: &str) -> Result<Installer> {
        let url = format!("{mirror}/{version}/neoforge-{version}-installer.jar");
        let res: FetcherResult<Vec<u8>> = FetcherBuilder::fetch(&url).byte().execute()?;
        Ok(Installer {
            installer: res.byte()?,
        })
    }

    /// Extracts the installer JAR contents to the specified path.
    ///
    /// Creates a Zip archive from the installer data and extracts all files
    /// to the target directory, creating necessary subdirectories as needed.
    /// Only files are extracted; directory entries are skipped.
    ///
    /// # Example
    /// ```no_run
    /// # use mc_api::neoforge::Installer;
    /// # let installer = Installer { installer: vec![0] };
    /// installer.extract("/tmp/neoforge-install")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the JAR file is corrupted or invalid, directory creation fails, or file writing fails.
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
