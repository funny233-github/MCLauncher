use clap::{Parser, Subcommand};
use launcher::config::{ConfigHandler, MCLoader, MCMirror, VersionType};
use launcher::install::install_mc;
use launcher::modmanage;
use launcher::runtime::gameruntime;
use log::error;
use mc_api::{fabric::Loader, official::VersionManifest};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// A simple Minecraft launcher
#[derive(Subcommand, Debug)]
enum Command {
    /// Init a new instance
    Init,

    /// List Minecraft or Loader versions
    #[command(subcommand)]
    List(ListSub),

    /// Change username
    Account { name: String },

    /// Install Minecraft
    Install {
        version: Option<String>,

        /// Install fabric loader
        #[arg(long)]
        fabric: Option<String>,
    },

    /// Running game
    Run,

    /// Set Mirror of minecraft api
    #[command(subcommand)]
    Mirror(Mirrors),

    /// Mod Manage
    #[command(subcommand)]
    Mod(ModManage),
}

#[derive(Subcommand, Debug)]
enum ListSub {
    MC {
        #[command(subcommand)]
        mc: VersionType,

        #[arg(long, default_value_t = 60)]
        limit: usize,
    },
    Loader {
        #[command(subcommand)]
        loader: Loaders,

        #[arg(long, default_value_t = 60)]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum Loaders {
    Fabric,
}

#[derive(Subcommand, Debug)]
enum Mirrors {
    Official,
    Bmclapi,
}

#[derive(Subcommand, Debug)]
enum ModManage {
    Add {
        name: String,

        version: Option<String>,

        #[arg(long)]
        local: bool,

        #[arg(long, short = 'c', default_value_t = false)]
        config_only: bool,
    },
    Remove {
        name: String,
    },
    Update {
        #[arg(long, short = 'c', default_value_t = false)]
        config_only: bool,
    },
    Install,
    Sync {
        #[arg(long, short = 'c', default_value_t = false)]
        config_only: bool,
    },
    Search {
        name: String,

        #[arg(long)]
        limit: Option<usize>,
    },
    Clean,
}

fn print_version_list(name: &str, versions: &[String], limit: usize) -> anyhow::Result<()> {
    let count = versions.len();
    let display_count = limit.min(count);
    let mut table = tabled::Table::from_iter(
        versions
            .iter()
            .take(limit)
            .cloned()
            .collect::<Vec<String>>()
            .chunks(6),
    );
    println!(
        "Available {} versions ({}/{}):\n{}",
        name,
        display_count,
        count,
        table.with(tabled::settings::Style::modern())
    );
    Ok(())
}

fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            ConfigHandler::init()?;
            println!("Initialized empty game direction");
        }
        Command::List(sub) => {
            let handle = ConfigHandler::read()?;
            match sub {
                ListSub::MC {
                    mc: version_type,
                    limit: list_limit,
                } => {
                    let list = VersionManifest::fetch(&handle.config().mirror.version_manifest)?
                        .list(version_type.into());
                    print_version_list("Minecraft", &list, list_limit)?;
                }
                ListSub::Loader {
                    loader: _loader,
                    limit: list_limit,
                } => {
                    let l = Loader::fetch(&handle.config().mirror.fabric_meta)?;
                    let list: Vec<String> = l.iter().map(|x| x.version.to_owned()).collect();
                    print_version_list("fabric loader", &list, list_limit)?;
                }
            }
        }
        Command::Account { name: _name } => {
            let mut handle = ConfigHandler::read()?;
            handle.add_offline_account(&_name);
        }
        Command::Install { version, fabric } => {
            let mut handle = ConfigHandler::read()?;
            if version.is_none() && fabric.is_none() {
                install_mc(handle.config())?;
                return Ok(());
            }

            if let Some(_version) = version {
                println!("Set version to {}", &_version);
                handle.config_mut().game_version = _version;
            }
            if let Some(_fabric) = fabric {
                println!("Set loader to {}", &_fabric);
                handle.config_mut().loader = MCLoader::Fabric(_fabric);
            } else {
                handle.config_mut().loader = MCLoader::None;
            }
            drop(handle);
            install_mc(ConfigHandler::read()?.config())?;
        }
        Command::Run => {
            let config = ConfigHandler::read()?;
            gameruntime(config)?;
        }
        Command::Mirror(mirror) => {
            let mut handle = ConfigHandler::read()?;
            match mirror {
                Mirrors::Official => handle.config_mut().mirror = MCMirror::official_mirror(),
                Mirrors::Bmclapi => handle.config_mut().mirror = MCMirror::bmcl_mirror(),
            }
            println!("Set official mirror");
        }
        Command::Mod(option) => match option {
            ModManage::Add {
                name,
                version,
                local,
                config_only,
            } => modmanage::add(&name, version, local, config_only)?,
            ModManage::Remove { name } => modmanage::remove(&name)?,
            ModManage::Update { config_only } => modmanage::update(config_only)?,
            ModManage::Install => modmanage::install()?,
            ModManage::Sync { config_only } => modmanage::sync(config_only)?,
            ModManage::Search { name, limit } => modmanage::search(&name, limit)?,
            ModManage::Clean => modmanage::clean()?,
        },
    }
    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(e) = handle_args() {
        error!("Error occurred: {}\nCaused by: {:?}", e, e.source());
    }
}
