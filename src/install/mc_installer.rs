use crate::config::ConfigHandler;
use anyhow::Result;

/// Trait for Minecraft installer implementations.
///
/// Provides a uniform interface for installing Minecraft with different mod loaders.
/// Each loader variant (vanilla, Fabric, `NeoForge`) implements this trait to handle
/// its specific installation workflow.
pub(super) trait MCInstaller {
    /// Installs Minecraft game files and loader dependencies.
    ///
    /// # Errors
    /// - `anyhow::Error` if the version manifest cannot be fetched
    /// - `anyhow::Error` if the loader version is not found
    /// - `anyhow::Error` if dependencies cannot be downloaded or installed
    fn install(config: &ConfigHandler) -> Result<()>;
}
