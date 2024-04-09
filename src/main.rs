pub mod config;
pub mod mcargument;
pub mod runtime;
use config::RuntimeConfig;
use runtime::gameruntime;
use log::error;

const GAME_PATH: &str = "/home/funny/Minecraft/HMCL/.minecraft/";

fn main() {
    env_logger::init();
    let config = RuntimeConfig {
        max_memory_size: 5000,
        window_weight: 400,
        window_height: 400,
        is_full_screen: false,
        user_name: "test".to_string(),
        user_type: "offline".to_string(),
        game_dir: GAME_PATH.to_string(),
        game_version: "1.20.4".to_string(),
        java_path: "/usr/lib/jvm/java-22-openjdk/bin/java".to_string(),
    };

    if let Err(e) = gameruntime(config) {
        error!("{}",e);
    }
}
