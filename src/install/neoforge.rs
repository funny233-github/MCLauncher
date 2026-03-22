use super::install_dependencies;
use super::mavencoord::MavenCoord;
use super::mc_installer::MCInstaller;
use crate::config::MCLoader;
use crate::config::RuntimeConfig;
use anyhow::Result;
use installer::{InstallTask, TaskPool};
use mc_api::neoforge;
use mc_api::neoforge::{InstallerProfile, Profile};
use mc_api::official::{Version, VersionManifest};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Default)]
pub(super) struct NeoforgeInstaller;

impl MCInstaller for NeoforgeInstaller {
    fn install(config: &RuntimeConfig) -> Result<()> {
        let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
            return Err(anyhow::anyhow!("the loader is not neoforge"));
        };
        println!("fetch neoforge installer.jar");
        let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
        if !tmp_dir.exists() {
            let neoforge_jar =
                neoforge::Installer::fetch(&config.mirror.neoforge_neoforge, &neoforge_version)?;

            println!("extract neoforge installer.jar");
            neoforge_jar.extract(tmp_dir.to_str().unwrap())?;
            fs::write(
                tmp_dir.join("installer.jar"),
                neoforge_jar.installer.clone(),
            )?;
        }

        let version_json_file_path = Path::new(&config.game_dir)
            .join("versions")
            .join(&config.game_version)
            .join(config.game_version.clone() + ".json");

        if !version_json_file_path.exists() {
            let version = fetch_version(config)?;
            version.install(&version_json_file_path);
        }

        let native_dir = Path::new(&config.game_dir).join("natives");
        fs::create_dir_all(native_dir).unwrap_or(());

        let mut version_json_file = File::open(version_json_file_path)?;
        let mut content = String::new();
        version_json_file.read_to_string(&mut content)?;

        let version: Version = serde_json::from_str(&content)?;
        install_dependencies(config, &version)?;
        install_installer_dependencies(config)?;
        process_processors(config)?;
        Ok(())
    }
}

fn fetch_version(config: &RuntimeConfig) -> Result<Version> {
    let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
        return Err(anyhow::anyhow!("the loader is not neoforge"));
    };
    let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
    let version_json_file = tmp_dir.join("version.json");
    let profile = fs::read_to_string(version_json_file)?;
    let profile: Profile = serde_json::from_str(&profile)?;

    println!("fetching version manifest...");
    let manifest = VersionManifest::fetch(&config.mirror.version_manifest)?;

    if !manifest.versions.iter().any(|x| x.id == config.vanilla) {
        return Err(anyhow::anyhow!(
            "Cant' find the minecraft version {}",
            &config.game_version
        ));
    }
    println!("fetching version...");
    let mut version = Version::fetch(&manifest, &config.vanilla, &config.mirror.version_manifest)?;
    version.merge(&profile);
    Ok(version)
}

fn install_installer_dependencies(config: &RuntimeConfig) -> Result<()> {
    let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
        return Err(anyhow::anyhow!("the loader is not neoforge"));
    };
    let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
    let installer_profile = tmp_dir.join("install_profile.json");
    let installer_profile: InstallerProfile =
        serde_json::from_str(&fs::read_to_string(installer_profile)?)?;

    println!("fetching neoforge installer dependencies...");

    let tasks: VecDeque<InstallTask> = libraries_installtask(
        &format!("{}libraries/", config.game_dir),
        &installer_profile,
    );
    TaskPool::from(tasks).install();
    Ok(())
}

fn libraries_installtask(path: &str, profile: &InstallerProfile) -> VecDeque<InstallTask> {
    profile
        .libraries
        .iter()
        .map(|lib| {
            let artifact = lib.downloads.artifact.clone();
            let file_path = format!("{}/{}", path, artifact.path);
            InstallTask {
                url: artifact.url,
                sha1: artifact.sha1,
                save_file: file_path.into(),
                message: format!("neoforge installer lib {} installed", lib.name.clone()),
            }
        })
        .collect()
}

fn get_variables(config: &RuntimeConfig) -> Result<HashMap<String, String>> {
    println!("format variables");
    let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
        return Err(anyhow::anyhow!("the loader is not neoforge"));
    };
    let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
    let install_profile = tmp_dir.join("install_profile.json");
    let install_profile: InstallerProfile =
        serde_json::from_str(&fs::read_to_string(install_profile)?)?;

    let mut variables: HashMap<String, String> = HashMap::new();
    variables.insert("{SIDE}".into(), "client".into());

    let version_dir = format!("{}-neoforge-{}", config.vanilla, neoforge_version);
    let filename = format!("{}-neoforge-{}.jar", config.vanilla, neoforge_version);
    let path = Path::new(&config.game_dir)
        .join("versions")
        .join(version_dir)
        .join(&filename);
    let path = std::path::absolute(&path)?;
    variables.insert("{MINECRAFT_JAR}".into(), path.to_str().unwrap().to_string());

    variables.insert("{MINECRAFT_VERSION}".into(), config.vanilla.clone());
    variables.insert(
        "{INSTALLER}".into(),
        tmp_dir.join("installer.jar").to_str().unwrap().to_string(),
    );

    variables.insert("{ROOT}".into(), "{ROOT}".into());

    for (k, v) in install_profile.data {
        let value = match v.client.as_bytes() {
            [b'[', .., b']'] => {
                let coord = &v.client[1..v.client.len() - 1];
                let coord_path = MavenCoord::parse(coord).to_path_string();
                let path = Path::new(&config.game_dir)
                    .join("libraries")
                    .join(coord_path);
                let path = std::path::absolute(path)?;
                path.to_str().unwrap().to_string()
            }
            [b'\'', .., b'\''] => v.client[1..v.client.len() - 1].to_string(),
            _ => {
                if &k == "BINPATCH" {
                    tmp_dir.join(&v.client[1..]).to_str().unwrap().to_string()
                } else {
                    v.client.clone()
                }
            }
        };
        variables.insert(format!("{{{k}}}"), value);
    }

    log::debug!("variables:{variables:#?}");

    Ok(variables)
}

fn process_processors(config: &RuntimeConfig) -> Result<()> {
    println!("process processors");
    let MCLoader::Neoforge(neoforge_version) = config.loader.clone() else {
        return Err(anyhow::anyhow!("the loader is not neoforge"));
    };
    let tmp_dir = std::env::temp_dir().join(format!("neoforge-{neoforge_version}"));
    let install_profile = tmp_dir.join("install_profile.json");
    let install_profile: InstallerProfile =
        serde_json::from_str(&fs::read_to_string(install_profile)?)?;

    let variables = get_variables(config)?;

    for process in install_profile.processors {
        if match process.sides {
            Some(sides) => !sides.contains(&"client".to_string()),
            None => false,
        } {
            continue;
        }

        let mut args: Vec<String> = Vec::new();
        for (name, value) in process.args.chunks(2).map(|x| (x[0].clone(), x[1].clone())) {
            let mut value = value;
            for k in variables.keys() {
                if value.contains(k) {
                    value = value.replace(k, variables.get(k).unwrap());
                }
                if let [b'[', .., b']'] = value.as_bytes() {
                    let path = MavenCoord::parse(&value[1..value.len() - 1]).to_path_string();
                    let path = Path::new(&config.game_dir).join("libraries").join(path);
                    value = std::path::absolute(path)?.to_str().unwrap().to_string();
                }
            }
            args.push(name);
            args.push(value);
        }

        log::debug!("args:{args:#?}");

        let classpath = MavenCoord::parse(&process.classpath[0]).to_path_string();
        let classpath = tmp_dir.join(classpath);
        log::debug!("program path: {}", classpath.to_str().unwrap());

        let mut command = Command::new(&config.java_path)
            .args(["-jar", classpath.to_str().unwrap()])
            .args(args)
            .stdout(Stdio::piped())
            .spawn()?;
        io::copy(
            &mut command
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?,
            &mut io::stdout(),
        )?;
        command.wait()?;
    }
    Ok(())
}
