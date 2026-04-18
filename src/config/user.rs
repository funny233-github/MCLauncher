//! User account structures for the Minecraft launcher.
//!
//! Contains user authentication information for both offline mode
//! and Microsoft account authentication.

use mc_oauth::MinecraftAuthenticator;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User account information for authentication.
///
/// Contains user details and access token for either offline mode
/// or Microsoft account authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserAccount {
    /// Display username.
    pub user_name: String,
    /// Account type ("offline" or "msa").
    pub user_type: String,
    /// User UUID string.
    pub user_uuid: String,
    /// Access token for Microsoft accounts.
    pub access_token: Option<String>,
}

impl Default for UserAccount {
    /// Creates a default offline user account with a generated UUID.
    fn default() -> Self {
        Self {
            user_name: "noname".to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
            access_token: None,
        }
    }
}

impl UserAccount {
    /// Creates an offline account with the given username.
    ///
    /// # Example
    /// ```
    /// use gluon::config::UserAccount;
    /// let account = UserAccount::new_offline("Steve");
    /// assert_eq!(account.user_name, "Steve");
    /// assert_eq!(account.user_type, "offline");
    /// ```
    #[must_use]
    pub fn new_offline(name: &str) -> Self {
        Self {
            user_name: name.to_owned(),
            user_type: "offline".to_owned(),
            user_uuid: Uuid::new_v4().to_string(),
            access_token: None,
        }
    }

    /// Creates a new Microsoft account by authenticating through device code flow.
    ///
    /// This method initiates an interactive authentication process where the user
    /// must visit a URL and enter a code to authorize the application.
    ///
    /// # Errors
    /// - `anyhow::Error` if Microsoft device flow initialization fails
    /// - `anyhow::Error` if user authentication times out
    /// - `anyhow::Error` if Xbox Live authentication fails
    /// - `anyhow::Error` if Minecraft authentication fails
    pub fn new_microsoft() -> anyhow::Result<Self> {
        // Step 1: Start device flow
        let device_flow_state = MinecraftAuthenticator::from_compile_env().start_device_flow()?;
        println!("{}", device_flow_state.initial_response.message);

        // Step 2: Wait for token
        let token_state = device_flow_state.wait_for_token()?;
        println!("Got access token");

        // Step 3: Request Xbox Live token
        let xbox_live_state = token_state.request_xbox_token()?;
        println!("Authenticated with Xbox Live");

        // Step 4: Request XSTS token
        let xsts_state = xbox_live_state.request_xsts_token()?;
        println!("Got XSTS token");

        // Step 5: Request Minecraft token
        let minecraft_state = xsts_state.request_minecraft_token()?;
        println!("Authenticated with Minecraft");

        // Step 6: Fetch Minecraft profile
        let profile = minecraft_state.fetch_minecraft_profile()?;
        println!("Got Minecraft profile: {}", profile.name);
        Ok(Self {
            user_name: profile.name,
            user_type: "msa".into(),
            user_uuid: profile.id,
            access_token: minecraft_state.minecraft_token_data.access_token.into(),
        })
    }
}
