use crate::config::{VersionManifestJson, VersionType};
use reqwest::header;

impl VersionManifestJson {
    pub fn new() -> anyhow::Result<VersionManifestJson> {
        let client = reqwest::blocking::Client::new();
        let data: VersionManifestJson = client
            .get("http://launchermeta.mojang.com/mc/game/version_manifest.json")
            .header(header::USER_AGENT, "mc_launcher")
            .send()?
            .json()?;
        Ok(data)
    }
    pub fn version_list(&self, version_type: VersionType) -> Vec<String> {
        match version_type {
            VersionType::All => self.versions.iter().map(|x| x.id.clone()).collect(),
            VersionType::Release => self
                .versions
                .iter()
                .filter(|x| x.r#type == "release")
                .map(|x| x.id.clone())
                .collect(),
            VersionType::Snapshot => self
                .versions
                .iter()
                .filter(|x| x.r#type == "snapshot")
                .map(|x| x.id.clone())
                .collect(),
        }
    }
}

#[test]
fn test_get_manifest() {
    let _ = VersionManifestJson::new().unwrap();
}
