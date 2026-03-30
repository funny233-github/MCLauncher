//! Minecraft launch arguments generation module.
//!
//! Provides functionality to generate complete command line arguments for launching Minecraft,
//! including JVM arguments, game arguments, and classpath. Supports variable substitution for
//! paths, user authentication, and game configuration.

use crate::config::ConfigHandler;
use anyhow::Result;
use mc_api::official::Version;
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};

/// Classpath separator for Windows.
#[cfg(target_os = "windows")]
const CLASSPATH_SEPARATOR: &str = ";";

/// Classpath separator for Linux.
#[cfg(target_os = "linux")]
const CLASSPATH_SEPARATOR: &str = ":";

/// Classpath separator for macOS.
#[cfg(target_os = "macos")]
const CLASSPATH_SEPARATOR: &str = ":";

/// Replaces variable placeholders in arguments with actual values.
///
/// Scans through argument strings and replaces variables in the format `${variable_name}`
/// with their corresponding values from the provided value map. Variables not found
/// in the map are left unchanged. Returns a new vector of strings with all recognized
/// variables replaced.
fn replace_arguments(args: &[String], valuemap: &HashMap<&str, String>) -> Vec<String> {
    let regex = Regex::new(r"(?<replace>\$\{\S+\})").unwrap();
    args.iter()
        .map(|arg| {
            regex
                .captures(arg.as_str())
                .and_then(|captures| valuemap.get_key_value(&captures["replace"]))
                .map_or(arg.clone(), |(capture, content)| {
                    arg.replace(capture, content)
                })
        })
        .collect()
}

/// Replaces JVM-specific variable placeholders in arguments.
///
/// Prepares a value map containing JVM-specific variables and applies variable
/// substitution to JVM arguments from the version manifest. Supports ${`natives_directory`},
/// ${`launcher_name`}, ${`launcher_version`}, and ${classpath} variables. Returns a new
/// vector of strings with JVM variables replaced.
///
/// # Errors
/// - `anyhow::Error` if the classpath cannot be generated from the version metadata.
fn replace_arguments_from_jvm(
    args: &[String],
    handle: &ConfigHandler,
    version_api: &Version,
) -> anyhow::Result<Vec<String>> {
    let natives_dir: String = Path::new(&handle.config().game_dir)
        .join("natives")
        .to_string_lossy()
        .into();
    let library_dir: String = Path::new(&handle.config().game_dir)
        .join("libraries")
        .to_str()
        .unwrap()
        .to_string();
    let valuemap = HashMap::from([
        ("${natives_directory}", natives_dir),
        ("${launcher_name}", "my_launcher".into()),
        ("${launcher_version}", "114.514".into()),
        ("${classpath}", handle.get_classpaths(version_api)?),
        ("${library_directory}", library_dir),
    ]);
    Ok(replace_arguments(args, &valuemap))
}

/// Replaces game-specific variable placeholders in arguments.
///
/// Prepares a value map containing game-specific variables including user authentication data,
/// game paths, and version information, then applies variable substitution to game arguments from
/// the version manifest. Supports ${`auth_player_name`}, ${`version_name`}, ${`game_directory`},
/// ${`assets_root`}, ${`assets_index_name`}, ${`auth_uuid`}, ${`user_type`}, ${`version_type`}, and
/// ${`auth_access_token`} variables. Returns a new vector of strings with game variables replaced.
///
/// # Errors
/// - `anyhow::Error` if the version API cannot be read.
fn replace_arguments_from_game(
    args: &[String],
    handle: &ConfigHandler,
) -> anyhow::Result<Vec<String>> {
    let js = handle.version_api()?;
    let assets_root: String = Path::new(&handle.config().game_dir)
        .join("assets")
        .to_string_lossy()
        .into();
    let assets_index_name = js.assets;
    let mut valuemap = HashMap::from([
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

    if let Some(access_token) = &handle.user_account().access_token {
        valuemap.insert("${auth_access_token}", access_token.to_owned());
    }

    Ok(replace_arguments(args, &valuemap))
}

impl ConfigHandler {
    /// Generates the complete launch arguments for Minecraft.
    ///
    /// Combines base JVM settings (memory settings, GC, security flags), version-specific JVM
    /// arguments from manifest, the main class specification, and version-specific game arguments
    /// with authentication data. Base JVM arguments include -Xmx{`max_memory_size`}m for maximum
    /// heap, -Xmn256m for minimum heap, -XX:+UseG1GC for G1 garbage collector, and several
    /// compatibility flags for Forge and Log4j security. Returns a vector of strings representing
    /// the complete command line for launching Minecraft.
    ///
    /// # Example
    /// ```no_run
    /// use gluon::config::ConfigHandler;
    /// let handler = ConfigHandler::read().expect("Failed to load config");
    /// let args = handler.args_provider().expect("Failed to generate arguments");
    /// let mut cmd = std::process::Command::new("java");
    /// cmd.args(&args);
    /// ```
    ///
    /// # Errors
    /// - `anyhow::Error` if version API JSON cannot be read
    /// - `anyhow::Error` if classpath cannot be generated
    /// - `anyhow::Error` if game directory or version files cannot be accessed
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

        let jvm_args = Self::get_normal_args_from(jvm);
        let mut jvm_args = replace_arguments_from_jvm(&jvm_args, self, &js)?;
        args.append(&mut jvm_args);
        args.push(js.main_class.as_str().into());

        let game = &mut js.arguments.game.clone();
        let game_args = Self::get_normal_args_from(game);
        let mut game_args = replace_arguments_from_game(&game_args, self)?;
        args.append(&mut game_args);

        Ok(args)
    }

    /// Extracts string arguments from a mixed JSON value array.
    ///
    /// Filters out only the string arguments from version manifest arrays that can contain
    /// either strings or complex objects with conditions. Complex argument objects with
    /// conditions are ignored. Returns a vector containing only the string arguments.
    fn get_normal_args_from(js: &mut [serde_json::Value]) -> Vec<String> {
        js.iter()
            .filter(|x| x.is_string())
            .map(|x| x.as_str().unwrap().into())
            .collect()
    }

    /// Generates the complete Java classpath for the game.
    ///
    /// Includes all required libraries and the client JAR file, handling library version
    /// resolution to ensure that only the latest version of each library is included.
    /// Filters libraries for the current platform and excludes older versions when a newer
    /// version of the same library exists. Returns a platform-specific classpath string
    /// with entries separated by `:` on Unix-like systems or `;` on Windows.
    ///
    /// # Example
    /// On Linux, the output might look like:
    /// ```text
    /// /game/lib/com/mojang/authlib:2.1.28/authlib-2.1.28.jar:/game/lib/org/lwjgl/lwjgl:3.2.2/lwjgl-3.2.2.jar:/game/versions/1.16.5/1.16.5.jar
    /// ```
    ///
    /// # Errors
    /// - `anyhow::Error` if a library version string cannot be parsed
    /// - `anyhow::Error` if a library path cannot be constructed
    /// - `anyhow::Error` if version comparison fails
    fn get_classpaths(&self, version_api: &Version) -> anyhow::Result<String> {
        let mut paths: Vec<String> = version_api
            .libraries
            .iter()
            .filter_map(|lib| {
                let lib_name = lib.name.as_str().to_string();
                let lib_name = lib_name.split(':').collect::<Vec<_>>();
                let has_greater_version: anyhow::Result<bool> =
                    version_api.libraries.iter().try_fold(false, |res, lib_y| {
                        let lib_y_name = lib_y.name.as_str().to_string();
                        let lib_y_name: Vec<_> = lib_y_name.split(':').collect();

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

    /// Reads and parses the version manifest JSON file.
    ///
    /// Loads the version JSON file that was downloaded during the installation process
    /// and parses it into a structured Version object. Returns a Version object containing
    /// parsed version metadata including main class name, JVM arguments, game arguments,
    /// library dependencies, and asset index information. The version JSON is located at
    /// `{game_dir}/versions/{game_version}/{game_version}.json`.
    ///
    /// # Errors
    /// - `anyhow::Error` if the version JSON file cannot be read
    /// - `anyhow::Error` if the JSON cannot be parsed
    /// - `anyhow::Error` if the file is missing or inaccessible
    fn version_api(&self) -> anyhow::Result<Version> {
        let jsfile_path = Path::new(&self.config().game_dir)
            .join("versions")
            .join(&self.config().game_version)
            .join(self.config().game_version.clone() + ".json");
        let jsfile = fs::read_to_string(jsfile_path)?;
        Ok(serde_json::from_str(&jsfile)?)
    }
}

/// Tests the argument variable replacement functionality.
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

    let res = replace_arguments(&args, &valuemap);

    assert_eq!(answer, res);
}
