use modrinth_api::Projects;
use modrinth_api::Versions;

fn main() {
    println!("{:#?}", Versions::fetch_blocking("fabric-api").unwrap());
    println!(
        "{:#?}",
        Projects::fetch_blocking("fabric-api", Some(10)).unwrap()
    );
}
