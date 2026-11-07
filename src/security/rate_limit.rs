// SPDX-License-Identifier: MIT OR Apache-2.0

//! A global limit for password checks, as safety net for the CPU.
//!
//! The per-client and per-account throttles stop guessing, but a distributed
//! flood with ever new clients and usernames would still keep the CPU busy
//! with Argon2. This limit caps the password checks of all clients together.
//! It's high enough that only such a flood reaches it, which then blocks all
//! logins for a while; existing sessions keep working.

use crate::clock::Clock;
use jiff::{SignedDuration, Timestamp};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// The password checks per [`WINDOW`]. At about 20 ms per check, this caps the
/// CPU use at a fifth of one core.
pub(super) const MAX_CHECKS: usize = 600;

/// The sliding window of [`MAX_CHECKS`].
const WINDOW: SignedDuration = SignedDuration::from_mins(1);

/// The answer of [`RateLimit::try_acquire`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Decision {
    /// The check may run.
    Allowed,
    /// The limit is reached.
    Refused {
        /// Whether this is the first refusal in this window. Callers log only
        /// this one, so that a flood doesn't flood the log as well.
        first: bool,
    },
}

/// The mutable state of the limit.
#[derive(Debug, Default)]
struct State {
    /// The times of the checks in the current window, oldest first.
    checks: VecDeque<Timestamp>,
    /// Until when refusals count as already reported.
    reported_until: Option<Timestamp>,
}

/// Limits the password checks of all clients together.
#[derive(Debug)]
pub(super) struct RateLimit {
    /// The source of the current time.
    clock: Arc<dyn Clock>,
    /// The checks and the reporting state.
    state: Mutex<State>,
}

impl RateLimit {
    /// Creates a limit without any recorded checks.
    pub(super) fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            state: Mutex::new(State::default()),
        }
    }

    /// Records a check if the limit allows it.
    pub(super) fn try_acquire(&self) -> Decision {
        let now = self.clock.now();
        let mut state = self.state.lock().unwrap();
        let oldest_allowed = now - WINDOW;
        while state
            .checks
            .pop_front_if(|time| *time <= oldest_allowed)
            .is_some()
        {}
        if state.checks.len() < MAX_CHECKS {
            state.checks.push_back(now);
            return Decision::Allowed;
        }
        let first = state.reported_until.is_none_or(|until| now >= until);
        if first {
            state.reported_until = Some(now + WINDOW);
        }
        drop(state);
        Decision::Refused { first }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockClock;

    /// Creates a limit that is used up at the current mock time.
    fn exhausted() -> (Arc<MockClock>, RateLimit) {
        let clock = Arc::new(MockClock::default());
        let limit = RateLimit::new(Arc::<MockClock>::clone(&clock));
        for _ in 0..MAX_CHECKS {
            assert_eq!(limit.try_acquire(), Decision::Allowed);
        }
        (clock, limit)
    }

    /// Checks beyond the limit are refused, but only the first refusal is
    /// reported for logging.
    #[test]
    fn refuses_beyond_limit_and_reports_once() {
        let (_, limit) = exhausted();

        assert_eq!(limit.try_acquire(), Decision::Refused { first: true });
        assert_eq!(limit.try_acquire(), Decision::Refused { first: false });
    }

    /// The window slides: old checks expire and allow new ones.
    #[test]
    fn allows_again_after_window() {
        let (clock, limit) = exhausted();

        clock.advance(WINDOW);

        assert_eq!(limit.try_acquire(), Decision::Allowed);
    }

    /// A flood that lasts longer than a window is reported once per window.
    #[test]
    fn reports_again_in_next_window() {
        let (clock, limit) = exhausted();
        assert_eq!(limit.try_acquire(), Decision::Refused { first: true });

        clock.advance(WINDOW);
        for _ in 0..MAX_CHECKS {
            limit.try_acquire();
        }

        assert_eq!(limit.try_acquire(), Decision::Refused { first: true });
    }
}
