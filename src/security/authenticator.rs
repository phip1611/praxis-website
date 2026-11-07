// SPDX-License-Identifier: MIT OR Apache-2.0

//! The login flow, combining credentials and sessions.

use crate::clock::Clock;
use crate::security::credentials::CredentialStore;
use crate::security::session::SessionStore;
use crate::security::token::{RandomError, SecretToken};
use std::sync::Arc;
use thiserror::Error;
use tokio::task;
use tracing::{info, warn};

/// The account name in logs for usernames without an account. Typed names
/// aren't logged, as they may be mistyped passwords.
const UNKNOWN_ACCOUNT: &str = "<unknown>";

/// Why a login failed.
#[derive(Debug, Error)]
pub(crate) enum LoginError {
    /// The username or password is wrong. Doesn't reveal which of them.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// The session couldn't be created.
    #[error(transparent)]
    Random(#[from] RandomError),
    /// The password check task failed unexpectedly.
    #[error("the password check task failed")]
    TaskFailed,
}

/// Performs logins and owns the session table.
#[derive(Debug)]
pub(crate) struct Authenticator {
    /// The accounts that may log in.
    credentials: Arc<CredentialStore>,
    /// The active sessions.
    sessions: SessionStore,
}

impl Authenticator {
    /// Creates an authenticator for the given accounts.
    pub(crate) fn new(credentials: Arc<CredentialStore>, clock: &Arc<dyn Clock>) -> Self {
        Self {
            credentials,
            sessions: SessionStore::new(Arc::clone(clock)),
        }
    }

    /// Returns the active sessions.
    pub(crate) const fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    /// Checks the credentials and creates a session.
    ///
    /// Only the names of existing accounts are logged, as people sometimes
    /// type their password into the username field.
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with the reason. See there what can be revealed
    /// to the client.
    pub(crate) async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SecretToken, LoginError> {
        let known_account = self.credentials.find(username);
        let account = known_account.as_deref().unwrap_or(UNKNOWN_ACCOUNT);

        // The check needs one CPU core for about 20 milliseconds, so it runs
        // outside of the async workers.
        let credentials = Arc::clone(&self.credentials);
        let (username, password) = (username.to_string(), password.to_string());
        let verified =
            task::spawn_blocking(move || credentials.verify(&username, &password).is_some())
                .await
                .map_err(|_| LoginError::TaskFailed)?;

        if !verified {
            warn!(account, "login failed");
            return Err(LoginError::InvalidCredentials);
        }
        info!(account, "login succeeded");
        Ok(self.sessions.create(account)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockClock, TEST_PASSWORD, TEST_USER, accounts_file};
    use std::assert_matches;

    /// Creates an authenticator with the test account.
    fn authenticator() -> Authenticator {
        let clock: Arc<dyn Clock> = Arc::new(MockClock::default());
        let credentials = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("fixture should be valid");
        Authenticator::new(Arc::new(credentials), &clock)
    }

    /// Valid credentials create a session for the account.
    #[tokio::test]
    async fn login_creates_session() {
        let auth = authenticator();

        let token = auth
            .login(TEST_USER, TEST_PASSWORD)
            .await
            .expect("login should succeed");

        let session = auth.sessions().authenticate(token.as_str());
        assert_eq!(session.expect("session should exist").username, TEST_USER);
    }

    /// Unknown users get the same error as wrong passwords.
    #[tokio::test]
    async fn login_does_not_reveal_existing_users() {
        let auth = authenticator();

        assert_matches!(
            auth.login(TEST_USER, "wrong").await,
            Err(LoginError::InvalidCredentials)
        );
        assert_matches!(
            auth.login("nobody", "wrong").await,
            Err(LoginError::InvalidCredentials)
        );
    }
}
