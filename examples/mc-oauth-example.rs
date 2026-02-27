use anyhow::Result;
use mc_oauth::MinecraftAuthenticator;

/// Example demonstrating the complete Minecraft OAuth authentication flow
///
/// This example shows how to:
/// 1. Get a device code for user authentication
/// 2. Poll for the access token after user completes authentication
/// 3. Authenticate with Xbox Live
/// 4. Get XSTS token
/// 5. Authenticate with Minecraft services
/// 6. Get the user's Minecraft profile
fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    let res = MinecraftAuthenticator::from_compile_env().authenticate()?;
    log::debug!("profile: {:#?}", res);

    Ok(())
}
