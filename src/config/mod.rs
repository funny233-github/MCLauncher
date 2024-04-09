#[derive(Debug)]
pub struct RuntimeConfig {
    pub max_memory_size: u32,
    pub window_weight: u32,
    pub window_height: u32,
    pub is_full_screen: bool,
    pub user_name: String,
    pub user_type: String,
    pub game_dir: String,
    pub game_version: String,
    pub java_path: String,
}
