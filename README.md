# MCLauncher
## Introduction
MCLauncher is a lightweight command-line tool designed for effortless Minecraft launching.

## Quick Start
1. **Create Minecraft Directory**: Initialize a new directory to store Minecraft files.
2. **Initialize Configuration**: Run `Launcher init` within the directory to set up configurations.
3. **Set Username**: Update your Minecraft username with `Launcher account <username>`.
4. **List Versions**: Explore available versions using `Launcher list <version_type>`.
5. **Select Mirror**: Specify a download mirror via `Launcher set-mirror <mirror>`.
6. **Install Minecraft**: Install a specific version with `Launcher install <version>`.
7. **Install with Fabric Loader**: Install Minecraft along with the Fabric Loader: `Launcher install <version> --fabric <fabric_loader_version>`.
8. **Help and Assistance**: For more commands and details, type `Launcher help`.

## Building from Source
Assuming you have Rust's package manager, Cargo, installed, execute `cargo install --path .` to build the launcher.
