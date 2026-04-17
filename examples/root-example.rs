use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

fn find_config_up(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let config_path = current.join("config.toml");
        if config_path.exists() {
            return Ok(config_path);
        }

        current = current
            .parent()
            .context("reached filesystem root without finding config.toml")?
            .to_path_buf();
    }
}

fn main() -> Result<()> {
    let root = find_config_up(&std::env::current_dir()?)?;
    println!("{}", root.display());
    Ok(())
}
