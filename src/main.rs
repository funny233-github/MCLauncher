use clap::{Parser, Subcommand};
use clap_cargo::style;
use gluon::config::{ConfigHandler, MCLoader, MCMirror, VersionType};
use gluon::install::install_mc;
use gluon::modmanage;
use gluon::runtime::gameruntime;
use mc_api::{fabric, neoforge, official::VersionManifest};
use version_compare::Version;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, styles = style::CLAP_STYLING)]
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
    #[command(subcommand)]
    Account(Account),

    /// Install Minecraft
    Install {
        version: Option<String>,

        /// Install fabric loader
        #[arg(long)]
        fabric: Option<String>,
    },

    /// Run the game
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
enum Account {
    Offline { name: String },
    Microsoft,
}

#[derive(Subcommand, Debug)]
enum Loaders {
    Fabric,
    Neoforge,
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

fn print_version_list(name: &str, versions: &[String], limit: usize) {
    let count = versions.len();
    let display_count = limit.min(count);
    let mut table: tabled::Table = versions
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<String>>()
        .chunks(6)
        .collect();
    println!(
        "Available {} versions ({}/{}):\n{}",
        name,
        display_count,
        count,
        table.with(tabled::settings::Style::modern())
    );
}

fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            ConfigHandler::init()?;
            println!("Initialized empty game directory");
        }
        Command::List(sub) => {
            let handle = ConfigHandler::read()?;
            match sub {
                ListSub::MC { mc, limit } => {
                    let list = VersionManifest::fetch(&handle.config().mirror.version_manifest)?
                        .list(&mc.into());
                    print_version_list("Minecraft", &list, limit);
                }
                ListSub::Loader { loader, limit } => match loader {
                    Loaders::Fabric => {
                        let l = fabric::Loader::fetch(&handle.config().mirror.fabric_meta)?;
                        let list: Vec<String> = l.iter().map(|x| x.version.clone()).collect();
                        print_version_list("fabric loader", &list, limit);
                    }
                    Loaders::Neoforge => {
                        let l = neoforge::Loader::fetch(&handle.config().mirror.neoforge_neoforge)?;

                        let mc_version = Version::from(&handle.config().game_version).unwrap();

                        let list: Vec<String> = l
                            .versioning
                            .versions
                            .version
                            .into_iter()
                            .filter(|x| {
                                let neoforge_version = Version::from(x).unwrap();
                                mc_version.part(1) == neoforge_version.part(0)
                                    && mc_version.part(2) == neoforge_version.part(1)
                            })
                            .collect();
                        print_version_list("neoforge loader", &list, limit);
                    }
                },
            }
        }
        Command::Account(account_type) => {
            let mut handle = ConfigHandler::read()?;
            match account_type {
                Account::Offline { name } => {
                    handle.add_offline_account(&name);
                }
                Account::Microsoft => handle.add_microsoft_account()?,
            }
        }
        Command::Install { version, fabric } => {
            let mut handle = ConfigHandler::read()?;
            if version.is_none() && fabric.is_none() {
                install_mc(handle.config())?;
                return Ok(());
            }

            if let Some(version) = version {
                println!("Set version to {}", &version);
                handle.config_mut().game_version = version;
            }
            if let Some(fabric) = fabric {
                println!("Set loader to {}", &fabric);
                handle.config_mut().loader = MCLoader::Fabric(fabric);
            } else {
                handle.config_mut().loader = MCLoader::None;
            }
            drop(handle);
            install_mc(ConfigHandler::read()?.config())?;
        }
        Command::Run => {
            let config = ConfigHandler::read()?;
            gameruntime(&config)?;
        }
        Command::Mirror(mirror) => {
            let mut handle = ConfigHandler::read()?;
            match mirror {
                Mirrors::Official => {
                    handle.config_mut().mirror = MCMirror::official_mirror();
                    println!("Set official mirror");
                }
                Mirrors::Bmclapi => {
                    handle.config_mut().mirror = MCMirror::bmcl_mirror();
                    println!("Set BMCLAPI mirror");
                }
            }
        }
        Command::Mod(option) => match option {
            ModManage::Add {
                name,
                version,
                local,
                config_only,
            } => modmanage::add(&name, version.as_ref(), local, config_only)?,
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

fn main() -> anyhow::Result<()> {
    env_logger::init();
    handle_args()?;
    Ok(())
}
