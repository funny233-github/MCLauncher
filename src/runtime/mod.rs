use crate::config::RuntimeConfig;
use std::io;
use std::process::{Command, Stdio};

pub fn gameruntime(config: RuntimeConfig) -> anyhow::Result<()> {
    let args = config.args_provider()?;
    let path = config.java_path;
    let mut child = Command::new(path)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    io::copy(&mut child.stdout.take().unwrap(), &mut io::stdout())?;
    child.wait()?;
    Ok(())
}
