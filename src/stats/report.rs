// SPDX-License-Identifier: MIT OR Apache-2.0

//! The periodic background work of the statistics: writing them to disk and
//! logging a summary of each finished day.

use crate::stats::{AccessStats, DayStats};
use jiff::civil::Date;
use std::mem;
use std::sync::Arc;
use std::time::Duration;
use tokio::{task, time};
use tracing::{info, warn};

/// How often the statistics are written to disk. A crash loses at most this
/// much counting.
const FLUSH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Notices when a day is over.
#[derive(Clone, Copy, Debug)]
struct DayChange {
    /// The day that is currently counted.
    current: Date,
}

impl DayChange {
    /// Starts watching on `today`.
    const fn new(today: Date) -> Self {
        Self { current: today }
    }

    /// Returns the finished day if `today` is a new day.
    fn check(&mut self, today: Date) -> Option<Date> {
        (today != self.current).then(|| mem::replace(&mut self.current, today))
    }
}

/// Returns the log line that summarizes `day`.
fn summary(date: Date, day: &DayStats) -> String {
    let pages = day
        .views
        .iter()
        .map(|(path, views)| format!("{path} {views}"))
        .collect::<Vec<_>>()
        .join(", ");
    let pages = if pages.is_empty() {
        String::new()
    } else {
        format!(" ({pages})")
    };
    format!(
        "access statistics for {date}: {} page views{pages}, {} by bots, {} not found",
        day.total_views(),
        day.bot_views,
        day.not_found,
    )
}

/// Writes the statistics periodically and logs a summary after each day.
///
/// Runs until the runtime shuts down.
pub(crate) async fn run(stats: Arc<AccessStats>) {
    let mut interval = time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    let mut day_change = DayChange::new(stats.today());
    loop {
        interval.tick().await;
        if let Some(finished) = day_change.check(stats.today()) {
            info!("{}", summary(finished, &stats.day(finished)));
        }
        // Writing syncs to disk, which blocks.
        let flushing = Arc::clone(&stats);
        match task::spawn_blocking(move || flushing.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => warn!("can't write the access statistics: {err}"),
            Err(err) => warn!("the task writing the access statistics failed: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Returns a date from `YYYY-MM-DD`.
    fn date(text: &str) -> Date {
        text.parse().expect("date should parse")
    }

    /// The day change is reported once, with the finished day.
    #[test]
    fn day_change_reports_finished_day_once() {
        let mut day_change = DayChange::new(date("2026-11-07"));

        assert_eq!(day_change.check(date("2026-11-07")), None);
        assert_eq!(
            day_change.check(date("2026-11-08")),
            Some(date("2026-11-07"))
        );
        assert_eq!(day_change.check(date("2026-11-08")), None);
    }

    /// The summary names totals and the views per page.
    #[test]
    fn summary_lists_totals_and_pages() {
        let day = DayStats {
            views: BTreeMap::from([("/".to_string(), 80), ("/kontakt/".to_string(), 20)]),
            bot_views: 45,
            not_found: 3,
            ..DayStats::default()
        };

        assert_eq!(
            summary(date("2026-11-07"), &day),
            "access statistics for 2026-11-07: 100 page views (/ 80, /kontakt/ 20), \
                    45 by bots, 3 not found"
        );
    }

    /// Quiet days get a short line without empty parentheses.
    #[test]
    fn summary_of_empty_day() {
        assert_eq!(
            summary(date("2026-11-07"), &DayStats::default()),
            "access statistics for 2026-11-07: 0 page views, 0 by bots, 0 not found"
        );
    }
}
