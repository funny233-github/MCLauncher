use clap::{Parser, Subcommand};
use launcher::config::{MCMirror, RuntimeConfig, VersionManifestJson, VersionType};
use launcher::install::install_mc;
use launcher::runtime::gameruntime;
use log::error;
use std::fs;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init,

    #[command(subcommand)]
    List(VersionType),

    Account {
        name: String,
    },

    Build {
        version: Option<String>,
    },

    Run,

    #[command(subcommand)]
    SetMirror(Mirrors),
}

#[derive(Subcommand, Debug)]
enum Mirrors {
    Official,
    BMCLAPI,
}

fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            let normal_config = RuntimeConfig {
                max_memory_size: 5000,
                window_weight: 854,
                window_height: 480,
                user_name: "no_name".to_string(),
                user_type: "offline".to_string(),
                game_dir: std::env::current_dir()?.to_str().unwrap().to_string() + "/",
                game_version: "no_game_version".to_string(),
                java_path: "/usr/bin/java".to_string(),
                mirror: MCMirror {
                    version_manifest: "https://launchermeta.mojang.com/".to_string(),
                    assets: "https://resources.download.minecraft.net/".to_string(),
                    client: "https://launcher.mojang.com/".to_string(),
                    libraries: "https://libraries.minecraft.net/".to_string(),
                },
            };
            fs::write("config.json", serde_json::to_string_pretty(&normal_config)?)?;
            println!("Initialized empty game direction");
        }
        Command::List(_type) => {
            let jsfile = fs::read_to_string("config.json")?;
            let js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            let list = VersionManifestJson::new(&js)?.version_list(_type);
            println!("{:?}", list);
        }
        Command::Account { name: _name } => {
            let jsfile = fs::read_to_string("config.json")?;
            let mut js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            js.user_name = _name;
            fs::write("config.json", serde_json::to_string_pretty(&js)?)?;
        }
        Command::Build { version: None } => {
            let jsfile = fs::read_to_string("config.json")?;
            let js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            install_mc(&js)?;
        }
        Command::Build {
            version: Some(_version),
        } => {
            let jsfile = fs::read_to_string("config.json")?;
            let mut js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            js.game_version = _version.clone();
            fs::write("config.json", serde_json::to_string_pretty(&js)?)?;
            println!("Set version to {}", _version);
            install_mc(&js)?;
        }
        Command::Run => {
            let jsfile = fs::read_to_string("config.json")?;
            let js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            gameruntime(js)?;
        }
        Command::SetMirror(Mirrors::Official) => {
            let jsfile = fs::read_to_string("config.json")?;
            let mut js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            js.mirror = MCMirror {
                version_manifest: "https://launchermeta.mojang.com/".to_string(),
                assets: "https://resources.download.minecraft.net/".to_string(),
                client: "https://launcher.mojang.com/".to_string(),
                libraries: "https://libraries.minecraft.net/".to_string(),
            };
            fs::write("config.json", serde_json::to_string_pretty(&js)?)?;
            println!("Set official mirror");
        }

        Command::SetMirror(Mirrors::BMCLAPI) => {
            let jsfile = fs::read_to_string("config.json")?;
            let mut js: RuntimeConfig = serde_json::from_str(&jsfile)?;
            js.mirror = MCMirror {
                version_manifest: "https://bmclapi2.bangbang93.com/".to_string(),
                assets: "https://bmclapi2.bangbang93.com/assets/".to_string(),
                client: "https://bmclapi2.bangbang93.com/".to_string(),
                libraries: "https://bmclapi2.bangbang93.com/maven/".to_string(),
            };
            fs::write("config.json", serde_json::to_string_pretty(&js)?)?;
            println!("Set BMCLAPI mirror");
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
