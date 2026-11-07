// SPDX-License-Identifier: MIT OR Apache-2.0

//! The source of the current time and the local time zone.
//!
//! Code that depends on time takes a [`Clock`], so that tests can control the
//! time instead of sleeping.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::fmt::Debug;

/// Provides the current time.
pub(crate) trait Clock: Debug + Send + Sync {
    /// Returns the current time.
    fn now(&self) -> Timestamp;

    /// Returns the current date in the local time zone.
    fn today(&self) -> Date {
        local_date(self.now())
    }
}

/// The clock of the operating system.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    /// Returns the current system time.
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// Returns the time zone of the system that runs the service. Days in the statistics and times in the
/// login area use it.
#[cfg(not(test))]
pub(crate) fn local_time_zone() -> TimeZone {
    TimeZone::system()
}

/// Returns the rules of Germany, so that tests don't depend on the machine.
#[cfg(test)]
pub(crate) fn local_time_zone() -> TimeZone {
    TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").expect("the time zone should be valid")
}

/// Returns the date of `time` in the local time zone.
fn local_date(time: Timestamp) -> Date {
    time.to_zoned(local_time_zone()).date()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shortly after midnight in Germany it's still the previous day in UTC.
    #[test]
    fn local_date_uses_german_time() {
        let time: Timestamp = "2026-11-06T23:30:00Z".parse().expect("should parse");

        assert_eq!(local_date(time).to_string(), "2026-11-07");
    }
}
