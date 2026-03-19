//! # Minecraft Runtime Module
//!
//! This module handles launching and managing the Minecraft game process.
//!
//! ## Main Functionality
//!
//! - **Process Spawning**: Creates a new Java process to run Minecraft
//! - **Argument Generation**: Generates JVM and game launch arguments from configuration
//! - **Output Streaming**: Captures and forwards game output to the console in real-time
//!
//! ## Example
//!
//! ```no_run
//! use gluon::config::ConfigHandler;
//! use gluon::runtime::gameruntime;
//!
//! let config = ConfigHandler::read().expect("Failed to read config");
//! gameruntime(&config).expect("Failed to launch Minecraft");
//! ```

use crate::config::ConfigHandler;
use std::io;
use std::process::{Command, Stdio};

/// Runs the Minecraft game with the provided configuration.
///
/// Generates the appropriate launch arguments and spawns a new process
/// to run Minecraft. Captures and forwards the game's stdout to the console.
///
/// # Errors
/// Returns an error if:
/// - Launch arguments cannot be generated
/// - Java process cannot be spawned
/// - Output cannot be captured
pub fn gameruntime(handle: &ConfigHandler) -> anyhow::Result<()> {
    let args = handle.args_provider()?;
    let path = &handle.config().java_path;
    let mut child = Command::new(path)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    io::copy(
        &mut child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?,
        &mut io::stdout(),
    )?;
    child.wait()?;
    Ok(())
}
