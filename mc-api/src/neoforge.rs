use crate::fetcher::{FetcherBuilder, FetcherResult};
use crate::official;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
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

/// Game and JVM arguments for Neoforge.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Arguments {
    /// Arguments to pass to the Minecraft game process.
    pub game: Vec<serde_json::Value>,
    /// Arguments to pass to the Java virtual machine.
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Artifact {
    pub url: String,
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<i32>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Downloads {
    pub artifact: Artifact,
}

/// Library dependency from a Neoforge profile.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Downloads,
}

impl From<Library> for official::Library {
    fn from(lib: Library) -> Self {
        let artifact = official::Artifact {
            path: lib.downloads.artifact.path,
            sha1: lib.downloads.artifact.sha1,
            size: lib.downloads.artifact.size,
            url: lib.downloads.artifact.url,
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
/// # Panics
/// TODO complete docs
#[must_use]
pub fn to_path(name: &str) -> String {
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

/// Neoforge loader profile JSON for the standard Minecraft launcher.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    /// Profile ID (e.g., "neoforge-21.11.0-beta").
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

/// Implementation of `official::MergeVersion` for `Profile`.
impl official::MergeVersion for Profile {
    fn official_libraries(&self) -> Option<Vec<official::Library>> {
        Some(self.libraries.iter().map(|x| x.clone().into()).collect())
    }

    fn main_class(&self) -> Option<String> {
        Some(self.main_class.clone())
    }

    fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
        Some(self.arguments.game.clone())
    }

    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
        Some(self.arguments.jvm.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Processor {
    pub sides: Option<Vec<String>>,
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataMapValue {
    pub client: String,
    pub server: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstallerProfile {
    pub spec: usize,
    pub profile: String,
    pub version: String,
    pub icon: String,
    pub minecraft: String,
    pub json: String,
    pub logo: String,
    pub welcome: String,
    #[serde(rename = "mirrorList")]
    pub mirror_list: String,
    #[serde(rename = "hideExtract")]
    pub hide_extract: bool,
    pub data: HashMap<String, DataMapValue>,
    pub processors: Vec<Processor>,
    pub libraries: Vec<Library>,
    #[serde(rename = "serverJarPath")]
    pub server_jar_path: String,
}
