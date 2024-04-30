use clap::{Parser, Subcommand};
use launcher::api::official::VersionManifest;
use launcher::config::{MCMirror, RuntimeConfig, VersionType, MCLoader};
use launcher::install::install_mc;
use launcher::runtime::gameruntime;
use launcher::api::fabric::Loader;
use log::error;
use std::fs;
use std::path::Path;
use uuid::Uuid;

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

    /// Change user name
    Account {
        name: String,
    },

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
}

#[derive(Subcommand, Debug)]
enum ListSub {
    #[command(subcommand)]
    MC(VersionType),
    Loader{
        #[command(subcommand)]
        loader: Loaders,
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

fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_path = Path::new(".").join("config.toml");
    let normal_config = RuntimeConfig::default();
    match args.command {
        Command::Init => {
            fs::write(config_path, toml::to_string_pretty(&normal_config)?)?;
            println!("Initialized empty game direction");
        }
        Command::List(sub) => {
            let config = fs::read_to_string("config.toml")?;
            let config: RuntimeConfig = toml::from_str(&config)?;
            match sub {
                ListSub::MC(_type) => {
                    let list = VersionManifest::fetch(&config.mirror.version_manifest)?.list(_type);
                    println!("{:?}",list);
                }
                ListSub::Loader{loader:_loader} => {
                    let l = Loader::fetch(&config.mirror.fabric_meta)?;
                    let list:Vec<&str> = l.iter().map(|x|x.version.as_ref()).collect();
                    println!("{:?}",list);
                }
            }
        }
        Command::Account { name: _name } => {
            let config = fs::read_to_string("config.toml")?;
            let mut config: RuntimeConfig = toml::from_str(&config)?;
            config.user_name = _name;
            config.user_uuid = Uuid::new_v4().into();
            config.user_type = "offline".into();
            fs::write(config_path, toml::to_string_pretty(&config)?)?;
        }
        Command::Install { version ,fabric } => {
            let config = fs::read_to_string("config.toml")?;
            let mut config: RuntimeConfig = toml::from_str(&config)?;
            if let Some(_version) = version {
                println!("Set version to {}", &_version);
                config.game_version = _version;
            }
            if let Some(_fabric) = fabric {
                println!("Set loader to {}", &_fabric);
                config.loader = MCLoader::Fabric(_fabric);
            }
            fs::write(config_path, toml::to_string_pretty(&config)?)?;
            install_mc(&config)?;
        }
        Command::Run => {
            let config = fs::read_to_string("config.toml")?;
            let config: RuntimeConfig = toml::from_str(&config)?;
            gameruntime(config)?;
        }
        Command::Mirror(mirror) => {
            let config = fs::read_to_string("config.toml")?;
            let mut config: RuntimeConfig = toml::from_str(&config)?;
            match mirror {
                Mirrors::Official => config.mirror = MCMirror::official_mirror(),
                Mirrors::Bmclapi => config.mirror = MCMirror::bmcl_mirror(),
            }
            fs::write(config_path, toml::to_string_pretty(&config)?)?;
            println!("Set official mirror");
        }
    }
    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(e) = handle_args() {
        error!("{:#?}", e);
    }
}
