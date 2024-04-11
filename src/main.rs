use clap::{Parser, Subcommand};
use launcher::config::{RuntimeConfig, VersionManifestJson, VersionType};
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

    Install {
        version: Option<String>,
    },

    Run,
}

fn handle_args() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            let manifest_data = VersionManifestJson::new()?;
            let latest = manifest_data.latest.release;
            let normal_config = RuntimeConfig {
                max_memory_size: 5000,
                window_weight: 854,
                window_height: 480,
                user_name: "no_name".to_string(),
                user_type: "offline".to_string(),
                game_dir: std::env::current_dir()?.to_str().unwrap().to_string()+"/",
                game_version: latest,
                java_path: "/usr/bin/java".to_string(),
            };
            fs::create_dir("versions").unwrap_or(());
            fs::create_dir("assets").unwrap_or(());
            fs::create_dir("libraries").unwrap_or(());
            fs::write("config.json", serde_json::to_string_pretty(&normal_config)?)?;
        }
        Command::List(_type) => {
            let list = VersionManifestJson::new()?.version_list(_type);
            println!("{:?}", list);
        }
        Command::Account{name:_name} => {
            let jsfile = fs::read_to_string("config.json")?;
            let mut js:RuntimeConfig = serde_json::from_str(&jsfile)?;
            js.user_name = _name;
            fs::write("config.json", serde_json::to_string_pretty(&js)?)?;
        }
        Command::Install{version:None} => {

        }
        Command::Install{version:Some(_version)} => {

        }
        Command::Run => {
            let jsfile = fs::read_to_string("config.json")?;
            let js:RuntimeConfig = serde_json::from_str(&jsfile)?;
            gameruntime(js)?;
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
