use anyhow::Result;
use mc_api::neoforge::Installer;

fn main() -> Result<()> {
    let data = Installer::fetch(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge",
        "21.11.38-beta",
    )?;
    println!("get installer success");
    data.extract("./mc/archive_test")?;
    println!("extract installer success");

    Ok(())
}
