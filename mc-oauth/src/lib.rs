//! Minecraft OAuth Authentication Library
//!
//! This library provides authentication functionality for Minecraft using Microsoft's OAuth 2.0 device code flow.
//! It handles the complete authentication pipeline from initial device code to final Minecraft profile access.
//!
//! # Authentication Flow
//!
//! The library implements the following authentication steps:
//!
//! 1. **Device Code Flow**: Initialize OAuth flow and get user verification code
//! 2. **Microsoft Token**: Exchange device code for Microsoft access token
//! 3. **Xbox Live Authentication**: Authenticate with Xbox Live using Microsoft token
//! 4. **XSTS Token**: Obtain Xbox Secure Token Service (XSTS) token
//! 5. **Minecraft Authentication**: Authenticate with Minecraft services using XSTS token
//! 6. **Profile Fetch**: Retrieve the user's Minecraft profile
//!
//! # Usage Example
//!
//! ```no_run
//! use mc_oauth::MinecraftAuthenticator;
//!
//! // Create authenticator with your Azure client ID
//! let authenticator = MinecraftAuthenticator::new("your_client_id");
//!
//! // Complete the full authentication flow
//! let auth = authenticator.authenticate()?;
//!
//! // Access the authentication result
//! println!("Username: {}", auth.profile.name);
//! println!("Access Token: {}", auth.access_token);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Error Handling
//!
//! All methods return `Result<T, anyhow::Error>` for comprehensive error handling.
//! Common errors include:
//! - Network failures during API calls
//! - Authentication timeouts if user doesn't complete the flow
//! - Invalid or expired tokens
//! - Account verification issues (age, region, etc.)
//!
//! # Environment Variables
//!
//! For production builds, you can use `MinecraftAuthenticator::from_compile_env()` which
//! requires the `AZURE_CLIENT_ID` environment variable to be set at compile time.
//!
//! # Features
//!
//! - Complete OAuth 2.0 device code flow implementation
//! - Xbox Live and XSTS token handling
//! - Minecraft profile fetching
//! - Comprehensive error messages for debugging
//! - Token validation and expiry handling

use anyhow::{Result, anyhow};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::thread;
use std::time::Duration;

/// Minecraft OAuth authenticator for handling Microsoft device code flow.
///
/// Manages the complete authentication process for Minecraft using Microsoft's OAuth 2.0
/// device code flow.
pub struct MinecraftAuthenticator {
    /// Azure application client ID for OAuth authentication.
    client_id: String,
}

impl MinecraftAuthenticator {
    /// Creates a new Minecraft OAuth authenticator with the specified Azure client ID.
    ///
    /// The `client_id` should be a valid Azure application client ID registered
    /// with Microsoft for OAuth authentication. This client ID will be used
    /// throughout the device code flow to authenticate with Microsoft services.
    ///
    /// # Example
    /// ```
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_azure_client_id");
    /// ```
    #[must_use]
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

    /// Initiates the OAuth device code flow by requesting a device code from Microsoft.
    ///
    /// Requests a device code and user verification instructions from Microsoft's OAuth endpoint.
    /// The returned `DeviceFlowState` contains the user code, verification URL, and timing
    /// information needed to guide the user through authentication.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let device_flow = authenticator.start_device_flow()?;
    ///
    /// // Display verification instructions to user
    /// println!("{}", device_flow.initial_response.message);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request to Microsoft's device code endpoint fails
    /// - Invalid client ID is provided
    /// - Microsoft's API returns an unexpected response
    /// - JSON parsing of the response fails
    pub fn start_device_flow(&self) -> Result<DeviceFlowState> {
        let param = json!({
            "client_id": self.client_id,
            "scope": "XboxLive.signin offline_access"
        });
        let client = reqwest::blocking::Client::new();
        let res = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .form(&param)
            .send()?;
        trace!("device code response: {res:#?}");
        let initial_response = res.json::<DeviceCodeResponse>()?;
        Ok(DeviceFlowState {
            initial_response,
            client_id: self.client_id.clone(),
        })
    }

    /// Completes the full OAuth authentication flow from device code to Minecraft profile.
    ///
    /// Handles the complete authentication pipeline including device code flow, Microsoft
    /// token acquisition, Xbox Live authentication, XSTS token, Minecraft authentication,
    /// and profile fetching.
    ///
    /// This method blocks until the user completes the device code verification on their device.
    /// The user typically has 15 minutes to complete verification.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    ///
    /// println!("Authenticated as: {}", auth.profile.name);
    /// println!("Access Token: {}", auth.access_token);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - User doesn't complete verification within the timeout period
    /// - User declines authorization
    /// - Network failures occur during any authentication step
    /// - Invalid credentials or tokens
    /// - Account verification issues (age, region restrictions)
    /// - Minecraft account not found or inactive
    pub fn authenticate(&self) -> Result<MinecraftAuth> {
        // Step 1: Start device flow
        let device_flow_state = self.start_device_flow()?;
        info!("{}", device_flow_state.initial_response.message);

        // Step 2: Wait for token
        let token_state = device_flow_state.wait_for_token()?;
        info!("Got access token");

        // Step 3: Request Xbox Live token
        let xbox_live_state = token_state.request_xbox_token()?;
        info!("Authenticated with Xbox Live");

        // Step 4: Request XSTS token
        let xsts_state = xbox_live_state.request_xsts_token()?;
        info!("Got XSTS token");

        // Step 5: Request Minecraft token
        let minecraft_state = xsts_state.request_minecraft_token()?;
        info!("Authenticated with Minecraft");

        // Step 6: Fetch Minecraft profile
        let profile = minecraft_state.fetch_minecraft_profile()?;
        info!("Got Minecraft profile: {}", profile.name);

        Ok(MinecraftAuth {
            access_token: minecraft_state.minecraft_token_data.access_token,
            profile,
        })
    }
}

impl MinecraftAuthenticator {
    /// Creates a Minecraft OAuth authenticator using the `AZURE_CLIENT_ID` environment variable.
    ///
    /// Reads the `AZURE_CLIENT_ID` environment variable at **compile time** and embeds
    /// it in the binary. This is useful for production builds where the client ID should
    /// not be configurable at runtime.
    ///
    /// # Example
    /// ```
    /// // Set AZURE_CLIENT_ID before compilation: export AZURE_CLIENT_ID="your_client_id"
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::from_compile_env();
    /// ```
    ///
    /// # Panics
    /// Panics if the `AZURE_CLIENT_ID` environment variable is not set at compile time.
    #[must_use]
    pub fn from_compile_env() -> Self {
        Self {
            client_id: env!("AZURE_CLIENT_ID").to_string(),
        }
    }
}

/// Response from Microsoft's device code endpoint.
///
/// Contains verification information for OAuth device code flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceCodeResponse {
    /// Long code used for token polling.
    pub device_code: String,
    /// Short code the user enters on the verification page.
    pub user_code: String,
    /// URL where the user completes authentication.
    pub verification_uri: String,
    /// Time in seconds until the device code expires (typically 900s = 15 minutes).
    pub expires_in: u32,
    /// Time in seconds between polling attempts (typically 5s).
    pub interval: u32,
    /// User-friendly message with verification instructions.
    pub message: String,
}

/// State representing the device code flow initialization.
///
/// Contains the device code response and client ID for token polling.
#[derive(Debug, Clone)]
pub struct DeviceFlowState {
    /// Device code response with verification information.
    pub initial_response: DeviceCodeResponse,
    /// Azure client ID for OAuth authentication.
    pub client_id: String,
}

impl DeviceFlowState {
    /// Polls Microsoft's token endpoint until the user completes authentication.
    ///
    /// Polls Microsoft's token endpoint at the interval specified in the device code response,
    /// waiting for the user to complete authentication on their device. Handles OAuth error states
    /// including pending authorization, declined authorization, and expired tokens.
    ///
    /// Polls every 5 seconds (default interval) with a maximum of 30 attempts (approximately 2.5 minutes).
    /// Returns immediately once authentication is complete.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let device_flow = authenticator.start_device_flow()?;
    ///
    /// // Display user instructions
    /// println!("{}", device_flow.initial_response.message);
    ///
    /// // Wait for user to complete authentication
    /// let token_state = device_flow.wait_for_token()?;
    /// println!("Authentication completed!");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - User doesn't complete verification within the timeout period (30 polling attempts)
    /// - User explicitly declines authorization
    /// - Device code expires before authentication completes
    /// - Network failures occur during polling
    /// - Invalid response from Microsoft's API
    pub fn wait_for_token(&self) -> Result<TokenState> {
        let client = reqwest::blocking::Client::new();
        let max_attempts = 30; // Maximum number of polling attempts
        let mut attempts = 0;

        loop {
            let param = json!({
                "client_id": self.client_id,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": self.initial_response.device_code,
            });

            let res = client
                .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
                .form(&param)
                .send()?;

            if res.status().is_success() {
                trace!("token response: {res:#?}");
                return Ok(TokenState {
                    token_data: res.json::<TokenResponse>()?,
                });
            }
            let error_response: TokenErrorResponse = res.json()?;
            match error_response.error.as_str() {
                "authorization_pending" => {
                    // Still waiting for user to complete authentication
                    thread::sleep(Duration::from_secs(u64::from(
                        self.initial_response.interval,
                    )));
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!(
                            "Timeout: User did not complete authentication in time"
                        ));
                    }
                }
                "authorization_declined" => {
                    return Err(anyhow!("Authorization declined by user"));
                }
                "expired_token" => {
                    return Err(anyhow!("Device code has expired"));
                }
                other => {
                    return Err(anyhow!("Unexpected error: {other} - {error_response:?}"));
                }
            }
        }
    }
}

/// Response from Microsoft's token endpoint containing OAuth tokens.
///
/// Contains the access token, refresh token, and related OAuth information.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenResponse {
    /// Token type (typically "Bearer").
    pub token_type: String,
    /// OAuth scopes granted by the token.
    pub scope: String,
    /// Time in seconds until token expiration.
    pub expires_in: u64,
    /// Access token for API calls.
    pub access_token: String,
    /// Token used to obtain new access tokens.
    pub refresh_token: String,
    /// Optional ID token containing user information.
    pub id_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TokenErrorResponse {
    error: String,
    #[allow(dead_code)]
    error_description: Option<String>,
}

/// State representing successful Microsoft token acquisition.
///
/// Contains the OAuth token response after device code authentication.
#[derive(Debug, Clone)]
pub struct TokenState {
    /// OAuth token response with access and refresh tokens.
    pub token_data: TokenResponse,
}

impl TokenState {
    /// Authenticates with Xbox Live using the Microsoft access token.
    ///
    /// Exchanges the Microsoft OAuth access token for an Xbox Live token using RPS
    /// (Relying Party Suite) authentication. The resulting Xbox Live token and user hash
    /// (UHS) are required for subsequent XSTS authentication.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let device_flow = authenticator.start_device_flow()?;
    /// let token_state = device_flow.wait_for_token()?;
    /// let xbox_state = token_state.request_xbox_token()?;
    ///
    /// println!("Xbox Live token: {}", xbox_state.xbox_auth_data.token);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request to Xbox Live authentication endpoint fails
    /// - Invalid Microsoft access token
    /// - Xbox Live authentication service unavailable
    /// - Invalid response format from Xbox Live API
    pub fn request_xbox_token(&self) -> Result<XboxLiveAuthState> {
        let auth_request = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", self.token_data.access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        });

        let client = reqwest::blocking::Client::new();
        let res = client
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .json(&auth_request)
            .header("x-xbl-contract-version", "1")
            .send()?;

        trace!("Xbox Live auth response: {res:#?}");
        let xbox_auth_data = res.json::<XboxLiveAuthResponse>()?;

        Ok(XboxLiveAuthState { xbox_auth_data })
    }
}

/// Response from Xbox Live authentication endpoint.
///
/// Contains the Xbox Live authentication token and user claims.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthResponse {
    /// Timestamp when token was issued.
    pub issue_instant: String,
    /// Timestamp when token expires.
    pub not_after: String,
    /// Xbox Live authentication token (JWT).
    pub token: String,
    /// User claims containing the user hash.
    pub display_claims: DisplayClaims,
}

/// Display claims containing user information.
///
/// Contains user-related claims from Xbox Live authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayClaims {
    /// Vector of Xbox user information claims.
    pub xui: Vec<XuiClaim>,
}

/// Xbox User Information (XUI) claim.
///
/// Contains user-specific information for Xbox Live authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XuiClaim {
    /// User hash string for identifying the user in Xbox Live services.
    pub uhs: String,
}

/// State representing successful Xbox Live authentication.
///
/// Contains the Xbox Live authentication data needed for XSTS token acquisition.
#[derive(Debug, Clone)]
pub struct XboxLiveAuthState {
    /// Xbox Live authentication response.
    pub xbox_auth_data: XboxLiveAuthResponse,
}

impl XboxLiveAuthState {
    /// Requests an XSTS token using the Xbox Live authentication.
    ///
    /// Exchanges the Xbox Live token for an XSTS (Xbox Secure Token Service) token,
    /// which is required for Minecraft authentication. Uses the RETAIL sandbox and
    /// `rp://api.minecraftservices.com/` as the relying party.
    ///
    /// Provides user-friendly error messages for common XSTS error codes:
    /// - `2148916233`: Account doesn't have an Xbox account
    /// - `2148916235`: Account from unsupported country/banned region
    /// - `2148916236`: Account needs adult verification
    /// - `2148916237`: Account needs age verification
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    /// // The authentication flow internally uses this method
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request to XSTS endpoint fails
    /// - Invalid Xbox Live token
    /// - Account verification issues (age, region)
    /// - XSTS service unavailable
    /// - Unknown error codes from XSTS service
    pub fn request_xsts_token(&self) -> Result<XSTSAuthState> {
        let auth_request = json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [
                    self.xbox_auth_data.token
                ]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        });

        let client = reqwest::blocking::Client::new();
        let res = client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&auth_request)
            .header("x-xbl-contract-version", "1")
            .send()?;

        trace!("XSTS auth response: {res:#?}");
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text()?;
            if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(&error_text)
                && let Some(xerr) = error_response.get("XErr")
            {
                return Err(anyhow!(
                    "XSTS authentication failed with error code {}: {}",
                    xerr,
                    Self::get_xsts_error_description(xerr.as_u64().unwrap_or(0))
                ));
            }
            return Err(anyhow!(
                "XSTS authentication failed: {status} - {error_text}",
            ));
        }

        Ok(XSTSAuthState {
            xsts_token_data: res.json::<XSTSAuthResponse>()?,
        })
    }

    fn get_xsts_error_description(error_code: u64) -> String {
        match error_code {
            2_148_916_233 => "The account doesn't have an Xbox account".to_string(),
            2_148_916_235 => {
                "The account is from a country where Xbox Live is not available/banned".to_string()
            }
            2_148_916_236 => "The account needs adult verification".to_string(),
            2_148_916_237 => "The account needs age verification".to_string(),
            _ => format!("Unknown error code: {error_code}"),
        }
    }
}

/// Response from XSTS authentication endpoint.
///
/// Contains the XSTS token needed for Minecraft authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XSTSAuthResponse {
    /// Timestamp when token was issued.
    pub issue_instant: String,
    /// Timestamp when token expires.
    pub not_after: String,
    /// XSTS authentication token (JWT).
    pub token: String,
    /// User claims containing the user hash.
    pub display_claims: DisplayClaims,
}

/// State representing successful XSTS authentication.
///
/// Contains the XSTS token data needed for Minecraft authentication.
#[derive(Debug, Clone)]
pub struct XSTSAuthState {
    /// XSTS authentication response.
    pub xsts_token_data: XSTSAuthResponse,
}

impl XSTSAuthState {
    /// Requests a Minecraft authentication token using the XSTS token.
    ///
    /// Exchanges the XSTS token for a Minecraft access token using the identity token
    /// format `XBL3.0 x={uhs};{xsts_token}`, where `uhs` is the user hash from XSTS
    /// claims and `xsts_token` is the XSTS token.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    /// // The authentication flow internally uses this method
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request to Minecraft authentication endpoint fails
    /// - Invalid XSTS token
    /// - Minecraft authentication service unavailable
    /// - Invalid response format from Minecraft API
    pub fn request_minecraft_token(&self) -> Result<MinecraftAuthState> {
        let auth_request = json!({
            "identityToken": format!(
                "XBL3.0 x={};{}",
                self.xsts_token_data.display_claims.xui[0].uhs,
                self.xsts_token_data.token
            )
        });

        let client = reqwest::blocking::Client::new();
        let res = client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&auth_request)
            .send()?;

        trace!("Minecraft auth response: {res:#?}");
        if !res.status().is_success() {
            return Err(anyhow!("Minecraft authentication failed: {}", res.status()));
        }

        Ok(MinecraftAuthState {
            minecraft_token_data: res.json::<MinecraftAuthResponse>()?,
        })
    }
}

/// Response from Minecraft authentication endpoint.
///
/// Contains the Minecraft access token and user information after successful XSTS-based authentication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftAuthResponse {
    /// User's Minecraft username (UUID format).
    pub username: String,
    /// List of roles assigned to the user.
    pub roles: Vec<String>,
    /// Minecraft access token for API calls.
    pub access_token: String,
    /// Token type (typically "Bearer").
    pub token_type: String,
    /// Time in seconds until token expiration.
    pub expires_in: u32,
}

/// State representing successful Minecraft authentication.
///
/// Contains the Minecraft authentication data needed for profile fetching.
#[derive(Debug, Clone)]
pub struct MinecraftAuthState {
    /// Minecraft authentication response.
    pub minecraft_token_data: MinecraftAuthResponse,
}

impl MinecraftAuthState {
    /// Fetches the user's Minecraft profile using the Minecraft access token.
    ///
    /// Retrieves the user's Minecraft profile information including their UUID, display
    /// username, and skin data. The user must own a valid Minecraft account for this
    /// request to succeed.
    ///
    /// # Example
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    ///
    /// println!("Username: {}", auth.profile.name);
    /// println!("UUID: {}", auth.profile.id);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - Network request to Minecraft profile endpoint fails
    /// - Invalid or expired Minecraft access token
    /// - Account doesn't own Minecraft (404 status)
    /// - Minecraft profile API unavailable
    /// - Invalid response format
    pub fn fetch_minecraft_profile(&self) -> Result<MinecraftProfile> {
        let client = reqwest::blocking::Client::new();
        let res = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header(
                "Authorization",
                format!("Bearer {}", self.minecraft_token_data.access_token),
            )
            .send()?;

        debug!("Minecraft profile response: {res:#?}");
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text()?;
            if status == 404 {
                return Err(anyhow!(
                    "Minecraft account not found. The account may not own Minecraft."
                ));
            }
            return Err(anyhow!(
                "Failed to get Minecraft profile: {status} - {error_text}",
            ));
        }

        Ok(res.json::<MinecraftProfile>()?)
    }
}

/// Minecraft skin information.
///
/// Contains metadata about a user's Minecraft skin texture.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftSkin {
    /// Unique identifier for this skin.
    pub id: String,
    /// State of the skin (e.g., "ACTIVE").
    pub state: String,
    /// URL where the skin texture can be downloaded.
    pub url: String,
    /// Skin variant (e.g., "CLASSIC", "SLIM").
    pub variant: String,
}

/// Minecraft player profile.
///
/// Contains player information including unique identifier, display name, and skin data.
///
/// The ID is a 32-character hexadecimal string representing the player's UUID,
/// formatted without hyphens. Players can change their usernames, so the username
/// may change over time while the UUID remains constant.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftProfile {
    /// Player's UUID (32-character hex string without hyphens).
    pub id: String,
    /// Player's display username.
    pub name: String,
    /// List of skin configurations for the player.
    pub skins: Vec<MinecraftSkin>,
}

/// Complete Minecraft authentication result.
///
/// Contains the final authentication data including the Minecraft access token and player profile.
/// Returned by `MinecraftAuthenticator::authenticate()`.
///
/// # Example
/// ```no_run
/// use mc_oauth::MinecraftAuthenticator;
///
/// let authenticator = MinecraftAuthenticator::new("your_client_id");
/// let auth = authenticator.authenticate()?;
///
/// println!("Authenticated as: {}", auth.profile.name);
/// println!("Access Token: {}", auth.access_token);
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct MinecraftAuth {
    /// Minecraft access token for API authentication.
    pub access_token: String,
    /// Player's Minecraft profile with UUID, username, and skin data.
    pub profile: MinecraftProfile,
}
