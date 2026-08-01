use anyhow::Result;
use mc_oauth::MinecraftAuthenticator;

/// Example demonstrating the complete Minecraft OAuth authentication flow
/// and token refresh.
///
/// This example shows how to:
/// 1. Get a device code for user authentication
/// 2. Poll for the access token after user completes authentication
/// 3. Authenticate with Xbox Live
/// 4. Get XSTS token
/// 5. Authenticate with Minecraft services
/// 6. Get the user's Minecraft profile
/// 7. Refresh the Microsoft access token using the refresh token
///    (token rotation: the old refresh token becomes invalid)
fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    let authenticator = MinecraftAuthenticator::from_compile_env();

    // Step 1-2: Start device flow and wait for user authorization
    let device_flow_state = authenticator.start_device_flow()?;
    println!("{}", device_flow_state.initial_response.message);
    let token_state = device_flow_state.wait_for_token()?;

    // Keep the refresh token for later token refresh
    let refresh_token = token_state.token_data.refresh_token.clone();

    // Step 3-6: Xbox Live -> XSTS -> Minecraft -> profile
    let profile = token_state
        .request_xbox_token()?
        .request_xsts_token()?
        .request_minecraft_token()?
        .fetch_minecraft_profile()?;
    log::debug!("profile: {profile:#?}");

    // Step 7: Refresh the Microsoft access token using the refresh token.
    // The returned state can be chained exactly like the original token state,
    // and the response contains a new refresh token (rotation).
    let refreshed_state = authenticator.refresh(&refresh_token)?;
    let refreshed_refresh_token = refreshed_state.token_data.refresh_token.clone();
    log::debug!("refreshed refresh token: {refreshed_refresh_token}");

    Ok(())
}
