use super::install_dependencies;
use super::mc_installer::MCInstaller;
use crate::config::MCLoader;
use crate::config::RuntimeConfig;
use anyhow::Result;
use mc_api::neoforge;
use mc_api::neoforge::Profile;
use mc_api::official::{Version, VersionManifest};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Default)]
pub(super) struct NeoforgeInstaller;

impl MCInstaller for NeoforgeInstaller {
    fn install(config: &RuntimeConfig) -> Result<()> {
        let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
            return Err(anyhow::anyhow!("the loader is not neoforge"));
        };
        println!("fetch neoforge installer.jar");
        let neoforge_jar =
            neoforge::Installer::fetch(&config.mirror.neoforge_neoforge, &neoforge_version)?;
        let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));

        println!("extract neoforge installer.jar");
        neoforge_jar.extract(tmp_dir.to_str().unwrap())?;

        let version_json_file_path = Path::new(&config.game_dir)
            .join("versions")
            .join(&config.game_version)
            .join(config.game_version.clone() + ".json");

        if !version_json_file_path.exists() {
            let version = fetch_version(config)?;
            version.install(&version_json_file_path);
        }

        let native_dir = Path::new(&config.game_dir).join("natives");
        fs::create_dir_all(native_dir).unwrap_or(());

        let mut version_json_file = File::open(version_json_file_path)?;
        let mut content = String::new();
        version_json_file.read_to_string(&mut content)?;

        let version: Version = serde_json::from_str(&content)?;
        install_dependencies(config, &version)?;
        todo!();
        Ok(())
    }
}

fn fetch_version(config: &RuntimeConfig) -> Result<Version> {
    let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
        return Err(anyhow::anyhow!("the loader is not neoforge"));
    };
    let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
    let version_json_file = tmp_dir.join("version.json");
    let profile = fs::read_to_string(version_json_file)?;
    let profile: Profile = serde_json::from_str(&profile)?;

    println!("fetching version manifest...");
    let manifest = VersionManifest::fetch(&config.mirror.version_manifest)?;

    if !manifest.versions.iter().any(|x| x.id == config.vanilla) {
        return Err(anyhow::anyhow!(
            "Cant' find the minecraft version {}",
            &config.game_version
        ));
    }
    println!("fetching version...");
    let mut version = Version::fetch(&manifest, &config.vanilla, &config.mirror.version_manifest)?;
    version.merge(&profile);
    Ok(version)
}
