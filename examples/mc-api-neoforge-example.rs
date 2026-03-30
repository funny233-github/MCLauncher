use anyhow::Result;
use mc_api::neoforge::{Installer, Loader};

fn main() -> Result<()> {
    // let data = Loader::fetch("https://maven.neoforged.net/releases/net/neoforged/neoforge")?;
    // println!("data:{data:#?}");

    let data = Installer::fetch(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge",
        "21.11.38-beta",
    )?;
    println!("get installer success");
    data.extract("./mc/archive_test")?;
    println!("extract installer success");

    Ok(())
}
