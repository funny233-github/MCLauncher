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
use std::thread;

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
/// # Panics
/// Panics if the stderr forwarding thread panics, or if joining the thread fails
/// (e.g., due to a panic in the stderr copy loop). This is unlikely under normal operation
/// but can occur if the system is under extreme memory pressure.
pub fn gameruntime(handle: &ConfigHandler) -> anyhow::Result<()> {
    let args = handle.args_provider()?;
    let path = &handle.config().java_path;
    let mut child = Command::new(path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&handle.get_absolute_game_dir()?)
        .spawn()?;

    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;

    let stderr_handle = thread::spawn(move || io::copy(&mut stderr, &mut io::stderr()));
    io::copy(&mut stdout, &mut io::stdout())?;
    stderr_handle.join().unwrap()?;
    child.wait()?;
    Ok(())
}
