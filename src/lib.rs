//! # Minecraft Launcher Library
//!
//! A Rust-based Minecraft launcher library that handles downloading, installing,
//! and running Minecraft games with Fabric mod support.
//!
//! ## Main Features
//!
//! - **Configuration Management**: Read and write game settings, mod configurations,
//!   and user account information
//! - **Game Installation**: Download and install Minecraft versions including
//!   Fabric loader integration
//! - **Mod Management**: Install, update, and manage mods from Modrinth
//! - **Runtime Execution**: Generate and execute proper Minecraft launch arguments
//!
//! ## Modules
//!
//! - [`config`]: Configuration handling for game settings, mods, and accounts
//! - [`install`]: Minecraft version and library downloading and installation
//! - [`mcargument`]: Launch argument generation for JVM and game
//! - [`modmanage`]: Mod installation, update, and management
//! - [`runtime`]: Minecraft game runtime execution

pub mod config;
pub mod install;
pub mod mcargument;
pub mod modmanage;
pub mod runtime;
