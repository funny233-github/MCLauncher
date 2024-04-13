use crate::config::RuntimeConfig;
use log::debug;
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};
use uuid::Uuid;
use walkdir::WalkDir;

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
    config: &RuntimeConfig,
) -> anyhow::Result<Vec<String>> {
    let valuemap = HashMap::from([
        (
            "${natives_directory}",
            // config.game_dir.clone() + "versions/" + &config.game_version + "/natives-linux-x86_64",
            config.game_dir.clone() + "natives/",
        ),
        ("${launcher_name}", "my_launcher".to_string()),
        ("${launcher_version}", "114.514".to_string()),
        ("${classpath}", config.get_classpaths()?),
    ]);
    Ok(replace_arguments(args, valuemap))
}

fn replace_arguments_from_game(
    args: Vec<String>,
    config: &RuntimeConfig,
) -> anyhow::Result<Vec<String>> {
    let js = config.version_json_provider()?;
    let game_directory = config.game_dir.clone();
    let auth_uuid = Uuid::new_v4().to_string();
    let assets_root = config.game_dir.clone() + "assets/";
    let assets_index_name = js["assets"].as_str().unwrap().to_string();
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

impl RuntimeConfig {
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
        args.push(js["mainClass"].as_str().unwrap().to_string());

        let game = &mut arguments["game"];
        let game_args = self.get_normal_args_from(game)?;
        let mut game_args = replace_arguments_from_game(game_args, self)?;
        args.append(&mut game_args);

        debug!("{:#?}", args);
        Ok(args)
    }

    fn get_normal_args_from(&self, js: &mut serde_json::Value) -> anyhow::Result<Vec<String>> {
        //TODO parse arg which contain "rules"
        let mut args = Vec::new();
        let mut jvm_normal_arg: Vec<String> = js
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x.is_string())
            .map(|x| x.as_str().unwrap().to_string())
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

    fn version_json_provider(&self) -> anyhow::Result<serde_json::Value> {
        let jsfile_path = Path::new(&self.game_dir)
            .join("versions")
            .join(&self.game_version)
            .join(self.game_version.clone() + ".json");
        let jsfile = fs::read_to_string(jsfile_path)?;
        let js = serde_json::from_str(&jsfile)?;
        Ok(js)
    }
}

#[test]
fn test_replace_arguments() {
    let valuemap = HashMap::from([
        ("${natives_directory}", "native".to_string()),
        ("${launcher_name}", "launcher".to_string()),
    ]);
    let args = Vec::from([
        "start--${natives_directory}--end".to_string(),
        "${abababa}end".to_string(),
        "normal".to_string(),
    ]);

    let answer = Vec::from([
        "start--native--end".to_string(),
        "${abababa}end".to_string(),
        "normal".to_string(),
    ]);

    let res = replace_arguments(args, valuemap);

    assert_eq!(answer, res);
}
