# Gluon

A modern, efficient Minecraft gluon written in Rust. This tool provides a command-line interface to launch and manage different versions of Minecraft with support for mod installation, Microsoft OAuth authentication, and more.

## Version

Current version: **0.8.3**

## Introduction

Welcome to Gluon! This tool is designed to make it easy to launch and manage different versions of Minecraft. With just a few commands, you can create and manage Minecraft directories, set up configurations, download and install different versions of Minecraft, authenticate with Microsoft accounts, and install additional mods from Modrinth.

## Key Features

- **Microsoft OAuth Authentication**: Securely authenticate with your Microsoft account for official Minecraft servers
- **Mod Management**: Seamless integration with Modrinth for installing and managing mods
- **Multiple Minecraft Versions**: Support for various Minecraft versions and loaders
- **Fabric Loader Support**: Easy installation of Fabric mods with different loader versions
- **NeoForge Loader Support**: Easy installation of NeoForge mods with different loader versions
- **Download Mirrors**: Choose from multiple download mirrors (Official / BMCLAPI) for faster downloads
- **Cross-platform**: Written in Rust for excellent performance on all platforms

## Getting Started

To get started with Gluon, follow these steps:

### Installation

**Using install scripts (recommended):**

| Platform | Command |
|----------|---------|
| Linux / macOS | `curl -fsSL https://raw.githubusercontent.com/funny233-github/MCLauncher/main/scripts/install.sh \| bash` |
| Windows (PowerShell) | `iwr -useb https://raw.githubusercontent.com/funny233-github/MCLauncher/main/scripts/install.ps1 \| iex` |

To uninstall, append `--uninstall` (Linux/macOS) or `-Uninstall` (Windows).

**From source:**

```bash
git clone <repository-url>
cd Gluon
cargo install --path .
```

**Using cargo:**

```bash
cargo install gluon
```

### Basic Setup

1. **Create a New Directory**: Initialize a new directory to store Minecraft files using the `gluon init` command.
2. **Set Up Configurations**: Run `gluon config` within the directory to set up configurations.
3. **Authenticate with Your Account**:
   - For Microsoft accounts: `gluon account microsoft`
4. **Explore Available Versions**: Explore available versions using `gluon list <version_type>`.
5. **Select a Mirror**: Specify a download mirror via `gluon mirror <mirror>`.
6. **Install Minecraft**: Install a specific version of Minecraft using the `gluon install <version>` command.
7. **Install with Fabric Loader**: Install Minecraft along with the Fabric Loader using the `gluon install <version> --fabric <fabric_loader_version>` command.
8. **Install with NeoForge Loader**: Install Minecraft along with the NeoForge Loader using the `gluon install <version> --neoforge <neoforge_version>` command.
9. **Run Minecraft**: Launch the game with `gluon run`
10. **Access Help**: For more commands and details, type `gluon help`.

### Microsoft OAuth Authentication

Gluon now supports full Microsoft OAuth authentication for playing on official Minecraft servers:

```bash
gluon account microsoft
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

Gluon provides comprehensive mod management capabilities through Modrinth integration:

### Prerequisites

Ensure you have installed Fabric via:

```bash
gluon install <minecraft_version> --fabric <fabric_loader_version>
```

### Mod Management Commands

1. **Find a Mod**: Search for mods on Modrinth, the Mod name is always at the end of the URL (e.g., `https://modrinth.com/mod/fabric-api` → `fabric-api`)
2. **Add a Mod**: `gluon mod add <mod_name>`
3. **Remove a Mod**: `gluon mod remove <mod_name>`
4. **Update All Mods**: Update all mods listed in `config.toml` with `gluon mod update`
5. **Sync All Mods**: Sync all mods from `config.toml` with `gluon mod sync`
6. **Install All Mods**: Install all mods from `config.lock` with `gluon mod install`
7. **Clean Unused Mods**: Remove all mods not in `config.toml` with `gluon mod clean`
8. **Search for Mods**: Search for related mods with `gluon mod search <name>`

### Example Workflow

```bash
# Install Minecraft with Fabric
gluon install 1.20.4 --fabric 0.15.11

# Search for a mod
gluon mod search sodium

# Add mods
gluon mod add fabric-api
gluon mod add sodium
gluon mod add lithium

# Install all mods
gluon mod install

# Run the game
gluon run
```

## Building from Source

To build Gluon from source, you'll need to have Rust's package manager, Cargo, installed.

### Azure Client ID

The `AZURE_CLIENT_ID` environment variable is required for Microsoft OAuth authentication.

**For production**: Register an application in [Azure Portal](https://portal.azure.com/) and set the `AZURE_CLIENT_ID` environment variable during compilation:

```bash
export AZURE_CLIENT_ID="your_client_id"
cargo build --release
```

### Prerequisites

- Rust 1.70 or higher
- Cargo

### Build Instructions

```bash
# Clone the repository
git clone <repository-url>
cd Gluon

# Build and install locally
AZURE_CLIENT_ID=<CLIENT_ID> cargo install --path .

# Or build without installing
AZURE_CLIENT_ID=<CLIENT_ID> cargo build --release
```

The resulting binary will be located at `target/release/gluon`

### Development

```bash
# Run tests
AZURE_CLIENT_ID=<CLIENT_ID> cargo test

# Run with debug output
AZURE_CLIENT_ID=<CLIENT_ID> RUST_LOG=debug cargo run -- <command>
```

## Features

- **Authentication**: Support for Microsoft OAuth accounts
- **Version Management**: Create and manage Minecraft directories with ease
- **Configuration**: Set up configurations and update your Minecraft settings
- **Version Exploration**: Explore available versions of Minecraft and download them
- **Fabric Loader**: Install Fabric Loader using the gluon
- **NeoForge Loader**: Install NeoForge Loader using the gluon
- **Mod Integration**: Seamless Modrinth integration for mod management
- **Help System**: Access help and assistance commands for more information
- **Cross-platform**: Written in Rust with excellent performance on all platforms
- **Mirrors**: Support for multiple download mirrors (Official / BMCLAPI) for faster downloads

## Architecture

Gluon is organized as a Rust workspace with the following components:

- **gluon**: Main CLI application
- **mc-api**: Official Minecraft API bindings
- **mc-oauth**: Microsoft OAuth authentication library
- **modrinth-api**: Modrinth API integration for mod management
- **installer**: Installation and file management utilities

## Limitations

- Microsoft OAuth authentication requires an internet connection
- Some features may not work with all Minecraft versions
- This tool is still in development and may not be suitable for all use cases

## Contributing

We would love to have your help contributing to Gluon! Please report any issues or suggestions on our GitHub repository.

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
