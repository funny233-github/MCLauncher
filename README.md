# MCLauncher

A modern, efficient Minecraft launcher written in Rust. This tool provides a command-line interface to launch and manage different versions of Minecraft with support for mod installation, Microsoft OAuth authentication, and more.

## Version

Current version: **0.4.1**

## Introduction

Welcome to MCLauncher! This tool is designed to make it easy to launch and manage different versions of Minecraft. With just a few commands, you can create and manage Minecraft directories, set up configurations, download and install different versions of Minecraft, authenticate with Microsoft accounts, and install additional mods from Modrinth.

## Key Features

- **Microsoft OAuth Authentication**: Securely authenticate with your Microsoft account for official Minecraft servers
- **Offline Mode**: Play with offline accounts when internet is not available
- **Mod Management**: Seamless integration with Modrinth for installing and managing mods
- **Multiple Minecraft Versions**: Support for various Minecraft versions and loaders
- **Fabric Loader Support**: Easy installation of Fabric mods with different loader versions
- **Download Mirrors**: Choose from multiple download mirrors for faster downloads
- **Cross-platform**: Written in Rust for excellent performance on all platforms

## Getting Started

To get started with MCLauncher, follow these steps:

### Installation

**From source:**

```bash
git clone <repository-url>
cd MCLauncher
cargo install --path .
```

**Using cargo:**

```bash
cargo install launcher
```

### Basic Setup

1. **Create a New Directory**: Initialize a new directory to store Minecraft files using the `launcher init` command.
2. **Set Up Configurations**: Run `launcher config` within the directory to set up configurations.
3. **Authenticate with Your Account**:
   - For Microsoft accounts: `launcher account microsoft`
   - For offline accounts: `launcher account offline <username>`
4. **Explore Available Versions**: Explore available versions using `launcher list <version_type>`.
5. **Select a Mirror**: Specify a download mirror via `launcher mirror <mirror>`.
6. **Install Minecraft**: Install a specific version of Minecraft using the `launcher install <version>` command.
7. **Install with Fabric Loader**: Install Minecraft along with the Fabric Loader using the `launcher install <version> --fabric <fabric_loader_version>` command.
8. **Run Minecraft**: Launch the game with `launcher run`
9. **Access Help**: For more commands and details, type `launcher help`.

### Microsoft OAuth Authentication

MCLauncher now supports full Microsoft OAuth authentication for playing on official Minecraft servers:

```bash
launcher account microsoft
```

This will:

1. Display a code and URL for device authentication
2. Poll for token authentication
3. Authenticate with Xbox Live
4. Get XSTS token
5. Authenticate with Minecraft
6. Fetch your Minecraft profile

All authentication tokens are securely stored in your local configuration file.

## Mod Management

MCLauncher provides comprehensive mod management capabilities through Modrinth integration:

### Prerequisites

Ensure you have installed Fabric via:

```bash
launcher install <minecraft_version> --fabric <fabric_loader_version>
```

### Mod Management Commands

1. **Find a Mod**: Search for mods on Modrinth, the Mod name is always at the end of the URL (e.g., `https://modrinth.com/mod/fabric-api` → `fabric-api`)
2. **Add a Mod**: `launcher mod add <mod_name>`
3. **Remove a Mod**: `launcher mod remove <mod_name>`
4. **Update All Mods**: Update all mods listed in `config.toml` with `launcher mod update`
5. **Sync All Mods**: Sync all mods from `config.toml` with `launcher mod sync`
6. **Install All Mods**: Install all mods from `config.lock` with `launcher mod install`
7. **Clean Unused Mods**: Remove all mods not in `config.toml` with `launcher mod clean`
8. **Search for Mods**: Search for related mods with `launcher mod search <name>`

### Example Workflow

```bash
# Install Minecraft with Fabric
launcher install 1.20.4 --fabric 0.15.11

# Search for a mod
launcher mod search sodium

# Add mods
launcher mod add fabric-api
launcher mod add sodium
launcher mod add lithium

# Install all mods
launcher mod install

# Run the game
launcher run
```

## Building from Source

To build MCLauncher from source, you'll need to have Rust's package manager, Cargo, installed.

### Prerequisites

- Rust 1.70 or higher
- Cargo

### Build Instructions

```bash
# Clone the repository
git clone <repository-url>
cd MCLauncher

# Build and install locally
AZURE_CLIENT_ID=<CLIENT_ID> cargo install --path .

# Or build without installing
AZURE_CLIENT_ID=<CLIENT_ID> cargo build --release
```

The resulting binary will be located at `target/release/launcher`

### Development

```bash
# Run tests
AZURE_CLIENT_ID=<CLIENT_ID> cargo test

# Run with debug output
AZURE_CLIENT_ID=<CLIENT_ID> RUST_LOG=debug cargo run -- <command>
```

## Features

- **Authentication**: Support for Microsoft OAuth and offline accounts
- **Version Management**: Create and manage Minecraft directories with ease
- **Configuration**: Set up configurations and update your Minecraft settings
- **Version Exploration**: Explore available versions of Minecraft and download them
- **Fabric Loader**: Install Fabric Loader using the launcher
- **Mod Integration**: Seamless Modrinth integration for mod management
- **Help System**: Access help and assistance commands for more information
- **Cross-platform**: Written in Rust with excellent performance on all platforms
- **Mirrors**: Support for multiple download mirrors for faster downloads

## Architecture

MCLauncher is organized as a Rust workspace with the following components:

- **launcher**: Main CLI application
- **mc-api**: Official Minecraft API bindings
- **mc-oauth**: Microsoft OAuth authentication library
- **modrinth-api**: Modrinth API integration for mod management
- **installer**: Installation and file management utilities

## Limitations

- Microsoft OAuth authentication requires an internet connection
- Some features may not work with all Minecraft versions
- This tool is still in development and may not be suitable for all use cases
- Offline accounts cannot connect to official Minecraft servers

## Contributing

We would love to have your help contributing to MCLauncher! Please report any issues or suggestions on our GitHub repository.

### Development Guidelines

- Follow Rust best practices and coding standards
- Test your changes thoroughly before submitting
- Update documentation as needed
- Submit pull requests with clear descriptions of your changes

## License

This project is released under the [UNLICENSE](UNLICENSE), placing it in the public domain. This means you are free to use, modify, and distribute this software for any purpose, commercial or non-commercial.

## Credits

Built with:

- Rust programming language
- Tokio for async runtime
- Clap for CLI argument parsing
- Reqwest for HTTP requests
- Serde for serialization
- And many other excellent open-source libraries

## Support

For support, feature requests, or bug reports, please visit our [GitHub repository](<repository-url>) and open an issue.
