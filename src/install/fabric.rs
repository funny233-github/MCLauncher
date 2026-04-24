use super::install_dependencies;
use super::mc_installer::MCInstaller;
use crate::config::{ConfigHandler, MCLoader, RuntimeConfig};
use anyhow::Result;
use mc_api::{
    fabric::{Loader, Profile},
    official::{Version, VersionManifest},
};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Installer for Fabric-modded Minecraft.
///
/// Downloads the official Minecraft version, merges the Fabric loader profile,
/// and installs all required dependencies. If the version JSON already exists
/// in the game directory, it skips the manifest fetch and proceeds directly
/// to installing dependencies.
pub(super) struct FabricInstaller;

impl MCInstaller for FabricInstaller {
    fn install(config: &ConfigHandler) -> Result<()> {
        let game_dir = config.get_absolute_game_dir()?;
        let version_json_file_path = Path::new(&game_dir)
            .join("versions")
            .join(&config.config().game_version)
            .join(config.config().game_version.clone() + ".json");

        if !version_json_file_path.exists() {
            let version = fetch_version(config.config())?;
            version.install(&version_json_file_path);
        }

        let native_dir = Path::new(&game_dir).join("natives");
        fs::create_dir_all(native_dir).unwrap_or(());

        let mut version_json_file = File::open(version_json_file_path)?;
        let mut content = String::new();
        version_json_file.read_to_string(&mut content)?;

        let version: Version = serde_json::from_str(&content)?;
        install_dependencies(config, &version)?;
        Ok(())
    }
}

/// Fetches the merged version JSON for a Fabric-modded Minecraft version.
///
/// Downloads the official Minecraft version manifest, fetches the base version
/// JSON, retrieves the Fabric loader profile, and merges them together.
///
/// # Errors
/// - `anyhow::Error` if the version manifest cannot be fetched
/// - `anyhow::Error` if the target Minecraft version is not found
/// - `anyhow::Error` if the Fabric loader version is not found
/// - `anyhow::Error` if the Fabric profile cannot be fetched
/// - `anyhow::Error` if the loader is not `MCLoader::Fabric`
fn fetch_version(config: &RuntimeConfig) -> Result<Version> {
    println!("fetching version manifest...");
    let manifest = VersionManifest::fetch(&config.mirror.version_manifest)?;

    if !manifest.versions.iter().any(|x| x.id == config.vanilla) {
        return Err(anyhow::anyhow!(
            "Cannot find the minecraft version {}",
            &config.game_version
        ));
    }

    println!("fetching version...");
    let mut version = Version::fetch(&manifest, &config.vanilla, &config.mirror.version_manifest)?;
    if let MCLoader::Fabric(v) = &config.loader {
        println!("fetching fabric loaders version...");
        let loaders = Loader::fetch(&config.mirror.fabric_meta)?;
        if !loaders.iter().any(|x| &x.version == v) {
            return Err(anyhow::anyhow!("Cannot find the fabric loader version {v}"));
        }
        println!("fetching fabric profile...");
        let profile = Profile::fetch(&config.mirror.fabric_meta, &config.vanilla, v)?;
        version.merge(&profile);
    } else {
        return Err(anyhow::anyhow!("loader is not Fabric"));
    }
    Ok(version)
}
