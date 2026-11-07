// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders the login area. Unlike public pages, these pages are rendered per
//! request, as they depend on the session.

use crate::clock::local_time_zone;
use crate::content::news::NewsDocument;
use crate::render::layout::Layout;
use crate::render::templates::{
    EditorTemplate, LoginTemplate, Notice, StatsDayRow, StatsPageRow, StatsTemplate,
};
use crate::render::{RenderError, SiteRenderer};
use crate::security::credentials::MAX_USERNAME_BYTES;
use crate::security::password::MAX_PASSWORD_BYTES;
use crate::security::sanitize::SanitizedHtml;
use crate::stats::{DayStats, tracked_pages};
use askama::Template;
use jiff::Timestamp;
use jiff::civil::{Date, Weekday};

/// What the editor shows.
#[derive(Debug)]
pub(crate) struct EditorView<'a> {
    /// The logged-in account.
    pub(crate) username: &'a str,
    /// The content in the editor. Differs from the stored content after a
    /// failed save.
    pub(crate) html: &'a SanitizedHtml,
    /// The stored document.
    pub(crate) stored: &'a NewsDocument,
    /// The CSRF token of the session.
    pub(crate) csrf: &'a str,
    /// The message to show, if any.
    pub(crate) notice: Option<Notice>,
}

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

    /// Renders the editor.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the template fails.
    pub(crate) fn render_editor(&self, view: &EditorView<'_>) -> Result<String, RenderError> {
        let mut layout = self.private_layout("Aktuelle Informationen bearbeiten", Vec::new());
        if view.notice.as_ref().is_some_and(Notice::is_error) {
            mark_error(&mut layout);
        }
        Ok(EditorTemplate {
            layout: &layout,
            username: view.username,
            html: view.html,
            revision: view.stored.revision,
            modified_at: &format_local_time(view.stored.modified_at),
            modified_by: &view.stored.modified_by,
            csrf: view.csrf,
            notice: view.notice.as_ref(),
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

/// Formats a time in the local time zone, e.g., `07.11.2026 um 09:31 Uhr`.
fn format_local_time(time: Timestamp) -> String {
    time.to_zoned(local_time_zone())
        .strftime("%d.%m.%Y um %H:%M Uhr")
        .to_string()
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

    /// Times are shown in German local time, including daylight saving time.
    #[test]
    fn format_local_time_uses_german_time() {
        let winter: Timestamp = "2026-11-07T08:31:14Z".parse().expect("should parse");
        let summer: Timestamp = "2026-07-01T08:00:00Z".parse().expect("should parse");

        assert_eq!(format_local_time(winter), "07.11.2026 um 09:31 Uhr");
        assert_eq!(format_local_time(summer), "01.07.2026 um 10:00 Uhr");
    }

    /// Dates in the statistics have a German weekday.
    #[test]
    fn format_german_date_has_weekday() {
        let date: Date = "2026-11-07".parse().expect("should parse");

        assert_eq!(format_german_date(date), "Sa, 07.11.2026");
    }
}
