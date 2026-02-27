use anyhow::{Result, anyhow};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MCAuth {
    client_id: String,
}

impl MCAuth {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

    pub fn get_device_code(&self) -> Result<DeviceCodeSession> {
        let param = json!({
            "client_id":self.client_id,
            "scope":"XboxLive.signin offline_access"
        });
        let client = reqwest::blocking::Client::new();
        let res = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .form(&param)
            .send()?;
        trace!("device code response :{:#?}", res);
        let res = res.json::<DeviceCodeResponse>()?;
        Ok(DeviceCodeSession {
            device_code_response: res,
            client_id: self.client_id.to_owned(),
        })
    }

    /// Complete OAuth flow from device code to final Minecraft authentication
    pub fn authenticate(&self) -> Result<MinecraftAuth> {
        // Step 1: Get device code
        let device_code_session = self.get_device_code()?;
        info!("{}", device_code_session.device_code_response.message);

        // Step 2: Poll for token
        let token_session = device_code_session.poll_for_token()?;
        info!("Got access token");

        // Step 3: Authenticate with Xbox Live
        let xbox_live_session = token_session.authenticate_xbox_live()?;
        info!("Authenticated with Xbox Live");

        // Step 4: Get XSTS token
        let xsts_session = xbox_live_session.get_xsts_token()?;
        info!("Got XSTS token");

        // Step 5: Authenticate with Minecraft
        let mc_auth_session = xsts_session.authenticate_minecraft()?;
        info!("Authenticated with Minecraft");

        // Step 6: Get Minecraft profile
        let profile = mc_auth_session.get_minecraft_profile()?;
        info!("Got Minecraft profile: {}", profile.name);

        Ok(MinecraftAuth {
            access_token: mc_auth_session.minecraft_auth_response.access_token,
            profile,
        })
    }
}

impl MCAuth {
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
pub struct DeviceCodeSession {
    pub device_code_response: DeviceCodeResponse,
    pub client_id: String,
}

impl DeviceCodeSession {
    pub fn poll_for_token(&self) -> Result<TokenSession> {
        let client = reqwest::blocking::Client::new();
        let max_attempts = 30; // Maximum number of polling attempts
        let mut attempts = 0;

        loop {
            let param = json!({
                "client_id": self.client_id,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": self.device_code_response.device_code,
            });

            let res = client
                .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
                .form(&param)
                .send()?;

            if res.status().is_success() {
                trace!("token response: {:#?}", res);
                return Ok(TokenSession {
                    token_response: res.json::<TokenResponse>()?,
                });
            } else {
                let error_response: TokenErrorResponse = res.json()?;
                match error_response.error.as_str() {
                    "authorization_pending" => {
                        // Still waiting for user to complete authentication
                        thread::sleep(Duration::from_secs(
                            self.device_code_response.interval as u64,
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
pub struct TokenSession {
    pub token_response: TokenResponse,
}

impl TokenSession {
    /// Authenticate with Xbox Live using Microsoft access token
    pub fn authenticate_xbox_live(&self) -> Result<XboxLiveAuthSession> {
        let auth_request = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}",self.token_response.access_token)
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
        let auth_response = res.json::<XboxLiveAuthResponse>()?;

        Ok(XboxLiveAuthSession {
            xbox_live_auth_response: auth_response,
        })
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
pub struct XboxLiveAuthSession {
    pub xbox_live_auth_response: XboxLiveAuthResponse,
}

impl XboxLiveAuthSession {
    pub fn get_xsts_token(&self) -> Result<XSTSAuthSession> {
        let auth_request = json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [
                    self.xbox_live_auth_response.token
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

        Ok(XSTSAuthSession {
            xsts_auth_response: res.json::<XSTSAuthResponse>()?,
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
pub struct XSTSAuthSession {
    pub xsts_auth_response: XSTSAuthResponse,
}

impl XSTSAuthSession {
    pub fn authenticate_minecraft(&self) -> Result<MineCraftAuthSession> {
        let auth_request = json!({
            "identityToken": format!("XBL3.0 x={};{}", self.xsts_auth_response.display_claims.xui[0].uhs, self.xsts_auth_response.token)
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

        Ok(MineCraftAuthSession {
            minecraft_auth_response: res.json::<MinecraftAuthResponse>()?,
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
pub struct MineCraftAuthSession {
    pub minecraft_auth_response: MinecraftAuthResponse,
}

impl MineCraftAuthSession {
    pub fn get_minecraft_profile(&self) -> Result<MinecraftProfile> {
        let client = reqwest::blocking::Client::new();
        let res = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header(
                "Authorization",
                format!("Bearer {}", self.minecraft_auth_response.access_token),
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
