use super::install_dependencies;
use super::mc_installer::MCInstaller;
use crate::config::RuntimeConfig;
use anyhow::Result;
use mc_api::official::{Version, VersionManifest};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Default)]
pub(super) struct NeoforgeInstaller;

impl MCInstaller for NeoforgeInstaller {
    fn install(config: &RuntimeConfig) -> Result<()> {
        todo!();
        Ok(())
    }
}
