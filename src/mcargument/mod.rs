use crate::config::ConfigHandler;
use mc_api::official::Version;
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};

#[cfg(target_os = "windows")]
const CLASSPATH_SEPARATOR: &str = ";";

#[cfg(target_os = "linux")]
const CLASSPATH_SEPARATOR: &str = ":";

#[cfg(target_os = "macos")]
const CLASSPATH_SEPARATOR: &str = ":";

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
    handle: &ConfigHandler,
    version_api: &Version,
) -> anyhow::Result<Vec<String>> {
    let natives_dir = Path::new(&handle.config().game_dir)
        .join("natives")
        .to_string_lossy()
        .into();
    let valuemap = HashMap::from([
        ("${natives_directory}", natives_dir),
        ("${launcher_name}", "my_launcher".into()),
        ("${launcher_version}", "114.514".into()),
        ("${classpath}", handle.get_classpaths(version_api)?),
    ]);
    Ok(replace_arguments(args, valuemap))
}

fn replace_arguments_from_game(
    args: Vec<String>,
    handle: &ConfigHandler,
) -> anyhow::Result<Vec<String>> {
    let js = handle.version_api()?;
    let assets_root: String = Path::new(&handle.config().game_dir)
        .join("assets")
        .to_string_lossy()
        .into();
    let assets_index_name = js.assets;
    let valuemap = HashMap::from([
        ("${auth_player_name}", handle.user_account().user_name.clone()),
        ("${version_name}", handle.config().game_version.clone()),
        ("${game_directory}", handle.config().game_dir.clone()),
        ("${assets_root}", assets_root),
        ("${assets_index_name}", assets_index_name),
        ("${auth_uuid}", handle.user_account().user_uuid.clone()),
        ("${user_type}", handle.user_account().user_type.clone()),
        ("${version_type}", "release".into()),
    ]);
    //TODO get user info
    // valuemap.insert("${auth_access_token}","");

    Ok(replace_arguments(args, valuemap))
}

impl ConfigHandler {
    pub fn args_provider(&self) -> anyhow::Result<Vec<String>> {
        let mut args = vec![
            format!("-Xmx{}m", self.config().max_memory_size),
            format!("-Xmn256m"),
            format!("-XX:+UseG1GC"),
            format!("-XX:-UseAdaptiveSizePolicy"),
            format!("-XX:-OmitStackTraceInFastThrow"),
            format!("-Dfml.ignoreInvalidMinecraftCertificates=True"),
            format!("-Dfml.ignorePatchDiscrepancies=True"),
            format!("-Dlog4j2.formatMsgNoLookups=true"),
            format!("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"),
        ];

        let js = self.version_api()?;
        let jvm = &mut js.arguments.jvm.clone();

        let jvm_args = self.get_normal_args_from(jvm)?;
        let mut jvm_args = replace_arguments_from_jvm(jvm_args, self, &js)?;
        args.append(&mut jvm_args);
        args.push(js.main_class.as_str().into());

        let game = &mut js.arguments.game.clone();
        let game_args = self.get_normal_args_from(game)?;
        let mut game_args = replace_arguments_from_game(game_args, self)?;
        args.append(&mut game_args);

        Ok(args)
    }

    fn get_normal_args_from(&self, js: &mut [serde_json::Value]) -> anyhow::Result<Vec<String>> {
        Ok(js
            .iter()
            .filter(|x| x.is_string())
            .map(|x| x.as_str().unwrap().into())
            .collect())
    }

    fn get_classpaths(&self, version_api: &Version) -> anyhow::Result<String> {
        let mut paths: Vec<String> = version_api
            .libraries
            .iter()
            .filter(|x| x.is_target_lib())
            .map(|x| {
                Path::new(&self.config().game_dir)
                    .join("libraries")
                    .join(&x.downloads.artifact.path)
                    .to_string_lossy()
                    .into()
            })
            .collect();

        let client_path = Path::new(&self.config().game_dir)
            .join("versions")
            .join(&self.config().game_version)
            .join(self.config().game_version.clone() + ".jar")
            .to_string_lossy()
            .into();
        paths.push(client_path);
        Ok(paths.join(CLASSPATH_SEPARATOR))
    }

    fn version_api(&self) -> anyhow::Result<Version> {
        let jsfile_path = Path::new(&self.config().game_dir)
            .join("versions")
            .join(&self.config().game_version)
            .join(self.config().game_version.to_owned() + ".json");
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
