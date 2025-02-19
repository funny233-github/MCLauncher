use crate::config::ConfigHandler;
use anyhow::Result;
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
        .map(|arg| {
            regex
                .captures(arg.as_str())
                .and_then(|captures| valuemap.get_key_value(&captures["replace"]))
                .map_or(arg.to_string(), |(capture, content)| {
                    arg.replace(capture, content)
                })
        })
        .collect()
}

fn replace_arguments_from_jvm(
    args: Vec<String>,
    handle: &ConfigHandler,
    version_api: &Version,
) -> anyhow::Result<Vec<String>> {
    let natives_dir: String = Path::new(&handle.config().game_dir)
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
        (
            "${auth_player_name}",
            handle.user_account().user_name.clone(),
        ),
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
            .filter_map(|lib| {
                let lib_name = lib.name.as_str().to_string();
                let lib_name = lib_name.split(":").collect::<Vec<_>>();
                let has_greater_version: anyhow::Result<bool> =
                    version_api.libraries.iter().try_fold(false, |res, lib_y| {
                        let lib_y_name = lib_y.name.as_str().to_string();
                        let lib_y_name: Vec<_> = lib_y_name.split(":").collect();

                        let is_same_lib =
                            lib_name[0] == lib_y_name[0] && lib_name[1] == lib_y_name[1];

                        let lib_version =
                            version_compare::Version::from(lib_name[2]).ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Invalid version format for library '{}': {}",
                                    lib_name[1],
                                    lib_name[2]
                                )
                            })?;
                        let lib_y_version = version_compare::Version::from(lib_y_name[2])
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Invalid version format for library '{}': {}",
                                    lib_y_name[1],
                                    lib_y_name[2]
                                )
                            })?;

                        Ok(res || (is_same_lib && lib_version < lib_y_version))
                    });

                match has_greater_version {
                    Ok(has_greater_version) => {
                        if lib.is_target_lib() && !has_greater_version {
                            let path = Path::new(&self.config().game_dir)
                                .join("libraries")
                                .join(&lib.downloads.artifact.path)
                                .to_string_lossy()
                                .to_string();

                            Some(Ok(path))
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            })
            .collect::<Result<_>>()?;

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
