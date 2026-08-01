use clap::{Parser, Subcommand};
use clap_cargo::style;
use gluon::config::{ConfigHandler, MCLoader, MCMirror, VersionType};
use gluon::install::install_mc;
use gluon::modmanage;
use gluon::runtime::gameruntime;
use mc_api::{fabric, neoforge, official::VersionManifest};
use tabled::{settings::Style, Table};
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
        #[arg(long, conflicts_with = "neoforge")]
        fabric: Option<String>,

        /// Install neoforge loader
        #[arg(long)]
        neoforge: Option<String>,
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

        /// Output as JSON
        #[arg(long, global = true)]
        json: bool,
    },
    Loader {
        #[command(subcommand)]
        loader: Loaders,

        #[arg(long, default_value_t = 60)]
        limit: usize,

        /// Output as JSON
        #[arg(long, global = true)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum Account {
    Offline { name: String },
    Microsoft,
    /// Refresh the stored Microsoft account tokens
    Refresh,
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

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

fn print_neoforge_table(neoforge_versions: &[String], mc_releases: &[String]) {
    let neoforge_groups = neoforge::group_by_mc_version(neoforge_versions, mc_releases);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(vec![
        "MC Version".to_string(),
        "Latest NeoForge".to_string(),
        "Total".to_string(),
    ]);

    for mc_ver in mc_releases {
        if let Some(neoforge_list) = neoforge_groups.get(mc_ver) {
            let latest = neoforge_list.first().cloned().unwrap_or_default();
            rows.push(vec![
                mc_ver.clone(),
                latest,
                neoforge_list.len().to_string(),
            ]);
        }
    }

    let mut table: Table = rows.into_iter().collect();
    println!(
        "Available NeoForge versions by MC version:\n{}",
        table.with(Style::modern())
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "Current implementation is easy to edit, need too many lines"
)]
fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            if ConfigHandler::init()? {
                println!("Initialized empty game directory");
            } else {
                println!("config.toml already exists, skip init");
            }
        }
        Command::List(sub) => {
            let handle = ConfigHandler::read()?;
            match sub {
                ListSub::MC { mc, limit, json } => {
                    let list = VersionManifest::fetch(&handle.config().mirror.version_manifest)?
                        .list(&mc.into());
                    if json {
                        let json = serde_json::json!({
                            "name": "Minecraft",
                            "count": list.len(),
                            "display_count": limit.min(list.len()),
                            "versions": list.iter().take(limit).collect::<Vec<_>>(),
                        });
                        println!("{json}");
                    } else {
                        print_version_list("Minecraft", &list, limit);
                    }
                }
                ListSub::Loader { loader, limit, json } => match loader {
                    Loaders::Fabric => {
                        let l = fabric::Loader::fetch(&handle.config().mirror.fabric_meta)?;
                        let list: Vec<String> = l.iter().map(|x| x.version.clone()).collect();
                        if json {
                            let json = serde_json::json!({
                                "name": "fabric loader",
                                "count": list.len(),
                                "display_count": limit.min(list.len()),
                                "versions": list.iter().take(limit).collect::<Vec<_>>(),
                            });
                            println!("{json}");
                        } else {
                            print_version_list("fabric loader", &list, limit);
                        }
                    }
                    Loaders::Neoforge => {
                        let l = neoforge::Loader::fetch(&handle.config().mirror.neoforge_neoforge)?;
                        let neoforge_versions = l.versioning.versions.version;

                        // Check if game_version is set and valid
                        if let Some(mc_version) = Version::from(&handle.config().game_version) {
                            // Filter by MC version
                            let list: Vec<String> = neoforge_versions
                                .into_iter()
                                .filter(|x| {
                                    if let Some(neoforge_version) = Version::from(x) {
                                        mc_version.part(1) == neoforge_version.part(0)
                                            && mc_version.part(2) == neoforge_version.part(1)
                                    } else {
                                        false
                                    }
                                })
                                .collect();
                            if json {
                                let json = serde_json::json!({
                                    "name": "neoforge loader",
                                    "count": list.len(),
                                    "display_count": limit.min(list.len()),
                                    "versions": list.iter().take(limit).collect::<Vec<_>>(),
                                });
                                println!("{json}");
                            } else {
                                print_version_list("neoforge loader", &list, limit);
                            }
                        } else {
                            // No MC version set, show table grouped by MC version
                            let manifest =
                                VersionManifest::fetch(&handle.config().mirror.version_manifest)?;
                            let mc_releases =
                                manifest.list(&mc_api::official::VersionType::Release);
                            if json {
                                let neoforge_groups =
                                    neoforge::group_by_mc_version(&neoforge_versions, &mc_releases);
                                let json = serde_json::json!({
                                    "mc_versions": mc_releases.iter().filter_map(|mc_ver| {
                                        neoforge_groups.get(mc_ver).map(|neoforge_list| {
                                            serde_json::json!({
                                                "mc_version": mc_ver,
                                                "latest": neoforge_list.first(),
                                                "total": neoforge_list.len(),
                                            })
                                        })
                                    }).collect::<Vec<_>>()
                                });
                                println!("{json}");
                            } else {
                                print_neoforge_table(&neoforge_versions, &mc_releases);
                            }
                        }
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
                Account::Refresh => handle.refresh_account()?,
            }
        }
        Command::Install {
            version,
            fabric,
            neoforge,
        } => {
            let mut handle = ConfigHandler::read()?;
            // Loader options must be given together with a version argument;
            // installing a loader without a version is undefined behavior
            // (the game_version suffix would be stacked on re-install).
            if version.is_none() && (fabric.is_some() || neoforge.is_some()) {
                return Err(anyhow::anyhow!(
                    "loader options (--fabric/--neoforge) require a version argument, \
                     e.g. 'gluon install 1.21.1 --fabric 0.16.1'"
                ));
            }
            if version.is_none() && fabric.is_none() && neoforge.is_none() {
                install_mc(&handle)?;
                return Ok(());
            }

            if let Some(version) = version {
                println!("Set version to {version}");
                version.clone_into(&mut handle.config_mut().vanilla);
                handle.config_mut().game_version = version;
            }
            let game_version = handle.config().game_version.clone();
            if let Some(fabric) = fabric {
                println!("Set loader to {fabric}");
                handle.config_mut().loader = MCLoader::Fabric(fabric.clone());

                handle.config_mut().game_version = format!("{game_version}-fabric-{fabric}");
            } else if let Some(neoforge) = neoforge {
                handle.config_mut().loader = MCLoader::Neoforge(neoforge.clone());
                handle.config_mut().game_version = format!("{game_version}-neoforge-{neoforge}");
            } else {
                handle.config_mut().loader = MCLoader::None;
            }
            drop(handle);
            install_mc(&ConfigHandler::read()?)?;
        }
        Command::Run => {
            let mut config = ConfigHandler::read()?;
            config.ensure_valid_token()?;
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
            ModManage::Search { name, limit, json } => modmanage::search(&name, limit, json)?,
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
