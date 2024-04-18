use crate::config::{RuntimeConfig, VersionJsonLibraries};
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const CLASSPATH_SEPARATOR: &str = ";";
#[cfg(target_os = "windows")]
const TARGET_OS: &str = "windows";

#[cfg(target_os = "linux")]
const CLASSPATH_SEPARATOR: &str = ":";
#[cfg(target_os = "linux")]
const TARGET_OS: &str = "linux";

#[cfg(target_os = "macos")]
const CLASSPATH_SEPARATOR: &str = ":";
#[cfg(target_os = "macos")]
const TARGET_OS: &str = "osx";

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
    let assets_root: String = Path::new(&config.game_dir)
        .join("assets")
        .to_string_lossy()
        .into();
    let assets_index_name = js["assets"].as_str().unwrap().into();
    let valuemap = HashMap::from([
        ("${auth_player_name}", config.user_name.clone()),
        ("${version_name}", config.game_version.clone()),
        ("${game_directory}", config.game_dir.clone()),
        ("${assets_root}", assets_root),
        ("${assets_index_name}", assets_index_name),
        ("${auth_uuid}", config.user_uuid.clone()),
        ("${user_type}", config.user_type.clone()),
        ("${version_type}", "release".into()),
    ]);
    //TODO get user info
    // valuemap.insert("${auth_access_token}","");

    Ok(replace_arguments(args, valuemap))
}

impl RuntimeConfig {
    pub fn args_provider(&self) -> anyhow::Result<Vec<String>> {
        let mut args = vec![
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

        Ok(args)
    }

    fn get_normal_args_from(&self, js: &mut serde_json::Value) -> anyhow::Result<Vec<String>> {
        //TODO parse arg which contain "rules"
        Ok(js
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x.is_string())
            .map(|x| x.as_str().unwrap().into())
            .collect())
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
                        .find(|rules| rules.os.clone().unwrap_or_default()["name"] == TARGET_OS);
                    obj.downloads.classifiers.is_none() && flag.is_some()
                } else {
                    obj.downloads.classifiers.is_none()
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
        Ok(paths.join(CLASSPATH_SEPARATOR))
    }

    fn version_json_provider(&self) -> anyhow::Result<serde_json::Value> {
        let jsfile_path = Path::new(&self.game_dir)
            .join("versions")
            .join(&self.game_version)
            .join(self.game_version.clone() + ".json");
        let jsfile = fs::read_to_string(jsfile_path)?;
        Ok(serde_json::from_str(&jsfile)?)
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
