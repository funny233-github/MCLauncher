use crate::config::ConfigHandler;
use std::io;
use std::process::{Command, Stdio};

pub fn gameruntime(handle: ConfigHandler) -> anyhow::Result<()> {
    let args = handle.args_provider()?;
    let path = &handle.config().java_path;
    let mut child = Command::new(path)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    io::copy(&mut child.stdout.take().unwrap(), &mut io::stdout())?;
    child.wait()?;
    Ok(())
}
