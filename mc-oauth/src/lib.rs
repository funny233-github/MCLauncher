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

#![allow(clippy::pedantic)]

use anyhow::{Result, anyhow};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::thread;
use std::time::Duration;

/// Minecraft OAuth authenticator for handling Microsoft device code flow.
///
/// This authenticator manages the complete authentication process for Minecraft
/// using Microsoft's OAuth 2.0 device code flow. It stores the Azure client ID
/// and provides methods to initiate and complete the authentication flow.
///
/// # Fields
///
/// * `client_id` - The Azure application client ID for OAuth authentication
#[derive(Debug, Clone)]
pub struct MinecraftAuthenticator {
    client_id: String,
}

impl MinecraftAuthenticator {
    /// Creates a new `MinecraftAuthenticator` with the specified Azure client ID.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The Azure application client ID for OAuth authentication
    ///
    /// # Returns
    ///
    /// Returns a new `MinecraftAuthenticator` instance configured with the provided client ID.
    ///
    /// # Example
    ///
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
    /// This method starts the device code authentication process by requesting a
    /// device code and user code from Microsoft's OAuth endpoint. The user will need
    /// to visit a verification URL and enter the provided user code to complete authentication.
    ///
    /// # Process
    ///
    /// 1. Sends a POST request to Microsoft's device code endpoint
    /// 2. Receives a device code and user verification instructions
    /// 3. Returns a `DeviceFlowState` containing the device code response
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid client ID is provided
    /// - Microsoft's API returns an unexpected response
    /// - JSON parsing fails
    ///
    /// # Example
    ///
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
    /// This is a convenience method that handles the complete authentication pipeline:
    ///
    /// 1. Initiates device code flow and displays user verification instructions
    /// 2. Polls for token completion (waits for user to complete verification)
    /// 3. Authenticates with Xbox Live
    /// 4. Obtains XSTS token
    /// 5. Authenticates with Minecraft services
    /// 6. Fetches the user's Minecraft profile
    ///
    /// # Blocking Behavior
    ///
    /// This method blocks until the user completes the device code verification
    /// on their device. The user has typically 15 minutes to complete verification.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - User doesn't complete verification within timeout period
    /// - User declines authorization
    /// - Network failures occur during any authentication step
    /// - Invalid credentials or tokens
    /// - Account verification issues (age, region restrictions)
    /// - Minecraft account not found or inactive
    ///
    /// # Example
    ///
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
    /// Creates a `MinecraftAuthenticator` using the `AZURE_CLIENT_ID` environment variable.
    ///
    /// This constructor reads the `AZURE_CLIENT_ID` environment variable at **compile time**
    /// and embeds it in the binary. This is useful for production builds where the client ID
    /// should not be configurable at runtime.
    ///
    /// # Compile-Time Requirement
    ///
    /// The `AZURE_CLIENT_ID` environment variable must be set **during compilation**:
    ///
    /// ```bash
    /// export AZURE_CLIENT_ID="your_client_id"
    /// cargo build
    /// ```
    ///
    /// # Panics
    ///
    /// This method will cause a compilation failure if `AZURE_CLIENT_ID` is not set.
    ///
    /// # Returns
    ///
    /// Returns a new `MinecraftAuthenticator` instance with the client ID from the environment variable.
    ///
    /// # Example
    ///
    /// ```rust,should_panic
    /// // Set AZURE_CLIENT_ID before compilation
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::from_compile_env();
    /// ```
    #[must_use]
    pub fn from_compile_env() -> Self {
        Self {
            client_id: env!("AZURE_CLIENT_ID").to_string(),
        }
    }
}

/// Response from Microsoft's device code endpoint.
///
/// Contains information needed for the user to complete the OAuth device code flow,
/// including the verification URL, user code, and timing information.
///
/// # Fields
///
/// * `device_code` - Long code used for token polling
/// * `user_code` - Short code the user enters on the verification page
/// * `verification_uri` - URL where the user should complete authentication
/// * `expires_in` - Time in seconds until the device code expires (typically 900s = 15 minutes)
/// * `interval` - Time in seconds between polling attempts (typically 5s)
/// * `message` - User-friendly message with verification instructions
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
    pub message: String,
}

/// State representing the device code flow initialization.
///
/// Contains the initial response from the device code endpoint and the client ID
/// needed for subsequent token polling.
///
/// # Fields
///
/// * `initial_response` - The device code response with verification information
/// * `client_id` - The Azure client ID for OAuth authentication
#[derive(Debug, Clone)]
pub struct DeviceFlowState {
    pub initial_response: DeviceCodeResponse,
    pub client_id: String,
}

impl DeviceFlowState {
    /// Polls Microsoft's token endpoint until the user completes authentication.
    ///
    /// This method polls Microsoft's token endpoint at the interval specified in the
    /// device code response, waiting for the user to complete authentication on their device.
    ///
    /// # Polling Behavior
    ///
    /// - Polls every `interval` seconds (typically 5 seconds)
    /// - Maximum of 30 polling attempts (approximately 2.5 minutes total)
    /// - Returns immediately once authentication is complete
    /// - Sleeps between polls to avoid rate limiting
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - User doesn't complete verification within the timeout period
    /// - User explicitly declines authorization
    /// - Device code expires before authentication completes
    /// - Network failures occur during polling
    /// - Invalid response from Microsoft's API
    ///
    /// # Authentication States
    ///
    /// The method handles several OAuth error states:
    /// - `authorization_pending`: Still waiting for user (continues polling)
    /// - `authorization_declined`: User declined authorization (returns error)
    /// - `expired_token`: Device code expired (returns error)
    ///
    /// # Example
    ///
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
/// Contains the access token, refresh token, and related OAuth information after
/// successful device code authentication.
///
/// # Fields
///
/// * `token_type` - Type of token (typically "Bearer")
/// * `scope` - OAuth scopes granted by the token
/// * `expires_in` - Time in seconds until token expiration
/// * `access_token` - The access token for API calls
/// * `refresh_token` - Token used to obtain new access tokens
/// * `id_token` - Optional ID token containing user information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenResponse {
    pub token_type: String,
    pub scope: String,
    pub expires_in: u64,
    pub access_token: String,
    pub refresh_token: String,
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
/// Contains the token response after the user completes device code authentication.
///
/// # Fields
///
/// * `token_data` - The OAuth token response with access and refresh tokens
#[derive(Debug, Clone)]
pub struct TokenState {
    pub token_data: TokenResponse,
}

impl TokenState {
    /// Authenticates with Xbox Live using the Microsoft access token.
    ///
    /// This method exchanges the Microsoft OAuth access token for an Xbox Live token,
    /// which is required for subsequent authentication steps.
    ///
    /// # Process
    ///
    /// 1. Constructs authentication request with Microsoft access token
    /// 2. Sends POST request to Xbox Live authentication endpoint
    /// 3. Receives Xbox Live token and user hash (UHS)
    ///
    /// # Authentication Details
    ///
    /// - Uses RPS (Relying Party Suite) authentication method
    /// - Authenticates with `http://auth.xboxlive.com` as the relying party
    /// - Returns JWT token for Xbox Live authentication
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid Microsoft access token
    /// - Xbox Live authentication service unavailable
    /// - Invalid response format from Xbox Live API
    ///
    /// # Example
    ///
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
///
/// # Fields
///
/// * `issue_instant` - Timestamp when token was issued
/// * `not_after` - Timestamp when token expires
/// * `token` - The Xbox Live authentication token (JWT)
/// * `display_claims` - User claims containing the user hash (UHS)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthResponse {
    pub issue_instant: String,
    pub not_after: String,
    pub token: String,
    pub display_claims: DisplayClaims,
}

/// Display claims containing user information.
///
/// Contains user-related claims from Xbox Live authentication, including
/// the user hash needed for subsequent authentication steps.
///
/// # Fields
///
/// * `xui` - Vector of Xbox user information claims
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayClaims {
    pub xui: Vec<XuiClaim>,
}

/// Xbox User Information (XUI) claim.
///
/// Contains user-specific information needed for Xbox Live authentication.
///
/// # Fields
///
/// * `uhs` - User hash string, used to identify the user in Xbox Live services
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XuiClaim {
    pub uhs: String,
}

/// State representing successful Xbox Live authentication.
///
/// Contains the Xbox Live authentication data needed for XSTS token acquisition.
///
/// # Fields
///
/// * `xbox_auth_data` - The Xbox Live authentication response
#[derive(Debug, Clone)]
pub struct XboxLiveAuthState {
    pub xbox_auth_data: XboxLiveAuthResponse,
}

impl XboxLiveAuthState {
    /// Requests an XSTS token using the Xbox Live authentication.
    ///
    /// This method exchanges the Xbox Live token for an XSTS (Xbox Secure Token Service)
    /// token, which is required for Minecraft authentication.
    ///
    /// # Process
    ///
    /// 1. Constructs XSTS authorization request with Xbox Live token
    /// 2. Sends POST request to XSTS authorize endpoint
    /// 3. Handles common XSTS error codes with user-friendly messages
    ///
    /// # XSTS Error Codes
    ///
    /// The method provides user-friendly messages for common XSTS errors:
    /// - `2148916233`: Account doesn't have an Xbox account
    /// - `2148916235`: Account from unsupported country/banned region
    /// - `2148916236`: Account needs adult verification
    /// - `2148916237`: Account needs age verification
    ///
    /// # Sandbox Configuration
    ///
    /// Uses "RETAIL" sandbox ID for production Minecraft authentication.
    ///
    /// # Relying Party
    ///
    /// The token is requested for `rp://api.minecraftservices.com/` as the relying party.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid Xbox Live token
    /// - Account verification issues (age, region)
    /// - XSTS service unavailable
    /// - Unknown error codes from XSTS service
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    /// // The authentication flow internally uses this method
    /// # Ok::<(), anyhow::Error>(())
    /// ```
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
///
/// # Fields
///
/// * `issue_instant` - Timestamp when token was issued
/// * `not_after` - Timestamp when token expires
/// * `token` - The XSTS authentication token (JWT)
/// * `display_claims` - User claims containing the user hash
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XSTSAuthResponse {
    pub issue_instant: String,
    pub not_after: String,
    pub token: String,
    pub display_claims: DisplayClaims,
}

/// State representing successful XSTS authentication.
///
/// Contains the XSTS token data needed for Minecraft authentication.
///
/// # Fields
///
/// * `xsts_token_data` - The XSTS authentication response
#[derive(Debug, Clone)]
pub struct XSTSAuthState {
    pub xsts_token_data: XSTSAuthResponse,
}

impl XSTSAuthState {
    /// Requests a Minecraft authentication token using the XSTS token.
    ///
    /// This method exchanges the XSTS token for a Minecraft access token,
    /// which can be used to access Minecraft services and fetch the user profile.
    ///
    /// # Process
    ///
    /// 1. Constructs Minecraft identity token from XSTS token and user hash
    /// 2. Sends POST request to Minecraft authentication endpoint
    /// 3. Receives Minecraft access token and user information
    ///
    /// # Identity Token Format
    ///
    /// The identity token follows the format: `XBL3.0 x={uhs};{xsts_token}`
    /// where `uhs` is the user hash from XSTS claims and `xsts_token` is the XSTS token.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid XSTS token
    /// - Minecraft authentication service unavailable
    /// - Invalid response format from Minecraft API
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_oauth::MinecraftAuthenticator;
    ///
    /// let authenticator = MinecraftAuthenticator::new("your_client_id");
    /// let auth = authenticator.authenticate()?;
    /// // The authentication flow internally uses this method
    /// # Ok::<(), anyhow::Error>(())
    /// ```
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
/// Contains the Minecraft access token and user information after successful
/// XSTS-based authentication.
///
/// # Fields
///
/// * `username` - The user's Minecraft username (UUID format)
/// * `roles` - List of roles assigned to the user
/// * `access_token` - The Minecraft access token for API calls
/// * `token_type` - Type of token (typically "Bearer")
/// * `expires_in` - Time in seconds until token expiration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftAuthResponse {
    pub username: String,
    pub roles: Vec<String>,
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
}

/// State representing successful Minecraft authentication.
///
/// Contains the Minecraft authentication data needed for profile fetching.
///
/// # Fields
///
/// * `minecraft_token_data` - The Minecraft authentication response
#[derive(Debug, Clone)]
pub struct MinecraftAuthState {
    pub minecraft_token_data: MinecraftAuthResponse,
}

impl MinecraftAuthState {
    /// Fetches the user's Minecraft profile using the Minecraft access token.
    ///
    /// This method retrieves the user's Minecraft profile information, including
    /// their username, UUID, and skin data.
    ///
    /// # Profile Information
    ///
    /// The profile contains:
    /// - User UUID (unique identifier)
    /// - Display username
    /// - Skin information (ID, URL, variant, state)
    ///
    /// # Authentication Requirement
    ///
    /// The user must own a valid Minecraft account. If the account doesn't own
    /// Minecraft, the API returns a 404 status.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails
    /// - Invalid Minecraft access token
    /// - Account doesn't own Minecraft (404 status)
    /// - Minecraft profile API unavailable
    /// - Invalid response format
    ///
    /// # Common Issues
    ///
    /// - **404 Not Found**: Account exists but doesn't own Minecraft
    /// - **401 Unauthorized**: Invalid or expired access token
    /// - **500+**: Minecraft API service issues
    ///
    /// # Example
    ///
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
///
/// # Fields
///
/// * `id` - Unique identifier for this skin
/// * `state` - State of the skin (e.g., "ACTIVE")
/// * `url` - URL where the skin texture can be downloaded
/// * `variant` - Skin variant (e.g., "CLASSIC", "SLIM")
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}

/// Minecraft player profile.
///
/// Contains comprehensive information about a Minecraft player, including
/// their unique identifier, display name, and skin data.
///
/// # Fields
///
/// * `id` - The player's UUID (unique identifier, 32-character hex string)
/// * `name` - The player's display username
/// * `skins` - List of skin configurations for the player
///
/// # UUID Format
///
/// The ID is a 32-character hexadecimal string representing the player's UUID,
/// formatted without hyphens (e.g., "1234567890abcdef1234567890abcdef").
///
/// # Username
///
/// The username is the player's current display name. Note that players can
/// change their usernames, so the username may change over time while the UUID
/// remains constant.
///
/// # Skin Data
///
/// The skins array contains all configured skins for the player. Typically,
/// only one skin is active at a time, but players may have multiple historical
/// skin configurations.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<MinecraftSkin>,
}

/// Complete Minecraft authentication result.
///
/// Contains the final authentication data after completing the entire OAuth flow,
/// including the Minecraft access token and player profile.
///
/// # Usage
///
/// This struct is returned by `MinecraftAuthenticator::authenticate()` and contains
/// all the information needed to make authenticated API calls to Minecraft services.
///
/// # Fields
///
/// * `access_token` - The Minecraft access token for API authentication
/// * `profile` - The player's Minecraft profile with UUID, username, and skin data
///
/// # Token Usage
///
/// The access token can be used to authenticate requests to Minecraft's API
/// by including it in the `Authorization` header:
///
/// ```text
/// Authorization: Bearer <access_token>
/// ```
///
/// # Example
///
/// ```no_run
/// use mc_oauth::MinecraftAuthenticator;
///
/// let authenticator = MinecraftAuthenticator::new("your_client_id");
/// let auth = authenticator.authenticate()?;
///
/// println!("Successfully authenticated as: {}", auth.profile.name);
/// println!("UUID: {}", auth.profile.id);
/// println!("Access Token: {}", auth.access_token);
///
/// // Use the access token for API calls
/// let client = reqwest::blocking::Client::new();
/// let response = client
///     .get("https://api.minecraftservices.com/some/endpoint")
///     .header("Authorization", format!("Bearer {}", auth.access_token))
///     .send()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct MinecraftAuth {
    pub access_token: String,
    pub profile: MinecraftProfile,
}
