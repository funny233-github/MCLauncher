use anyhow::{Result, anyhow};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MinecraftAuthenticator {
    client_id: String,
}

impl MinecraftAuthenticator {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

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
        trace!("device code response: {:#?}", res);
        let initial_response = res.json::<DeviceCodeResponse>()?;
        Ok(DeviceFlowState {
            initial_response,
            client_id: self.client_id.to_owned(),
        })
    }

    /// Complete OAuth flow from device code to final Minecraft authentication
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
    pub fn from_compile_env() -> Self {
        Self {
            client_id: env!("AZURE_CLIENT_ID").to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DeviceFlowState {
    pub initial_response: DeviceCodeResponse,
    pub client_id: String,
}

impl DeviceFlowState {
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
                trace!("token response: {:#?}", res);
                return Ok(TokenState {
                    token_data: res.json::<TokenResponse>()?,
                });
            } else {
                let error_response: TokenErrorResponse = res.json()?;
                match error_response.error.as_str() {
                    "authorization_pending" => {
                        // Still waiting for user to complete authentication
                        thread::sleep(Duration::from_secs(
                            self.initial_response.interval as u64,
                        ));
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
                        return Err(anyhow!(
                            "Unexpected error: {} - {:?}",
                            other,
                            error_response
                        ));
                    }
                }
            }
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct TokenState {
    pub token_data: TokenResponse,
}

impl TokenState {
    /// Request Xbox Live token using Microsoft access token
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

        trace!("Xbox Live auth response: {:#?}", res);
        let xbox_auth_data = res.json::<XboxLiveAuthResponse>()?;

        Ok(XboxLiveAuthState { xbox_auth_data })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthResponse {
    pub issue_instant: String,
    pub not_after: String,
    pub token: String,
    pub display_claims: DisplayClaims,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayClaims {
    pub xui: Vec<XuiClaim>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XuiClaim {
    pub uhs: String,
}

#[derive(Debug, Clone)]
pub struct XboxLiveAuthState {
    pub xbox_auth_data: XboxLiveAuthResponse,
}

impl XboxLiveAuthState {
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

        trace!("XSTS auth response: {:#?}", res);
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text()?;
            if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(&error_text)
                && let Some(xerr) = error_response.get("XErr")
            {
                return Err(anyhow!(
                    "XSTS authentication failed with error code {}: {}",
                    xerr,
                    self.get_xsts_error_description(xerr.as_u64().unwrap_or(0))
                ));
            }
            return Err(anyhow!(
                "XSTS authentication failed: {} - {}",
                status,
                error_text
            ));
        }

        Ok(XSTSAuthState {
            xsts_token_data: res.json::<XSTSAuthResponse>()?,
        })
    }

    fn get_xsts_error_description(&self, error_code: u64) -> String {
        match error_code {
            2148916233 => "The account doesn't have an Xbox account".to_string(),
            2148916235 => {
                "The account is from a country where Xbox Live is not available/banned".to_string()
            }
            2148916236 => "The account needs adult verification".to_string(),
            2148916237 => "The account needs age verification".to_string(),
            _ => format!("Unknown error code: {}", error_code),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XSTSAuthResponse {
    pub issue_instant: String,
    pub not_after: String,
    pub token: String,
    pub display_claims: DisplayClaims,
}

#[derive(Debug, Clone)]
pub struct XSTSAuthState {
    pub xsts_token_data: XSTSAuthResponse,
}

impl XSTSAuthState {
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

        trace!("Minecraft auth response: {:#?}", res);
        if !res.status().is_success() {
            return Err(anyhow!("Minecraft authentication failed: {}", res.status()));
        }

        Ok(MinecraftAuthState {
            minecraft_token_data: res.json::<MinecraftAuthResponse>()?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftAuthResponse {
    pub username: String,
    pub roles: Vec<String>,
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
}

#[derive(Debug, Clone)]
pub struct MinecraftAuthState {
    pub minecraft_token_data: MinecraftAuthResponse,
}

impl MinecraftAuthState {
    pub fn fetch_minecraft_profile(&self) -> Result<MinecraftProfile> {
        let client = reqwest::blocking::Client::new();
        let res = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header(
                "Authorization",
                format!("Bearer {}", self.minecraft_token_data.access_token),
            )
            .send()?;

        debug!("Minecraft profile response: {:#?}", res);
        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text()?;
            if status == 404 {
                return Err(anyhow!(
                    "Minecraft account not found. The account may not own Minecraft."
                ));
            }
            return Err(anyhow!(
                "Failed to get Minecraft profile: {} - {}",
                status,
                error_text
            ));
        }

        Ok(res.json::<MinecraftProfile>()?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<MinecraftSkin>,
}

#[derive(Debug, Clone)]
pub struct MinecraftAuth {
    pub access_token: String,
    pub profile: MinecraftProfile,
}
