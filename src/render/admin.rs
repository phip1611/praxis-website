// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders the login area. Unlike public pages, these pages are rendered per
//! request, as they depend on the session.

use crate::render::layout::Layout;
use crate::render::templates::{LoginTemplate, StatsDayRow, StatsPageRow, StatsTemplate};
use crate::render::{RenderError, SiteRenderer};
use crate::security::credentials::MAX_USERNAME_BYTES;
use crate::security::password::MAX_PASSWORD_BYTES;
use crate::stats::{DayStats, tracked_pages};
use askama::Template;
use jiff::civil::{Date, Weekday};

impl SiteRenderer {
    /// Renders the login form.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the template fails.
    pub(crate) fn render_login(
        &self,
        username: &str,
        error: Option<&str>,
    ) -> Result<String, RenderError> {
        let mut layout = self.private_layout("Anmeldung", Vec::new());
        if error.is_some() {
            mark_error(&mut layout);
        }
        Ok(LoginTemplate {
            layout: &layout,
            username,
            error,
            max_username: MAX_USERNAME_BYTES,
            max_password: MAX_PASSWORD_BYTES,
        }
        .render()?)
    }

    /// Renders the access statistics of `days`, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the template fails.
    pub(crate) fn render_stats(
        &self,
        username: &str,
        csrf: &str,
        days: &[(Date, DayStats)],
    ) -> Result<String, RenderError> {
        let rows: Vec<_> = days
            .iter()
            .map(|(date, day)| StatsDayRow {
                date: format_german_date(*date),
                views: day.total_views(),
                bot_views: day.bot_views,
                not_found: day.not_found,
            })
            .collect();
        let pages: Vec<_> = tracked_pages()
            .map(|(path, label)| StatsPageRow {
                label,
                path,
                views: days.iter().filter_map(|(_, day)| day.views.get(path)).sum(),
            })
            .collect();
        let layout = self.private_layout("Zugriffsstatistik", Vec::new());
        Ok(StatsTemplate {
            layout: &layout,
            username,
            csrf,
            period_days: days.len(),
            total_views: rows.iter().map(|row| row.views).sum(),
            total_bot_views: rows.iter().map(|row| row.bot_views).sum(),
            // At least 1, as the bars divide by it.
            max_views: rows.iter().map(|row| row.views).max().unwrap_or(0).max(1),
            days: &rows,
            pages: &pages,
        }
        .render()?)
    }
}

/// Prefixes the title of a page that reports an error. Screen readers read
/// the title first, but don't reliably announce an alert that is already
/// part of the page when it loads.
fn mark_error(layout: &mut Layout) {
    layout.title = format!("Fehler: {}", layout.title);
}

/// Formats a date with German weekday, e.g., `Sa, 07.11.2026`.
fn format_german_date(date: Date) -> String {
    let weekday = match date.weekday() {
        Weekday::Monday => "Mo",
        Weekday::Tuesday => "Di",
        Weekday::Wednesday => "Mi",
        Weekday::Thursday => "Do",
        Weekday::Friday => "Fr",
        Weekday::Saturday => "Sa",
        Weekday::Sunday => "So",
    };
    format!("{weekday}, {}", date.strftime("%d.%m.%Y"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::assets::AssetCatalog;
    use crate::test_support::test_config;
    use std::sync::Arc;

    /// Creates a renderer for the test configuration.
    fn renderer() -> SiteRenderer {
        SiteRenderer::new(&test_config(), Arc::new(AssetCatalog::new()))
    }

    /// A failed login starts the title with "Fehler", a fresh form doesn't.
    #[test]
    fn login_error_is_in_title() {
        let failed = renderer()
            .render_login("dirk", Some("Falsches Passwort"))
            .expect("rendering should succeed");
        let fresh = renderer()
            .render_login("", None)
            .expect("rendering should succeed");

        assert!(failed.contains("<title>Fehler: Anmeldung"));
        assert!(fresh.contains("<title>Anmeldung"));
    }

    /// Dates in the statistics have a German weekday.
    #[test]
    fn format_german_date_has_weekday() {
        let date: Date = "2026-11-07".parse().expect("should parse");

        assert_eq!(format_german_date(date), "Sa, 07.11.2026");
    }
}
