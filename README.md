Introduction
============

Welcome to your new Minecraft launcher! This tool is designed to make it easy to launch and manage different versions of Minecraft. With just a few commands, you can create and manage Minecraft directories, set up configurations, and download and install different versions of Minecraft. You can even specify a download mirror and install additional mods.

Getting Started
===============

To get started with your new Minecraft launcher, follow these steps:

1. **Create a New Directory**: Initialize a new directory to store Minecraft files using the `launcher init` command.
2. **Set Up Configurations**: Run `launcher config` within the directory to set up configurations.
3. **Update Your Username**: Update your Minecraft username with `launcher account <username>`.
4. **Explore Available Versions**: Explore available versions using `launcher list <version_type>`.
5. **Select a Mirror**: Specify a download mirror via `launcher mirror <mirror>`.
6. **Install Minecraft**: Install a specific version of Minecraft using the `launcher install <version>` command.
7. **Install with Fabric Loader**: Install Minecraft along with the Fabric Loader using the `launcher install <version> --fabric <fabric_loader_version>` command.
8. **Install with Config**: Install Minecraft with already Config using the `launcher install` command.
9. **Access Help and Assistance**: For more commands and details, type `launcher help`.

ModManage
=========

To enhance manage with launcher, follow these steps:

1. Ensure you have installed Fabric via:  
   `launcher install <minecraft_version> --fabric <fabric_loader_version>`
2. Find Mod name which you want from Modrinth, the Mod name always in the end with URL such as `https://modrinth.com/mod/fabric-api`, the `fabric-api` is Mod name.
3. Add a Mod: Run `launcher mod add <mod_name>`.
4. Remove a Mod: Run `launcher mod remove <mod_name>`
5. Update All Mod which in `config.toml`: Run `launcher mod update`
6. Sync All Mod which in `config.toml`: Run `launcher mod sync`
7. Install All Mod which in `config.lock`: Run `launcher mod install`
8. Clean All Mod which not in `config.toml`: Run `launcher mod clean`
9. Search related Mod: Run `launcher mod search <name>`

Building from Source
======================

To build MCLauncher from source, you'll need to have Rust's package manager, Cargo, installed. Once you have Cargo installed, execute `cargo install --path .` within the project directory to build the launcher.

Features
========

* Create and manage Minecraft directories with ease
* Set up configurations and update your Minecraft username
* Explore available versions of Minecraft and download them
* Install Fabric Loader using the launcher
* Access help and assistance commands for more information on using the launcher
* Build MCLauncher from source with Rust's package manager, Cargo

Limitations
===========

There are no known limitations or issues with this tool at this time. However, please note that this tool is still in beta and may not be suitable for all users.

Contributing
============

We would love to have your help contributing to MCLauncher! Please report any issues or suggestions you have on our GitHub repository.
