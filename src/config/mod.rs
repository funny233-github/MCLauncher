use json::JsonValue;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct LauncherConfig {
    pub max_memory_size: u32,
    pub window_weight: u32,
    pub window_height: u32,
    pub is_full_screen: bool,
    pub user_name: String,
    pub user_type: String,
    pub game_dir: String,
    pub game_version: String,
}

fn replace_arguments(args: Vec<String>, valuemap: HashMap<&str, String>) -> Vec<String> {
    let regex = Regex::new(r"(?<replace>\$\{\S+\})").unwrap();
    args.iter()
        .map(|x| {
            if let Some(c) = regex.captures(x.as_str()) {
                if let Some(content) = valuemap.get(&c["replace"]) {
                    return x.replace(&c["replace"], content);
                }
            }
            x.to_string()
        })
        .collect()
}

fn replace_arguments_from_jvm(
    args: Vec<String>,
    config: &LauncherConfig,
) -> anyhow::Result<Vec<String>> {
    let valuemap = HashMap::from([
        (
            "${natives_directory}",
            config.game_dir.clone() + "versions/" + &config.game_version + "/natives-linux-x86_64",
        ),
        ("${launcher_name}", "my_launcher".to_string()),
        ("${launcher_version}", "114.514".to_string()),
        ("${classpath}", config.get_classpaths()?),
    ]);
    Ok(replace_arguments(args, valuemap))
}

fn replace_arguments_from_game(
    args: Vec<String>,
    config: &LauncherConfig,
) -> anyhow::Result<Vec<String>> {
    let js = config.version_json_provider()?;
    let game_directory = config.game_dir.clone();
    let auth_uuid = Uuid::new_v4().to_string();
    let assets_root = config.game_dir.clone() + "assets/";
    let assets_index_name = js["assets"].to_string();
    let valuemap = HashMap::from([
        ("${auth_player_name}", config.user_name.clone()),
        ("${version_name}", config.game_version.clone()),
        ("${game_directory}", game_directory),
        ("${assets_root}", assets_root),
        ("${assets_index_name}", assets_index_name),
        ("${auth_uuid}", auth_uuid),
        ("${user_type}", config.user_type.clone()),
        ("${version_type}", "release".to_string()),
    ]);
    //TODO get user info
    // valuemap.insert("${auth_access_token}","");

    Ok(replace_arguments(args, valuemap))
}

impl LauncherConfig {
    pub fn args_provider(&self) -> anyhow::Result<Vec<String>> {
        let mut args = Vec::new();
        let mut jvm_basic_arg = vec![
            format!("-Xmx{}m", self.max_memory_size),
            format!("-Xmn256m"),
            format!("-XX:+UseG1GC"),
            format!("-XX:-UseAdaptiveSizePolicy"),
            format!("-XX:-OmitStackTraceInFastThrow"),
            format!("-Dfml.ignoreInvalidMinecraftCertificates=True"),
            format!("-Dfml.ignorePatchDiscrepancies=True"),
            format!("-Dlog4j2.formatMsgNoLookups=true"),
            format!("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"),
        ];
        args.append(&mut jvm_basic_arg);

        let js = self.version_json_provider()?;
        let arguments = &mut js["arguments"].clone();
        let jvm = &mut arguments["jvm"];

        let jvm_args = self.get_normal_args_from(jvm)?;
        let mut jvm_args = replace_arguments_from_jvm(jvm_args, self)?;
        args.append(&mut jvm_args);
        args.push(js["mainClass"].to_string());

        let game = &mut arguments["game"];
        let game_args = self.get_normal_args_from(game)?;
        let mut game_args = replace_arguments_from_game(game_args, self)?;
        args.append(&mut game_args);

        Ok(args)
    }

    fn get_normal_args_from(&self, js: &mut JsonValue) -> anyhow::Result<Vec<String>> {
        //TODO parse arg which contain "rules"
        let mut args = Vec::new();
        let mut jvm_normal_arg: Vec<String> = js
            .members()
            .filter(|x| x.is_string())
            .map(|x| x.to_string())
            .collect();
        args.append(&mut jvm_normal_arg);
        Ok(args)
    }

    fn get_classpaths(&self) -> anyhow::Result<String> {
        let classpath_dir = Path::new(&self.game_dir).join("libraries");
        let mut res = String::new();
        for entry in WalkDir::new(classpath_dir) {
            if let Some(e) = entry?.path().to_str() {
                res += e;
                res += ":";
            }
        }
        res += self.game_dir.as_ref();
        res += "versions/";
        res += self.game_version.as_ref();
        res += "/";
        res += self.game_version.as_ref();
        res += ".jar";
        Ok(res)
    }


    fn version_json_provider(&self) -> anyhow::Result<JsonValue> {
        let jsfile_path = Path::new(&self.game_dir)
            .join("versions")
            .join(&self.game_version)
            .join(self.game_version.clone() + ".json");
        let jsfile = fs::read_to_string(jsfile_path)?;
        let js = json::parse(&jsfile)?;
        Ok(js)
    }
}

impl Into<JsonValue> for LauncherConfig {
    fn into(self) -> JsonValue {
        json::object! {
            max_memory_size: self.max_memory_size,
            window_weight: self.window_weight,
            window_height: self.window_height,
            user_name: self.user_name,
            is_full_screen: self.is_full_screen,
            game_dir: self.game_dir,
            game_version: self.game_version,
        }
    }
}

impl LauncherConfig {
    pub fn from(js: JsonValue) -> Result<LauncherConfig, Error> {
        let max_memory_size = match js["max_memory_size"].as_u32() {
            Some(res) => res,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "read json fail, not found max_memory_size",
                ))
            }
        };

        let window_weight = match js["window_weight"].as_u32() {
            Some(res) => res,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "read json fail, not found window_weight",
                ))
            }
        };

        let window_height = match js["window_height"].as_u32() {
            Some(res) => res,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "read json fail, not found window_height",
                ))
            }
        };

        let is_full_screen = match js["is_full_screen"].as_bool() {
            Some(res) => res,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "read json fail, not found is_full_screen",
                ))
            }
        };

        let game_dir = js["game_dir"].to_string();

        let user_name = js["user_name"].to_string();

        let user_type = js["user_type"].to_string();

        let game_version = js["game_version"].to_string();

        Ok(LauncherConfig {
            max_memory_size,
            window_weight,
            window_height,
            user_name,
            user_type,
            is_full_screen,
            game_dir,
            game_version,
        })
    }
}
