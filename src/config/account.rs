//! Account-related methods for `ConfigHandler`.

use super::{ConfigHandler, UserAccount};

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
}
