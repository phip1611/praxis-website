// SPDX-License-Identifier: MIT OR Apache-2.0

//! Blocks keys, such as clients or accounts, after repeated failed logins.

use crate::clock::Clock;
use jiff::{SignedDuration, Timestamp};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// How long a failed login counts.
pub(crate) const FAILURE_WINDOW: SignedDuration = SignedDuration::from_mins(15);

/// The maximum number of keys with recent failures. Bounds the memory use
/// when someone tries many different usernames. When full, the key with the
/// fewest failures is dropped, and among those the one with the oldest last
/// failure. So many new keys can't erase the failures of a blocked key.
const MAX_KEYS: usize = 1024;

/// Blocks keys with too many failed logins within [`FAILURE_WINDOW`].
///
/// At most [`MAX_KEYS`] keys are tracked, so arbitrary keys are fine.
#[derive(Debug)]
pub(super) struct LoginThrottle {
    /// The source of the current time.
    clock: Arc<dyn Clock>,
    /// The failures within the window that block a key.
    max_failures: usize,
    /// The times of recent failures per key, oldest first.
    failures: Mutex<HashMap<String, VecDeque<Timestamp>>>,
}

impl LoginThrottle {
    /// Creates a throttle that blocks a key after `max_failures` failures.
    pub(super) fn new(clock: Arc<dyn Clock>, max_failures: usize) -> Self {
        Self {
            clock,
            max_failures,
            failures: Mutex::new(HashMap::new()),
        }
    }

    /// Checks if a login attempt for `key` may proceed.
    ///
    /// # Errors
    ///
    /// Returns how long the caller has to wait if the key is blocked.
    pub(super) fn check(&self, key: &str) -> Result<(), SignedDuration> {
        let now = self.clock.now();
        let oldest_counted = {
            let mut failures = self.failures.lock().unwrap();
            failures.get_mut(key).and_then(|entries| {
                Self::prune(entries, now);
                (entries.len() >= self.max_failures)
                    .then(|| entries.front().copied())
                    .flatten()
            })
        };
        // The account unblocks once the oldest counted failure expires.
        oldest_counted.map_or(Ok(()), |oldest| {
            Err(now.duration_until(oldest + FAILURE_WINDOW))
        })
    }

    /// Records a failed login for `key`, and returns whether this failure
    /// blocked the key.
    pub(super) fn record_failure(&self, key: &str) -> bool {
        let now = self.clock.now();
        let mut failures = self.failures.lock().unwrap();
        if failures.len() >= MAX_KEYS && !failures.contains_key(key) {
            Self::forget_expired_keys(&mut failures, now);
            if failures.len() >= MAX_KEYS {
                let stalest = failures
                    .iter()
                    .min_by_key(|(_, entries)| (entries.len(), entries.back().copied()))
                    .map(|(key, _)| key.clone());
                if let Some(stalest) = stalest {
                    failures.remove(&stalest);
                }
            }
        }
        let entries = failures.entry(key.to_string()).or_default();
        Self::prune(entries, now);
        entries.push_back(now);
        let blocked = entries.len() == self.max_failures;
        drop(failures);
        blocked
    }

    /// Clears the failures of `key` after a successful login.
    pub(super) fn record_success(&self, key: &str) {
        self.failures.lock().unwrap().remove(key);
    }

    /// Forgets the keys whose failures are all older than [`FAILURE_WINDOW`].
    ///
    /// Otherwise, a key would stay until it fails again or the table is full.
    /// Call it periodically, so that client pseudonyms don't stay in memory
    /// longer than needed.
    pub(super) fn sweep(&self) {
        let now = self.clock.now();
        Self::forget_expired_keys(&mut self.failures.lock().unwrap(), now);
    }

    /// Returns the keys with recorded failures, for tests in other modules.
    #[cfg(test)]
    pub(super) fn keys(&self) -> Vec<String> {
        self.failures.lock().unwrap().keys().cloned().collect()
    }

    /// Removes the keys without failures within [`FAILURE_WINDOW`].
    fn forget_expired_keys(failures: &mut HashMap<String, VecDeque<Timestamp>>, now: Timestamp) {
        failures.retain(|_, entries| {
            Self::prune(entries, now);
            !entries.is_empty()
        });
    }

    /// Removes entries older than [`FAILURE_WINDOW`].
    fn prune(entries: &mut VecDeque<Timestamp>, now: Timestamp) {
        let oldest_allowed = now - FAILURE_WINDOW;
        while entries
            .pop_front_if(|time| *time <= oldest_allowed)
            .is_some()
        {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockClock;

    /// The limit of the throttles in these tests.
    const MAX_FAILURES: usize = 5;

    /// Creates a throttle with `failures` failures already recorded for "dirk".
    fn throttle_with_failures(failures: usize) -> (Arc<MockClock>, LoginThrottle) {
        let clock = Arc::new(MockClock::default());
        let throttle = LoginThrottle::new(Arc::<MockClock>::clone(&clock), MAX_FAILURES);
        for _ in 0..failures {
            throttle.record_failure("dirk");
        }
        (clock, throttle)
    }

    /// Attempts stay allowed right below the limit.
    #[test]
    fn allows_attempts_below_limit() {
        let (_, throttle) = throttle_with_failures(MAX_FAILURES - 1);

        throttle
            .check("dirk")
            .expect("the key should not be blocked");
    }

    /// Reaching the limit blocks and reports the full remaining window.
    #[test]
    fn blocks_at_limit_and_reports_wait_time() {
        let (_, throttle) = throttle_with_failures(MAX_FAILURES);

        let wait = throttle
            .check("dirk")
            .expect_err("the key should be blocked");
        assert_eq!(wait, FAILURE_WINDOW);
    }

    /// The reported wait time shrinks as time passes.
    #[test]
    fn wait_time_shrinks_with_time() {
        let (clock, throttle) = throttle_with_failures(MAX_FAILURES);

        clock.advance(SignedDuration::from_mins(10));

        let wait = throttle
            .check("dirk")
            .expect_err("the key should be blocked");
        assert_eq!(wait, SignedDuration::from_mins(5));
    }

    /// The block ends by itself once the failures are old enough.
    #[test]
    fn unblocks_after_window() {
        let (clock, throttle) = throttle_with_failures(MAX_FAILURES);

        clock.advance(FAILURE_WINDOW);

        throttle
            .check("dirk")
            .expect("the key should not be blocked");
    }

    /// One blocked account doesn't affect other accounts.
    #[test]
    fn accounts_are_independent() {
        let (_, throttle) = throttle_with_failures(MAX_FAILURES);

        throttle
            .check("webmaster")
            .expect("the key should not be blocked");
    }

    /// Many different keys can't grow the table beyond its limit, and the key
    /// with the oldest failure goes first.
    #[test]
    fn table_size_is_bounded() {
        let (clock, throttle) = throttle_with_failures(1);
        clock.advance(SignedDuration::from_secs(1));

        for index in 0..MAX_KEYS {
            throttle.record_failure(&format!("key{index}"));
        }

        assert_eq!(throttle.failures.lock().unwrap().len(), MAX_KEYS);
        assert!(!throttle.failures.lock().unwrap().contains_key("dirk"));
    }

    /// Filling the table with new keys doesn't unblock a blocked key, even
    /// if its failures are the oldest.
    #[test]
    fn full_table_keeps_blocked_key() {
        let (clock, throttle) = throttle_with_failures(MAX_FAILURES);
        clock.advance(SignedDuration::from_secs(1));

        for index in 0..=MAX_KEYS {
            throttle.record_failure(&format!("key{index}"));
        }

        throttle
            .check("dirk")
            .expect_err("the key should stay blocked");
    }

    /// The sweep forgets keys once their last failure leaves the window and
    /// keeps keys with recent failures.
    #[test]
    fn sweep_forgets_expired_keys() {
        let (clock, throttle) = throttle_with_failures(1);
        clock.advance(SignedDuration::from_mins(10));
        throttle.record_failure("webmaster");
        clock.advance(SignedDuration::from_mins(5));

        throttle.sweep();

        assert_eq!(throttle.keys(), ["webmaster"]);
    }

    /// Only the failure that blocks reports it, so it's logged once.
    #[test]
    fn record_failure_reports_block_once() {
        let (_, throttle) = throttle_with_failures(MAX_FAILURES - 1);

        assert!(throttle.record_failure("dirk"));
        assert!(!throttle.record_failure("dirk"));
    }

    /// A successful login forgets earlier typos.
    #[test]
    fn success_clears_failures() {
        let (_, throttle) = throttle_with_failures(MAX_FAILURES - 1);

        throttle.record_success("dirk");
        throttle.record_failure("dirk");

        throttle
            .check("dirk")
            .expect("the key should not be blocked");
    }
}
