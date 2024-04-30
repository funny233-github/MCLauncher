use super::official;
/// provide related function with minecraft fabric meta api
/// https://github.com/FabricMC/fabric-meta
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::VecDeque};

/// Lists all of the supported game versions.
#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    pub version: String,
    pub stable: bool,
}

impl Game {
    /// fetch fabric all of the supported game versions.
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Game;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let _ = Game::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "/v2/versions/game";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

/// Lists all of the compatible game versions for yarn.
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
    /// fetch all of the yarn versions, stable is based on the Minecraft version.
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Yarn;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let _ = Yarn::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "/v2/versions/yarn";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

/// Lists all of the loader versions.
#[derive(Debug, Serialize, Deserialize)]
pub struct Loader {
    pub separator: String,
    pub build: i32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

impl Loader {
    /// fetch fabric all of the loader versions
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Loader;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let _ = Loader::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "/v2/versions/loader";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

/// Lists all of the intermediary versions, stable is based of the Minecraft version.
#[derive(Debug, Serialize, Deserialize)]
pub struct Intermediary {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

impl Intermediary {
    /// install all of the intermediary versions, stable is based of the Minecraft version.
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Intermediary;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let _ = Intermediary::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Vec<Self>> {
        let url = mirror.to_owned() + "/v2/versions/intermediary";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

/// Lists all of the installer.
#[derive(Debug, Serialize, Deserialize)]
pub struct Installer {
    pub url: String,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

/// Full database, includes all the data.
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
    /// fetch full database, includes all the data
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Versions;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let _ = Versions::fetch(mirror).unwrap();
    /// ```
    pub fn fetch(mirror: &str) -> anyhow::Result<Self> {
        let url = mirror.to_owned() + "/v2/versions";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

/// library that from fabric profile
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
            path: to_path(lib.name.clone()),
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

fn to_path(name: String) -> String {
    let mut name: VecDeque<&str> = name.split(':').collect();
    let version = &name.pop_back().unwrap();
    let file = &name.pop_back().unwrap();
    let mut res = "".to_owned();
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
    assert_eq!(to_path(name), ans);
}

/// return the JSON file that should be used in the standard Minecraft launcher.
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    id: String,
    #[serde(rename = "inheritsFrom")]
    inherits_from: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
    time: String,
    r#type: String,
    #[serde(rename = "mainClass")]
    main_class: String,
    arguments: Arguments,
    libraries: Vec<Library>,
}

impl Profile {
    /// fetch the JSON file that should be used in the standard Minecraft launcher.
    /// # Copy on write
    /// game version and loader version might copy
    /// for example 1.14 Pre-Release 5 becomes 1.14%20Pre-Release%205
    /// # Examples
    /// ```
    /// use launcher::api::fabric::Profile;
    /// use std::borrow::Cow;
    /// let mirror = "https://bmclapi2.bangbang93.com/fabric-meta";
    /// let game_version = Cow::from("1.20.6-rc1");
    /// let loader_version = Cow::from("0.15.10");
    /// let _ = Profile::fetch(mirror, game_version, loader_version).unwrap();
    /// ```
    pub fn fetch(
        mirror: &str,
        game_version: Cow<str>,
        loader_version: Cow<str>,
    ) -> anyhow::Result<Self> {
        let url = mirror.to_owned()
            + "/v2/versions/loader/"
            + game_version.replace(' ', "%20").as_ref()
            + "/"
            + loader_version.replace(' ', "%20").as_ref()
            + "/profile/json";
        let client = reqwest::blocking::Client::new();
        fetch!(client, url, json)
    }
}

impl official::MergeVersion for Profile {
    fn official_libraries(&self) -> Option<Vec<official::Library>> {
        Some(self.libraries.iter().map(|x| x.clone().into()).collect())
    }
    fn main_class(&self) -> Option<String> {
        Some(self.main_class.clone())
    }
    fn arguments_game(&self) -> Option<Vec<serde_json::Value>> {
        None
    }
    fn arguments_jvm(&self) -> Option<Vec<serde_json::Value>> {
        Some(self.arguments.jvm.clone())
    }
}
