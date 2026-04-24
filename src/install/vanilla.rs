use super::install_dependencies;
use super::mc_installer::MCInstaller;
use crate::config::{ConfigHandler, RuntimeConfig};
use anyhow::Result;
use mc_api::official::{Version, VersionManifest};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Installer for vanilla (unmodded) Minecraft.
///
/// Downloads and installs the official Minecraft version without any mod loader.
/// If the version JSON already exists in the game directory, it skips downloading
/// the manifest and proceeds directly to installing dependencies.
#[derive(Default)]
pub(super) struct VanillaInstaller;

impl MCInstaller for VanillaInstaller {
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

/// Fetches the version JSON for a vanilla Minecraft version.
///
/// Downloads the version manifest, validates that the target version exists,
/// and fetches the corresponding version JSON.
///
/// # Errors
/// - `anyhow::Error` if the version manifest cannot be fetched
/// - `anyhow::Error` if the target Minecraft version is not found
/// - `anyhow::Error` if the version JSON cannot be fetched
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
    let version = Version::fetch(&manifest, &config.vanilla, &config.mirror.version_manifest)?;
    Ok(version)
}
