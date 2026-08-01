//! Account-related methods for `ConfigHandler`.

use super::{ConfigHandler, UserAccount};
use anyhow::Result;
use mc_oauth::MinecraftAuthenticator;
use std::time::SystemTime;

impl ConfigHandler {
    /// Adds an offline account with the given username.
    ///
    /// # Example
    /// ```no_run
    /// use gluon::config::ConfigHandler;
    /// let mut config = ConfigHandler::read().unwrap();
    /// config.add_offline_account("Steve");
    /// ```
    pub fn add_offline_account(&mut self, name: &str) {
        *self.user_account_mut() = UserAccount::new_offline(name);
    }

    /// Adds a Microsoft account to the configuration.
    ///
    /// Initiates an interactive authentication process where the user
    /// must visit a URL and enter a code to authorize the application.
    ///
    /// # Errors
    /// - `anyhow::Error` if Microsoft device flow initialization fails
    /// - `anyhow::Error` if user authentication times out
    /// - `anyhow::Error` if Xbox Live authentication fails
    /// - `anyhow::Error` if Minecraft authentication fails
    pub fn add_microsoft_account(&mut self) -> anyhow::Result<()> {
        *self.user_account_mut() = UserAccount::new_microsoft()?;
        Ok(())
    }

    /// Refreshes the Microsoft account tokens using the stored refresh token.
    ///
    /// Runs the full refresh pipeline: Microsoft refresh token -> Xbox Live
    /// -> XSTS -> Minecraft, then updates the stored access token, the rotated
    /// refresh token (the old one becomes invalid) and the new expiry time.
    ///
    /// # Example
    /// ```no_run
    /// use gluon::config::ConfigHandler;
    ///
    /// let mut config = ConfigHandler::read().unwrap();
    /// config.refresh_account().unwrap();
    /// ```
    ///
    /// # Errors
    /// - `anyhow::Error` if no refresh token is stored (offline account or
    ///   old configuration)
    /// - `anyhow::Error` if any step of the refresh pipeline fails
    pub fn refresh_account(&mut self) -> Result<()> {
        let Some(refresh_token) = self.user_account().refresh_token.clone() else {
            return Err(anyhow::anyhow!(
                "No refresh token stored, run 'gluon account microsoft' first"
            ));
        };

        let authenticator = MinecraftAuthenticator::from_compile_env();
        let token_state = authenticator.refresh(&refresh_token)?;
        let minecraft_state = token_state
            .request_xbox_token()?
            .request_xsts_token()?
            .request_minecraft_token()?;

        let user = self.user_account_mut();
        user.access_token = Some(minecraft_state.minecraft_token_data.access_token.clone());
        // Token rotation: the refresh response contains a new refresh token,
        // the old one becomes invalid.
        user.refresh_token = Some(token_state.token_data.refresh_token.clone());
        user.token_expires_at = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(|e| anyhow::anyhow!("system time before unix epoch: {e}"))?
                .as_secs()
                + u64::from(minecraft_state.minecraft_token_data.expires_in),
        );
        Ok(())
    }

    /// Ensures the stored access token is still valid, refreshing it if expired.
    ///
    /// Uses a pure local check (no network) when the token is still valid.
    /// When expired, attempts a full refresh; on failure prints a warning and
    /// continues so the game can still be launched offline.
    ///
    /// # Example
    /// ```no_run
    /// use gluon::config::ConfigHandler;
    ///
    /// let mut config = ConfigHandler::read().unwrap();
    /// config.ensure_valid_token().unwrap();
    /// ```
    ///
    /// # Errors
    /// - `anyhow::Error` if the system clock cannot be read
    pub fn ensure_valid_token(&mut self) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| anyhow::anyhow!("system time before unix epoch: {e}"))?
            .as_secs();

        let Some(expires_at) = self.user_account().token_expires_at else {
            // Offline account, or old configuration without expiry info:
            // nothing to check or refresh.
            return Ok(());
        };

        if now < expires_at {
            return Ok(());
        }

        if let Err(e) = self.refresh_account() {
            eprintln!("Warning: token refresh failed: {e}");
            eprintln!(
                "Run 'gluon account refresh' to retry, or 'gluon account microsoft' to re-login"
            );
        }
        Ok(())
    }
}
