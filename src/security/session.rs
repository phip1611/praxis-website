// SPDX-License-Identifier: MIT OR Apache-2.0

//! Server-side sessions with idle and absolute timeouts.
//!
//! Sessions live in memory only, so a restart logs everyone out. This is fine
//! for a handful of editors, and there is no session data to protect at rest.

use crate::clock::Clock;
use crate::security::csrf::CsrfToken;
use crate::security::token::{RandomError, SecretToken, digest};
use jiff::{SignedDuration, Timestamp};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A session ends after this time without any request.
const IDLE_TIMEOUT: SignedDuration = SignedDuration::from_hours(1);

/// A session ends after this time, even when active.
pub(super) const ABSOLUTE_TIMEOUT: SignedDuration = SignedDuration::from_hours(12);

/// The maximum number of concurrent sessions. Only authenticated users can
/// create sessions, so this just guards against leaks. The oldest is dropped.
const MAX_SESSIONS: usize = 64;

/// The data of a logged-in session.
#[derive(Clone, Debug)]
pub(crate) struct Session {
    /// The name of the logged-in account.
    pub(crate) username: String,
    /// The token that state-changing requests must contain.
    pub(crate) csrf: CsrfToken,
}

/// A session and its timing data.
#[derive(Debug)]
struct SessionRecord {
    /// The data handed to request handlers.
    session: Session,
    /// When the session was created.
    created_at: Timestamp,
    /// The time of the most recent request.
    last_seen: Timestamp,
}

impl SessionRecord {
    /// Checks if the session is past one of its timeouts.
    fn is_expired(&self, now: Timestamp) -> bool {
        now >= self.created_at + ABSOLUTE_TIMEOUT || now >= self.last_seen + IDLE_TIMEOUT
    }
}

/// All active sessions, addressed by the digest of their token.
#[derive(Debug)]
pub(crate) struct SessionStore {
    /// The source of the current time.
    clock: Arc<dyn Clock>,
    /// The sessions by [`digest`] of their token.
    sessions: Mutex<HashMap<[u8; 32], SessionRecord>>,
}

impl SessionStore {
    /// Creates an empty store.
    pub(super) fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a session for `username` and returns its new secret token.
    ///
    /// Every login gets a new token, which prevents session fixation.
    ///
    /// # Errors
    ///
    /// Returns [`RandomError`] if the random number generator fails.
    pub(super) fn create(&self, username: &str) -> Result<SecretToken, RandomError> {
        let token = SecretToken::generate()?;
        let session = Session {
            username: username.to_string(),
            csrf: CsrfToken::generate()?,
        };
        let now = self.clock.now();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|_, record| !record.is_expired(now));
        if sessions.len() >= MAX_SESSIONS {
            let oldest = sessions
                .iter()
                .min_by_key(|(_, record)| record.created_at)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(
            digest(token.as_str()),
            SessionRecord {
                session,
                created_at: now,
                last_seen: now,
            },
        );
        drop(sessions);
        Ok(token)
    }

    /// Returns the session of `token`, if it is valid, and extends its idle
    /// timeout.
    pub(crate) fn authenticate(&self, token: &str) -> Option<Session> {
        let key = digest(token);
        let now = self.clock.now();
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.get(&key)?.is_expired(now) {
            sessions.remove(&key);
            return None;
        }
        let record = sessions.get_mut(&key)?;
        record.last_seen = now;
        let session = record.session.clone();
        drop(sessions);
        Some(session)
    }

    /// Ends the session of `token`, if there is one.
    pub(crate) fn revoke(&self, token: &str) {
        self.sessions.lock().unwrap().remove(&digest(token));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockClock;

    /// Creates a store and a session for the account "dirk".
    fn store_with_session() -> (Arc<MockClock>, SessionStore, String) {
        let clock = Arc::new(MockClock::default());
        let store = SessionStore::new(Arc::<MockClock>::clone(&clock));
        let token = store.create("dirk").expect("OS RNG should work in tests");
        (clock, store, token.as_str().to_string())
    }

    /// A new session is valid and knows its account.
    #[test]
    fn authenticate_returns_new_session() {
        let (_, store, token) = store_with_session();

        let session = store.authenticate(&token).expect("session should be valid");

        assert_eq!(session.username, "dirk");
    }

    /// Made-up tokens don't authenticate.
    #[test]
    fn authenticate_rejects_unknown_token() {
        let (_, store, _) = store_with_session();

        assert!(store.authenticate("made-up").is_none());
    }

    /// Without requests, the session ends after the idle timeout.
    #[test]
    fn session_expires_after_idle_timeout() {
        let (clock, store, token) = store_with_session();

        clock.advance(IDLE_TIMEOUT);

        assert!(store.authenticate(&token).is_none());
    }

    /// Requests extend the idle timeout.
    #[test]
    fn activity_extends_idle_timeout() {
        let (clock, store, token) = store_with_session();

        clock.advance(IDLE_TIMEOUT - SignedDuration::from_secs(1));
        assert!(store.authenticate(&token).is_some());
        clock.advance(IDLE_TIMEOUT - SignedDuration::from_secs(1));

        assert!(store.authenticate(&token).is_some());
    }

    /// Even active sessions end after the absolute timeout.
    #[test]
    fn session_expires_after_absolute_timeout_despite_activity() {
        let (clock, store, token) = store_with_session();
        let step = IDLE_TIMEOUT - SignedDuration::from_secs(1);

        let mut elapsed = SignedDuration::ZERO;
        while elapsed + step < ABSOLUTE_TIMEOUT {
            clock.advance(step);
            elapsed += step;
            assert!(store.authenticate(&token).is_some());
        }
        clock.advance(step);

        assert!(store.authenticate(&token).is_none());
    }

    /// A revoked token stops working immediately.
    #[test]
    fn revoke_ends_session() {
        let (_, store, token) = store_with_session();

        store.revoke(&token);

        assert!(store.authenticate(&token).is_none());
    }

    /// Each session has its own CSRF token.
    #[test]
    fn sessions_have_distinct_csrf_tokens() {
        let (_, store, token) = store_with_session();
        let other = store.create("dirk").expect("OS RNG should work in tests");

        let first = store.authenticate(&token).expect("session should be valid");
        let second = store
            .authenticate(other.as_str())
            .expect("session should be valid");

        assert!(!first.csrf.verify(second.csrf.as_str()));
    }

    /// The table can't grow without bounds: the oldest session is dropped.
    #[test]
    fn oldest_session_is_dropped_at_capacity() {
        let (clock, store, oldest) = store_with_session();

        for _ in 0..MAX_SESSIONS {
            clock.advance(SignedDuration::from_secs(1));
            store.create("dirk").expect("OS RNG should work in tests");
        }

        assert!(store.authenticate(&oldest).is_none());
        assert_eq!(store.sessions.lock().unwrap().len(), MAX_SESSIONS);
    }

    /// The table stores digests, so tokens can't be read back from memory.
    #[test]
    fn table_does_not_store_raw_tokens() {
        let (_, store, token) = store_with_session();

        let table = format!("{:?}", store.sessions);

        assert!(!table.contains(&token));
    }
}
