pub mod config;
use config::LauncherConfig;
use log::{debug, info};
use std::process::Command;

const GAME_PATH: &str = "/home/funny/Minecraft/HMCL/.minecraft/";

fn get_java_path() -> String {
    //TODO: implement find java path then return
    "/usr/lib/jvm/java-22-openjdk/bin/java".to_string()
}

fn main() {
    env_logger::init();
    let config = LauncherConfig {
        max_memory_size: 5000,
        window_weight: 400,
        window_height: 400,
        is_full_screen: false,
        user_name: "test".to_string(),
        user_type: "offline".to_string(),
        game_dir: GAME_PATH.to_string(),
        game_version: "1.20.4".to_string(),
    };
    debug!("{:#?}", config.args_provider());
    if let Ok(args) = config.args_provider() {
        let path = get_java_path();
        let output = Command::new(path).args(args).output().unwrap();
    }
}
