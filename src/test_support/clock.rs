// SPDX-License-Identifier: MIT OR Apache-2.0

//! A clock that tests control.

use crate::clock::Clock;
use jiff::{SignedDuration, Timestamp};
use std::sync::Mutex;

/// The start time of every [`MockClock`].
const START: &str = "2026-11-07T10:00:00Z";

/// A clock that only moves when the test calls [`MockClock::advance`].
#[derive(Debug)]
pub(crate) struct MockClock(Mutex<Timestamp>);

impl MockClock {
    /// Moves the clock forward by `duration`.
    pub(crate) fn advance(&self, duration: SignedDuration) {
        let mut now = self.0.lock().unwrap();
        *now += duration;
    }
}

impl Default for MockClock {
    /// Starts at a fixed time, so that tests are deterministic.
    fn default() -> Self {
        Self(Mutex::new(
            START.parse().expect("the start time should parse"),
        ))
    }
}

impl Clock for MockClock {
    /// Returns the current mock time.
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}
