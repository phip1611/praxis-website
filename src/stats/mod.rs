// SPDX-License-Identifier: MIT OR Apache-2.0

//! Anonymous access statistics: page views per day and page.
//!
//! Only counters are stored, no IP addresses, user agents, or other personal
//! data. The user agent is only inspected to tell crawlers apart, see
//! [`bots`]. Only known pages get counters, so the memory use is bounded no
//! matter which paths clients request.
//!
//! The counters live in memory and are written to the state directory every
//! few minutes and on shutdown, see [`report`].

pub(crate) mod bots;
#[expect(dead_code, reason = "used by code that a later commit adds")]
pub(crate) mod clients;
pub(crate) mod report;

use crate::clock::Clock;
use crate::content::pages::HOME_PATH;
use crate::content::site::SITE;
use crate::persist::{self, StoreError};
use jiff::ToSpan;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::warn;

/// The name of the statistics file in the state directory.
const FILE_NAME: &str = "statistik.json";

/// The extension added to a statistics file that can't be read.
const UNREADABLE_EXTENSION: &str = "unreadable";

/// How many days are kept.
const RETENTION_DAYS: i32 = 366;

/// One request to count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Hit<'a> {
    /// A view of a known page.
    Page {
        /// The path of the page.
        path: &'a str,
        /// Whether a crawler requested it.
        bot: bool,
    },
    /// A request to an unknown path.
    NotFound,
}

/// The counters of one day.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub(crate) struct DayStats {
    /// Views by people, per page path.
    pub(crate) views: BTreeMap<String, u64>,
    /// Views by crawlers, of all pages.
    pub(crate) bot_views: u64,
    /// Requests to unknown paths.
    pub(crate) not_found: u64,
}

impl DayStats {
    /// Returns the views by people of all pages.
    pub(crate) fn total_views(&self) -> u64 {
        self.views.values().sum()
    }
}

/// Returns the counted pages as `(path, label)`, in navigation order.
pub(crate) fn tracked_pages() -> impl Iterator<Item = (&'static str, &'static str)> {
    iter::once((HOME_PATH, SITE.home.label.as_str())).chain(
        SITE.pages
            .iter()
            .map(|page| (page.path.as_str(), page.heading.as_str())),
    )
}

/// Returns whether views of `path` are counted.
pub(crate) fn is_tracked(path: &str) -> bool {
    tracked_pages().any(|(tracked, _)| tracked == path)
}

/// Moves a statistics file that can't be read aside, so that the next write
/// doesn't destroy it. Its data may be intact, e.g., after a permission error.
fn keep_unreadable_file(file: &Path) {
    if let Err(err) = fs::rename(file, file.with_added_extension(UNREADABLE_EXTENSION)) {
        warn!("can't move {} aside: {err}", file.display());
    }
}

/// The statistics data in memory.
#[derive(Debug, Default)]
struct Counters {
    /// The counters per local date.
    days: BTreeMap<Date, DayStats>,
    /// Whether there are changes that aren't written to disk yet.
    dirty: bool,
}

/// The access statistics of the service.
#[derive(Debug)]
pub(crate) struct AccessStats {
    /// The source of the current time.
    clock: Arc<dyn Clock>,
    /// The statistics file.
    file: PathBuf,
    /// The counters.
    counters: Mutex<Counters>,
    /// Serializes writes, as the periodic and the final flush at shutdown may
    /// overlap and would share the temporary file.
    flush_lock: Mutex<()>,
}

impl AccessStats {
    /// Loads the statistics from the state directory.
    ///
    /// Statistics aren't essential, so a damaged file is logged and the
    /// counting starts over instead of stopping the service.
    pub(crate) fn load(clock: Arc<dyn Clock>, state_dir: &Path) -> Self {
        let file = state_dir.join(FILE_NAME);
        let days = persist::read_json(&file)
            .unwrap_or_else(|err| {
                warn!("starting with empty access statistics: {err}");
                keep_unreadable_file(&file);
                None
            })
            .unwrap_or_default();
        Self {
            clock,
            file,
            counters: Mutex::new(Counters { days, dirty: false }),
            flush_lock: Mutex::new(()),
        }
    }

    /// Returns the current local date.
    pub(crate) fn today(&self) -> Date {
        self.clock.today()
    }

    /// Counts a request.
    pub(crate) fn record(&self, hit: Hit<'_>) {
        let today = self.today();
        let mut counters = self.counters.lock().unwrap();
        if !counters.days.contains_key(&today) {
            let oldest_kept = today - RETENTION_DAYS.days();
            counters.days.retain(|date, _| *date > oldest_kept);
        }
        let day = counters.days.entry(today).or_default();
        match hit {
            Hit::Page { bot: true, .. } => day.bot_views += 1,
            Hit::Page { path, bot: false } => {
                *day.views.entry(path.to_string()).or_default() += 1;
            }
            Hit::NotFound => day.not_found += 1,
        }
        counters.dirty = true;
    }

    /// Returns the counters of `date`.
    pub(crate) fn day(&self, date: Date) -> DayStats {
        self.counters
            .lock()
            .unwrap()
            .days
            .get(&date)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the counters of the last `days` days including today, newest
    /// first. Days without requests are included with zero counters.
    pub(crate) fn recent(&self, days: i32) -> Vec<(Date, DayStats)> {
        let today = self.today();
        let counters = self.counters.lock().unwrap();
        (0..days)
            .map(|offset| {
                let date = today - offset.days();
                (date, counters.days.get(&date).cloned().unwrap_or_default())
            })
            .collect()
    }

    /// Writes the statistics to disk if they changed since the last write.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if writing fails. The changes are written with
    /// the next call then.
    pub(crate) fn flush(&self) -> Result<(), StoreError> {
        let _writing = self.flush_lock.lock().unwrap();
        let days = {
            let mut counters = self.counters.lock().unwrap();
            if !counters.dirty {
                return Ok(());
            }
            counters.dirty = false;
            counters.days.clone()
        };
        persist::write_json(&self.file, &days).inspect_err(|_| {
            self.counters.lock().unwrap().dirty = true;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockClock;
    use jiff::SignedDuration;
    use std::fs;
    use tempfile::TempDir;

    /// Creates empty statistics in a temporary state directory.
    fn stats() -> (TempDir, Arc<MockClock>, AccessStats) {
        let dir = TempDir::new().expect("temp dir should be created");
        let clock = Arc::new(MockClock::default());
        let stats = AccessStats::load(Arc::<MockClock>::clone(&clock), dir.path());
        (dir, clock, stats)
    }

    /// A view by a person.
    const fn view(path: &str) -> Hit<'_> {
        Hit::Page { path, bot: false }
    }

    /// Views are counted per page, crawlers and errors separately.
    #[test]
    fn record_counts_per_page_and_kind() {
        let (_dir, _clock, stats) = stats();

        stats.record(view("/"));
        stats.record(view("/"));
        stats.record(view("/kontakt/"));
        stats.record(Hit::Page {
            path: "/",
            bot: true,
        });
        stats.record(Hit::NotFound);

        let today = stats.day(stats.today());
        assert_eq!(today.views["/"], 2);
        assert_eq!(today.views["/kontakt/"], 1);
        assert_eq!(today.total_views(), 3);
        assert_eq!(today.bot_views, 1);
        assert_eq!(today.not_found, 1);
    }

    /// Each local day has its own counters.
    #[test]
    fn days_are_counted_separately() {
        let (_dir, clock, stats) = stats();
        stats.record(view("/"));
        let yesterday = stats.today();

        clock.advance(SignedDuration::from_hours(24));
        stats.record(view("/"));
        stats.record(view("/"));

        assert_eq!(stats.day(yesterday).total_views(), 1);
        assert_eq!(stats.day(stats.today()).total_views(), 2);
    }

    /// `recent` fills days without requests, newest first.
    #[test]
    fn recent_includes_empty_days() {
        let (_dir, clock, stats) = stats();
        stats.record(view("/"));
        clock.advance(SignedDuration::from_hours(48));

        let recent = stats.recent(3);

        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].0, stats.today());
        assert_eq!(recent[1].1.total_views(), 0);
        assert_eq!(recent[2].1.total_views(), 1);
    }

    /// Old days are dropped, so the file doesn't grow forever.
    #[test]
    fn old_days_are_dropped() {
        let (_dir, clock, stats) = stats();
        stats.record(view("/"));
        let first_day = stats.today();

        clock.advance(SignedDuration::from_hours(24 * i64::from(RETENTION_DAYS)));
        stats.record(view("/"));

        assert_eq!(stats.day(first_day).total_views(), 0);
        assert_eq!(stats.counters.lock().unwrap().days.len(), 1);
    }

    /// The counters survive a restart.
    #[test]
    fn flush_persists_counters() {
        let (dir, clock, stats) = stats();
        stats.record(view("/"));

        stats.flush().expect("flush should succeed");

        let reloaded = AccessStats::load(clock, dir.path());
        assert_eq!(reloaded.day(reloaded.today()).views["/"], 1);
    }

    /// Without changes, nothing is written.
    #[test]
    fn flush_skips_unchanged_data() {
        let (dir, _clock, stats) = stats();

        stats.flush().expect("flush should succeed");

        assert!(!dir.path().join(FILE_NAME).exists());
    }

    /// A failed write is retried with the next flush.
    #[test]
    fn failed_flush_keeps_changes_pending() {
        let clock = Arc::new(MockClock::default());
        let stats = AccessStats::load(clock, Path::new("/does/not/exist"));
        stats.record(view("/"));

        stats
            .flush()
            .expect_err("the flush should fail, as the directory does not exist");
        assert!(stats.counters.lock().unwrap().dirty);
    }

    /// A damaged file doesn't prevent the start.
    #[test]
    fn load_ignores_corrupt_file() {
        let dir = TempDir::new().expect("temp dir should be created");
        fs::write(dir.path().join(FILE_NAME), "{ broken").expect("write should succeed");

        let stats = AccessStats::load(Arc::new(MockClock::default()), dir.path());

        assert_eq!(stats.day(stats.today()).total_views(), 0);
        // The data stays available, also after the next write.
        stats.record(view("/"));
        stats.flush().expect("flush should succeed");
        let kept = dir
            .path()
            .join(FILE_NAME)
            .with_added_extension(UNREADABLE_EXTENSION);
        assert_eq!(
            fs::read_to_string(kept).expect("the file should be kept"),
            "{ broken"
        );
    }

    /// The start page and all fixed pages are counted, nothing else.
    #[test]
    fn only_known_pages_are_tracked() {
        assert!(is_tracked("/"));
        assert!(is_tracked("/kontakt/"));
        assert!(!is_tracked("/robots.txt"));
        assert!(!is_tracked("/assets/style.css"));
        assert_eq!(tracked_pages().count(), SITE.pages.len() + 1);
    }
}
