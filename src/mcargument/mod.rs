use crate::config::{RuntimeConfig, VersionJsonLibraries};
use log::debug;
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};
use uuid::Uuid;

fn replace_arguments(args: Vec<String>, valuemap: HashMap<&str, String>) -> Vec<String> {
    let regex = Regex::new(r"(?<replace>\$\{\S+\})").unwrap();
    args.iter()
        .map(|x| {
            if let Some(c) = regex.captures(x.as_str()) {
                if let Some(content) = valuemap.get(&c["replace"]) {
                    return x.replace(&c["replace"], content);
                }
            }
            x.into()
        })
        .collect()
}

fn replace_arguments_from_jvm(
    args: Vec<String>,
    config: &RuntimeConfig,
) -> anyhow::Result<Vec<String>> {
    let natives_dir = Path::new(&config.game_dir)
        .join("natives")
        .to_string_lossy()
        .into();
    let valuemap = HashMap::from([
        ("${natives_directory}", natives_dir),
        ("${launcher_name}", "my_launcher".into()),
        ("${launcher_version}", "114.514".into()),
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
    let auth_uuid = Uuid::new_v4().into();
    let assets_root: String = Path::new(&config.game_dir)
        .join("assets")
        .to_string_lossy()
        .into();
    let assets_index_name = js["assets"].as_str().unwrap().into();
    let valuemap = HashMap::from([
        ("${auth_player_name}", config.user_name.clone()),
        ("${version_name}", config.game_version.clone()),
        ("${game_directory}", game_directory),
        ("${assets_root}", assets_root),
        ("${assets_index_name}", assets_index_name),
        ("${auth_uuid}", auth_uuid),
        ("${user_type}", config.user_type.clone()),
        ("${version_type}", "release".into()),
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
        args.push(js["mainClass"].as_str().unwrap().into());

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
            .map(|x| x.as_str().unwrap().into())
            .collect();
        args.append(&mut jvm_normal_arg);
        Ok(args)
    }

    fn get_classpaths(&self) -> anyhow::Result<String> {
        let version_json_path = Path::new(&self.game_dir)
            .join("versions")
            .join(&self.game_version)
            .join(self.game_version.clone() + ".json");
        let version_json = fs::read_to_string(version_json_path)?;
        let version_json: serde_json::Value = serde_json::from_str(version_json.as_ref())?;
        let libraries: VersionJsonLibraries =
            serde_json::from_value(version_json["libraries"].clone())?;
        let mut paths: Vec<String> = libraries
            .iter()
            .filter(|obj| {
                let objs = &obj.rules.clone();
                if let Some(_objs) = objs {
                    let flag = _objs
                        .iter()
                        .find(|rules| rules.os.clone().unwrap_or_default()["name"] == "linux");
                    obj.downloads.classifiers == None && flag.clone() != None
                } else {
                    obj.downloads.classifiers == None
                }
            })
            .map(|x| {
                Path::new(&self.game_dir)
                    .join("libraries")
                    .join(&x.downloads.artifact.path)
                    .to_string_lossy()
                    .into()
            })
            .collect();

        let client_path = Path::new(&self.game_dir)
            .join("versions")
            .join(&self.game_version)
            .join(self.game_version.clone() + ".jar")
            .to_string_lossy()
            .into();
        paths.push(client_path);
        let res = paths.join(":");
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
        ("${natives_directory}", "native".into()),
        ("${launcher_name}", "launcher".into()),
    ]);
    let args = Vec::from([
        "start--${natives_directory}--end".into(),
        "${abababa}end".into(),
        "normal".into(),
    ]);

    let answer = Vec::from([
        "start--native--end".to_string(),
        "${abababa}end".to_string(),
        "normal".to_string(),
    ]);

    let res = replace_arguments(args, valuemap);

    assert_eq!(answer, res);
}
