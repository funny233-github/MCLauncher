use crate::config::RuntimeConfig;
use anyhow::Result;

pub(super) trait MCInstaller {
    fn install(config: &RuntimeConfig) -> Result<()>;
}
