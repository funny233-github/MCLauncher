use modrinth_api::Versions;

fn main() {
    println!("{:#?}", Versions::fetch_blocking("fabric-api").unwrap());
}
