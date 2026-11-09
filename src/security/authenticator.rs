// SPDX-License-Identifier: MIT OR Apache-2.0

//! The login flow, combining credentials, throttling, and sessions.
//!
//! Failed logins are limited on three levels:
//!
//! 1. Per client IP, if the proxy header is configured. This stops guessing
//!    from one client without affecting anyone else. The throttle only keeps
//!    pseudonyms, see [`ClientPseudonyms`], and forgets them within
//!    [`SWEEP_INTERVAL`] after the throttle window.
//! 2. Per account, with a higher limit. This stops guessing that is spread
//!    over many clients. Such an attacker can block the account, but needs
//!    several clients to do so.
//! 3. Globally, see [`rate_limit`], as safety net for the CPU.

use crate::clock::Clock;
use crate::hex;
use crate::security::client_ip::ClientPseudonyms;
use crate::security::credentials::CredentialStore;
use crate::security::rate_limit::{self, Decision, RateLimit};
use crate::security::session::SessionStore;
use crate::security::throttle::LoginThrottle;
use crate::security::token::{RandomError, SecretToken, digest};
use jiff::SignedDuration;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task;
use tokio::time::{self, MissedTickBehavior};
use tracing::{debug, info, warn};

/// How many password checks may run at once.
///
/// Each check needs about 19 MiB of memory and one CPU core for about 20
/// milliseconds. The limit keeps a flood of logins from exhausting either.
const MAX_CONCURRENT_VERIFICATIONS: usize = 4;

/// The failed logins per client within the throttle window.
pub(crate) const MAX_FAILURES_PER_CLIENT: usize = 5;

/// The failed logins per account within the throttle window. Higher than per
/// client, so that a single client can't block an account.
const MAX_FAILURES_PER_ACCOUNT: usize = 20;

/// How often failures that left the throttle window are forgotten.
const SWEEP_INTERVAL: Duration = Duration::from_mins(1);

/// The account name in logs for usernames without an account. Typed names
/// aren't logged, as they may be mistyped passwords.
const UNKNOWN_ACCOUNT: &str = "<unknown>";

/// The prefix of throttle keys for usernames without an account. It contains
/// `:`, so it can't collide with a valid username.
const UNKNOWN_KEY_PREFIX: &str = "unknown:";

/// Returns the throttle key of `username`.
///
/// Unknown names are throttled one by one like real accounts, so that the
/// answers don't reveal which accounts exist. Their keys are digests, so that
/// typed text (possibly passwords) isn't kept in memory.
fn account_key(known_account: Option<&str>, username: &str) -> String {
    if let Some(account) = known_account {
        return account.to_string();
    }
    format!(
        "{UNKNOWN_KEY_PREFIX}{}",
        hex::encode(&digest(username)[..8])
    )
}

/// Why a login failed.
#[derive(Debug, Error)]
pub(crate) enum LoginError {
    /// The username or password is wrong. Doesn't reveal which of them.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// There were too many recent failures of this client or account.
    #[error("too many failed logins")]
    Throttled {
        /// How long the client has to wait.
        retry_after: SignedDuration,
    },
    /// Too many password checks are running or ran recently.
    #[error("too many logins")]
    Busy,
    /// The session couldn't be created.
    #[error(transparent)]
    Random(#[from] RandomError),
    /// The password check task failed unexpectedly.
    #[error("the password check task failed")]
    TaskFailed,
}

/// Why a password check may not run now.
#[derive(Debug)]
enum Refusal {
    /// The client or account is blocked for this long.
    Throttled(SignedDuration),
    /// Too many checks are running or ran recently.
    Busy,
}

/// Performs logins and owns the session table.
#[derive(Debug)]
pub(crate) struct Authenticator {
    /// The accounts that may log in.
    credentials: Arc<CredentialStore>,
    /// Turns client addresses into throttle keys.
    client_pseudonyms: ClientPseudonyms,
    /// The limit for failed logins per client.
    client_throttle: LoginThrottle,
    /// The limit for failed logins per account.
    account_throttle: LoginThrottle,
    /// The limit for password checks of all clients together.
    rate_limit: RateLimit,
    /// The active sessions.
    sessions: SessionStore,
    /// Limits the concurrent password checks.
    verification_slots: Arc<Semaphore>,
}

impl Authenticator {
    /// Creates an authenticator for the given accounts and their changed
    /// passwords.
    ///
    /// # Errors
    ///
    /// Returns [`RandomError`] if the key for client pseudonyms can't be
    /// generated.
    pub(crate) fn new(
        credentials: Arc<CredentialStore>,
        clock: &Arc<dyn Clock>,
    ) -> Result<Self, RandomError> {
        Ok(Self {
            credentials,
            client_pseudonyms: ClientPseudonyms::generate()?,
            client_throttle: LoginThrottle::new(Arc::clone(clock), MAX_FAILURES_PER_CLIENT),
            account_throttle: LoginThrottle::new(Arc::clone(clock), MAX_FAILURES_PER_ACCOUNT),
            rate_limit: RateLimit::new(Arc::clone(clock)),
            sessions: SessionStore::new(Arc::clone(clock)),
            verification_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_VERIFICATIONS)),
        })
    }

    /// Forgets expired failures every [`SWEEP_INTERVAL`]. Runs forever.
    pub(crate) async fn sweep_periodically(self: Arc<Self>) {
        let mut interval = time::interval(SWEEP_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            self.sweep();
        }
    }

    /// Forgets the failures of clients and accounts that left the throttle
    /// window.
    fn sweep(&self) {
        self.client_throttle.sweep();
        self.account_throttle.sweep();
    }

    /// Returns the active sessions.
    pub(crate) const fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    /// Checks the credentials and creates a session.
    ///
    /// `client` is the address of the client, if the proxy reports it. Only
    /// the names of existing accounts are logged, as people sometimes type
    /// their password into the username field. Client addresses aren't logged.
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with the reason. See there what can be revealed
    /// to the client.
    pub(crate) async fn login(
        &self,
        username: &str,
        password: &str,
        client: Option<IpAddr>,
    ) -> Result<SecretToken, LoginError> {
        let known_account = self.credentials.find(username);
        let account = known_account.as_deref().unwrap_or(UNKNOWN_ACCOUNT);
        let account_key = account_key(known_account.as_deref(), username);
        let client_key = client.map(|address| self.client_pseudonyms.of(address));

        let slot = match self.admit(client_key.as_deref(), &account_key) {
            Ok(slot) => slot,
            Err(Refusal::Throttled(retry_after)) => {
                debug!(account, "login refused: client or account blocked");
                return Err(LoginError::Throttled { retry_after });
            }
            Err(Refusal::Busy) => return Err(LoginError::Busy),
        };

        // The slot moves into the task: if the request is cancelled, e.g., by
        // a client disconnect, the slot stays taken until the check ends.
        let credentials = Arc::clone(&self.credentials);
        let (username, password) = (username.to_string(), password.to_string());
        let verified = task::spawn_blocking(move || {
            let verified = credentials.verify(&username, &password).is_some();
            drop(slot);
            verified
        })
        .await
        .map_err(|_| LoginError::TaskFailed)?;

        if !verified {
            warn!(account, "login failed");
            self.record_failure(account, client_key.as_deref(), &account_key);
            return Err(LoginError::InvalidCredentials);
        }
        self.record_success(client_key.as_deref(), &account_key);
        info!(account, "login succeeded");
        Ok(self.sessions.create(account)?)
    }

    /// Checks the throttles and limits, and returns a password check slot.
    fn admit(
        &self,
        client_key: Option<&str>,
        account_key: &str,
    ) -> Result<OwnedSemaphorePermit, Refusal> {
        client_key
            .map_or(Ok(()), |key| self.client_throttle.check(key))
            .and_then(|()| self.account_throttle.check(account_key))
            .map_err(Refusal::Throttled)?;
        if let Decision::Refused { first } = self.rate_limit.try_acquire() {
            if first {
                warn!(
                    "more than {} password checks per minute, refusing logins for now",
                    rate_limit::MAX_CHECKS
                );
            }
            return Err(Refusal::Busy);
        }
        Arc::clone(&self.verification_slots)
            .try_acquire_owned()
            .map_err(|_| {
                debug!("login refused: too many concurrent password checks");
                Refusal::Busy
            })
    }

    /// Counts a failed password check for the client and the account.
    fn record_failure(&self, account: &str, client_key: Option<&str>, account_key: &str) {
        if let Some(key) = client_key
            && self.client_throttle.record_failure(key)
        {
            warn!(
                account,
                "blocking logins of a client after repeated failures"
            );
        }
        if self.account_throttle.record_failure(account_key) {
            warn!(
                account,
                "blocking logins of the account after repeated failures"
            );
        }
    }

    /// Forgets the failures of the client and the account.
    fn record_success(&self, client_key: Option<&str>, account_key: &str) {
        if let Some(key) = client_key {
            self.client_throttle.record_success(key);
        }
        self.account_throttle.record_success(account_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{MockClock, TEST_PASSWORD, TEST_USER, accounts_file};
    use std::assert_matches;
    use std::time::Duration;
    use tokio::time::timeout;

    /// Creates an authenticator with the test account and a controllable clock.
    fn authenticator() -> (Arc<MockClock>, Authenticator) {
        let mock = Arc::new(MockClock::default());
        let clock: Arc<dyn Clock> = Arc::<MockClock>::clone(&mock);
        let credentials = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("fixture should be valid");
        let auth =
            Authenticator::new(Arc::new(credentials), &clock).expect("OS RNG should work in tests");
        (mock, auth)
    }

    /// Returns the address of test client `n`.
    fn client(n: u8) -> Option<IpAddr> {
        Some(IpAddr::from([192, 0, 2, n]))
    }

    /// Fails one login of `username` per entry of `clients`.
    async fn fail_logins(
        auth: &Authenticator,
        username: &str,
        clients: impl Iterator<Item = Option<IpAddr>>,
    ) {
        for client in clients {
            let _ = auth.login(username, "wrong", client).await;
        }
    }

    /// Valid credentials create a session for the account.
    #[tokio::test]
    async fn login_creates_session() {
        let (_, auth) = authenticator();

        let token = auth
            .login(TEST_USER, TEST_PASSWORD, None)
            .await
            .expect("login should succeed");

        let session = auth.sessions().authenticate(token.as_str());
        assert_eq!(session.expect("session should exist").username, TEST_USER);
    }

    /// Unknown users get the same error as wrong passwords.
    #[tokio::test]
    async fn login_does_not_reveal_existing_users() {
        let (_, auth) = authenticator();

        assert_matches!(
            auth.login(TEST_USER, "wrong", None).await,
            Err(LoginError::InvalidCredentials)
        );
        assert_matches!(
            auth.login("nobody", "wrong", None).await,
            Err(LoginError::InvalidCredentials)
        );
    }

    /// A guessing client is blocked, even with the right password, while the
    /// real user on another client can still log in. This is why the limit
    /// per client is lower than the one per account.
    #[tokio::test]
    async fn guessing_client_is_blocked_without_blocking_account() {
        let (clock, auth) = authenticator();
        fail_logins(
            &auth,
            TEST_USER,
            (0..MAX_FAILURES_PER_CLIENT).map(|_| client(1)),
        )
        .await;

        assert_matches!(
            auth.login(TEST_USER, TEST_PASSWORD, client(1)).await,
            Err(LoginError::Throttled { .. })
        );
        auth.login(TEST_USER, TEST_PASSWORD, client(2))
            .await
            .expect("the login should succeed");

        clock.advance(SignedDuration::from_mins(15));
        auth.login(TEST_USER, TEST_PASSWORD, client(1))
            .await
            .expect("the login should succeed");
    }

    /// Guessing spread over many clients still hits the account limit.
    #[tokio::test]
    async fn distributed_guessing_blocks_account() {
        let (_, auth) = authenticator();
        let clients = (0..MAX_FAILURES_PER_ACCOUNT).map(|n| client(u8::try_from(n).expect("fits")));
        fail_logins(&auth, TEST_USER, clients).await;

        assert_matches!(
            auth.login(TEST_USER, TEST_PASSWORD, client(200)).await,
            Err(LoginError::Throttled { .. })
        );
    }

    /// Without the proxy header, only the account limit applies.
    #[tokio::test]
    async fn without_client_address_the_account_limit_applies() {
        let (_, auth) = authenticator();
        fail_logins(
            &auth,
            TEST_USER,
            (1..MAX_FAILURES_PER_ACCOUNT).map(|_| None),
        )
        .await;

        auth.login(TEST_USER, TEST_PASSWORD, None)
            .await
            .expect("the login should succeed");
    }

    /// Unknown names are throttled like real accounts, each on its own. If
    /// they shared one counter, a blocked counter would answer differently
    /// for unknown and real names and reveal which accounts exist.
    #[tokio::test]
    async fn unknown_users_are_throttled_individually() {
        let (_, auth) = authenticator();
        fail_logins(&auth, "nobody", (0..MAX_FAILURES_PER_ACCOUNT).map(|_| None)).await;

        assert_matches!(
            auth.login("nobody", "wrong", None).await,
            Err(LoginError::Throttled { .. })
        );
        assert_matches!(
            auth.login("somebody", "wrong", None).await,
            Err(LoginError::InvalidCredentials)
        );
        auth.login(TEST_USER, TEST_PASSWORD, None)
            .await
            .expect("the login should succeed");
    }

    /// A cancelled login (client disconnect, timeout) keeps its slot until the
    /// password check really ends. Otherwise, cancelling requests in a loop
    /// would start unlimited parallel checks and exhaust the memory.
    #[tokio::test]
    async fn cancelled_login_keeps_slot_until_check_ends() {
        let (_, auth) = authenticator();

        // An unknown user is checked against the dummy hash with production
        // parameters, which takes several milliseconds.
        let cancelled = timeout(
            Duration::from_millis(1),
            auth.login("nobody", "wrong", None),
        )
        .await;

        cancelled.expect_err("the login should still run after 1 ms");
        assert_eq!(
            auth.verification_slots.available_permits(),
            MAX_CONCURRENT_VERIFICATIONS - 1
        );
        let all_slots = auth
            .verification_slots
            .acquire_many(MAX_CONCURRENT_VERIFICATIONS as u32);
        let _permits = timeout(Duration::from_secs(5), all_slots)
            .await
            .expect("the slot should return after the check")
            .expect("the semaphore should stay open");
    }

    /// The client throttle keys failures by pseudonym, not by address.
    #[tokio::test]
    async fn client_addresses_are_pseudonymized() {
        let (_, auth) = authenticator();

        fail_logins(&auth, TEST_USER, [client(7)].into_iter()).await;

        let keys = auth.client_throttle.keys();
        assert_eq!(keys.len(), 1);
        assert!(keys.iter().all(|key| !key.contains("192.0.2.7")));
    }

    /// Once the throttle window has passed, the sweep removes all traces of
    /// failed logins, so pseudonyms don't stay until the next failure.
    #[tokio::test]
    async fn sweep_forgets_expired_failures() {
        let (clock, auth) = authenticator();
        fail_logins(&auth, "nobody", [client(7)].into_iter()).await;

        clock.advance(SignedDuration::from_mins(15));
        auth.sweep();

        assert!(auth.client_throttle.keys().is_empty());
        assert!(auth.account_throttle.keys().is_empty());
    }

    /// Unknown names get stable keys that can't collide with account names.
    #[test]
    fn account_key_separates_known_and_unknown_names() {
        assert_eq!(account_key(Some("dirk"), "dirk"), "dirk");
        assert_eq!(account_key(None, "nobody"), account_key(None, "nobody"));
        assert_ne!(account_key(None, "nobody"), account_key(None, "somebody"));
        assert!(account_key(None, "dirk").starts_with(UNKNOWN_KEY_PREFIX));
        assert!(!account_key(None, "secret password").contains("secret"));
    }

    /// A flood of logins hits the global limit, and real accounts are refused
    /// as well until it passes.
    #[tokio::test]
    async fn login_is_refused_when_rate_limit_is_exhausted() {
        let (clock, auth) = authenticator();
        for _ in 0..rate_limit::MAX_CHECKS {
            auth.rate_limit.try_acquire();
        }

        assert_matches!(
            auth.login(TEST_USER, TEST_PASSWORD, None).await,
            Err(LoginError::Busy)
        );

        clock.advance(SignedDuration::from_mins(1));
        auth.login(TEST_USER, TEST_PASSWORD, None)
            .await
            .expect("the login should succeed");
    }

    /// Password checks beyond the limit are refused instead of queued, so a
    /// flood of logins can't exhaust the memory.
    #[tokio::test]
    async fn login_is_refused_when_all_slots_are_busy() {
        let (_, auth) = authenticator();
        let _slots = auth
            .verification_slots
            .acquire_many(MAX_CONCURRENT_VERIFICATIONS as u32)
            .await
            .expect("semaphore should be open");

        assert_matches!(
            auth.login(TEST_USER, TEST_PASSWORD, None).await,
            Err(LoginError::Busy)
        );
    }
}
