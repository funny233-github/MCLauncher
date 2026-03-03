//! Minecraft launch arguments generation module.
//!
//! This module provides functionality to generate the complete command line arguments
//! needed to launch Minecraft, including JVM arguments, game arguments, and classpath.
//! It handles variable substitution for paths, user authentication, and game configuration.
//!
//! # Architecture
//!
//! The argument generation process involves:
//!
//! 1. **Base JVM arguments** - Standard memory settings and JVM flags
//! 2. **Version-specific JVM args** - From version manifest with variable substitution
//! 3. **Main class specification** - The entry point for Minecraft
//! 4. **Game arguments** - Version-specific game arguments with authentication data
//! 5. **Classpath construction** - All required libraries and client JAR
//!
//! # Variable Substitution
//!
//! The module supports variable substitution in the format `${variable_name}`:
//!
//! - `${natives_directory}` - Path to native libraries
//! - `${launcher_name}` - Launcher name
//! - `${launcher_version}` - Launcher version
//! - `${classpath}` - Complete Java classpath
//! - `${auth_player_name}` - Player username
//! - `${version_name}` - Game version
//! - `${game_directory}` - Game installation directory
//! - `${assets_root}` - Assets directory
//! - `${assets_index_name}` - Asset index identifier
//! - `${auth_uuid}` - Player UUID
//! - `${user_type}` - User account type
//! - `${version_type}` - Version type (release/snapshot)
//! - `${auth_access_token}` - OAuth access token (optional)
//!
//! # Example
//!
//! ```no_run
//! use launcher::config::ConfigHandler;
//!
//! let handler = ConfigHandler::read().expect("Failed to load config");
//!
//! let args = handler.args_provider().expect("Failed to generate arguments");
//!
//! // Use args to launch Minecraft
//! println!("Launch arguments: {:?}", args);
//! ```

use crate::config::ConfigHandler;
use anyhow::Result;
use mc_api::official::Version;
use regex::Regex;
use std::{collections::HashMap, fs, path::Path};

/// Classpath separator for Windows.
///
/// On Windows, classpath entries are separated by semicolons.
#[cfg(target_os = "windows")]
const CLASSPATH_SEPARATOR: &str = ";";

/// Classpath separator for Linux.
///
/// On Linux and Unix-like systems, classpath entries are separated by colons.
#[cfg(target_os = "linux")]
const CLASSPATH_SEPARATOR: &str = ":";

/// Classpath separator for macOS.
///
/// On macOS, classpath entries are separated by colons.
#[cfg(target_os = "macos")]
const CLASSPATH_SEPARATOR: &str = ":";

/// Replaces variable placeholders in arguments with actual values.
///
/// This function scans through argument strings and replaces variables in the
/// format `${variable_name}` with their corresponding values from the provided
/// value map. Variables not found in the map are left unchanged.
///
/// # Arguments
///
/// * `args` - Slice of argument strings to process
/// * `valuemap` - `HashMap` mapping variable names to their replacement values
///
/// # Returns
///
/// A new vector of strings with all recognized variables replaced.
///
/// # Examples
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// let valuemap = HashMap::from([
///     ("${version}", "1.16.5".into()),
///     ("${user}", "Player".into()),
/// ]);
/// let args = vec![
///     "--version=${version}".to_string(),
///     "--user=${user}".to_string(),
///     "${unknown_var}".to_string(),
/// ];
///
/// let result = replace_arguments(&args, &valuemap);
/// assert_eq!(result, vec![
///     "--version=1.16.5".to_string(),
///     "--user=Player".to_string(),
///     "${unknown_var}".to_string(),
/// ]);
/// ```
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
/// This function prepares a value map containing JVM-specific variables and
/// applies variable substitution to JVM arguments from the version manifest.
///
/// # Variables Replaced
///
/// - `${natives_directory}` - Path to native libraries directory
/// - `${launcher_name}` - Name of the launcher
/// - `${launcher_version}` - Version of the launcher
/// - `${classpath}` - Complete classpath for the game
///
/// # Arguments
///
/// * `args` - Slice of JVM argument strings to process
/// * `handle` - Configuration handler providing runtime settings
/// * `version_api` - Version metadata for the game
///
/// # Returns
///
/// A new vector of strings with JVM variables replaced.
///
/// # Errors
///
/// Returns an error if the classpath cannot be generated from the version metadata.
fn replace_arguments_from_jvm(
    args: &[String],
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
    Ok(replace_arguments(args, &valuemap))
}

/// Replaces game-specific variable placeholders in arguments.
///
/// This function prepares a value map containing game-specific variables including
/// user authentication data, game paths, and version information, then applies
/// variable substitution to game arguments from the version manifest.
///
/// # Variables Replaced
///
/// - `${auth_player_name}` - Player's username
/// - `${version_name}` - Minecraft version
/// - `${game_directory}` - Game installation directory
/// - `${assets_root}` - Assets directory path
/// - `${assets_index_name}` - Asset index identifier
/// - `${auth_uuid}` - Player's UUID
/// - `${user_type}` - User account type (e.g., "msa" for Microsoft account)
/// - `${version_type}` - Version type (typically "release")
/// - `${auth_access_token}` - OAuth access token (if available)
///
/// # Arguments
///
/// * `args` - Slice of game argument strings to process
/// * `handle` - Configuration handler providing runtime settings and user data
///
/// # Returns
///
/// A new vector of strings with game variables replaced.
///
/// # Errors
///
/// Returns an error if the version API cannot be read.
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
    /// This is the main entry point for generating Minecraft launch arguments.
    /// It combines base JVM settings, version-specific JVM arguments, the main class,
    /// and game-specific arguments into a complete command line.
    ///
    /// # Process
    ///
    /// 1. Add base JVM arguments (memory settings, GC, security flags)
    /// 2. Fetch version-specific JVM arguments from manifest
    /// 3. Replace JVM variables (paths, launcher info, classpath)
    /// 4. Add main class specification
    /// 5. Fetch version-specific game arguments from manifest
    /// 6. Replace game variables (user info, paths, version)
    /// 7. Return complete argument list
    ///
    /// # Base JVM Arguments
    ///
    /// - `-Xmx{max_memory_size}m` - Maximum heap size
    /// - `-Xmn256m` - Minimum heap size
    /// - `-XX:+UseG1GC` - Use G1 garbage collector
    /// - `-XX:-UseAdaptiveSizePolicy` - Disable adaptive size policy
    /// - `-XX:-OmitStackTraceInFastThrow` - Preserve stack traces
    /// - `-Dfml.ignoreInvalidMinecraftCertificates=True` - Forge compatibility
    /// - `-Dfml.ignorePatchDiscrepancies=True` - Forge compatibility
    /// - `-Dlog4j2.formatMsgNoLookups=true` - Log4j security fix
    /// - `-XX:HeapDumpPath=...` - Heap dump path configuration
    ///
    /// # Arguments
    ///
    /// * `self` - Reference to the configuration handler
    ///
    /// # Returns
    ///
    /// A vector of strings representing the complete command line for launching Minecraft.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Version API JSON cannot be read
    /// - Classpath cannot be generated
    /// - Game directory or version files cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use launcher::config::ConfigHandler;
    /// let handler = ConfigHandler::read().expect("Failed to load config");
    /// let args = handler.args_provider().expect("Failed to generate arguments");
    ///
    /// // The args can be passed directly to Process::Command:
    /// let mut cmd = std::process::Command::new("java");
    /// cmd.args(&args);
    /// ```
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
    /// The version manifest contains arrays of arguments that can be either
    /// strings or complex objects with conditions. This helper filters out only
    /// the string arguments.
    ///
    /// # Arguments
    ///
    /// * `js` - Mutable slice of JSON values representing arguments
    ///
    /// # Returns
    ///
    /// A vector containing only the string arguments.
    ///
    /// # Note
    ///
    /// Complex argument objects with conditions are ignored by this function.
    /// In a full implementation, these would need to be evaluated based on
    /// the runtime environment (OS, Java version, etc.).
    fn get_normal_args_from(js: &mut [serde_json::Value]) -> Vec<String> {
        js.iter()
            .filter(|x| x.is_string())
            .map(|x| x.as_str().unwrap().into())
            .collect()
    }

    /// Generates the complete Java classpath for the game.
    ///
    /// The classpath includes all required libraries and the client JAR file.
    /// This function handles library version resolution, ensuring that only the
    /// latest version of each library is included.
    ///
    /// # Library Selection Logic
    ///
    /// - Filters libraries for the current platform
    /// - Excludes older versions when a newer version of the same library exists
    /// - Includes the Minecraft client JAR at the end
    ///
    /// # Arguments
    ///
    /// * `self` - Reference to the configuration handler
    /// * `version_api` - Version metadata containing library information
    ///
    /// # Returns
    ///
    /// A platform-specific classpath string with entries separated by the
    /// appropriate separator (`:` on Unix-like systems, `;` on Windows).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A library version string cannot be parsed
    /// - A library path cannot be constructed
    /// - Version comparison fails
    ///
    /// # Example
    ///
    /// On Linux, the output might look like:
    /// ```text
    /// /game/lib/com/mojang/authlib:2.1.28/authlib-2.1.28.jar:/game/lib/org/lwjgl/lwjgl:3.2.2/lwjgl-3.2.2.jar:/game/versions/1.16.5/1.16.5.jar
    /// ```
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
    /// This function loads the version JSON file that was downloaded during
    /// the installation process and parses it into a structured Version object.
    ///
    /// # Arguments
    ///
    /// * `self` - Reference to the configuration handler
    ///
    /// # Returns
    ///
    /// A `Version` object containing parsed version metadata including:
    /// - Main class name
    /// - JVM arguments
    /// - Game arguments
    /// - Library dependencies
    /// - Asset index information
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The version JSON file cannot be read
    /// - The JSON cannot be parsed
    /// - The file is missing or inaccessible
    ///
    /// # File Location
    ///
    /// The version JSON is located at:
    /// `{game_dir}/versions/{game_version}/{game_version}.json`
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
///
/// Verifies that the `replace_arguments` function correctly replaces
/// known variables while leaving unknown variables unchanged.
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
