// SPDX-License-Identifier: MIT OR Apache-2.0

//! The askama templates. The markup lives in the `templates` directory.

use crate::content::pages::{PageBody, PageSpec};
use crate::content::practice::{OpeningDay, Practice};
use crate::render::layout::Layout;
use crate::render::seo::sitemap::SitemapEntry;
use crate::security::sanitize::SanitizedHtml;
use askama::Template;

/// The start page with the news.
#[derive(Debug, Template)]
#[template(path = "home.html")]
pub(super) struct HomeTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
    /// The heading of the news.
    pub(super) heading: &'a str,
    /// The news.
    pub(super) news: &'a SanitizedHtml,
}

/// The images of the business card in all sizes.
#[derive(Debug)]
pub(super) struct BusinessCardImages {
    /// The full-size PNG, also the download.
    pub(super) full_png: String,
    /// The `srcset` of the PNG variants.
    pub(super) png_srcset: String,
    /// The `srcset` of the WebP variants.
    pub(super) webp_srcset: String,
    /// The width of the full size in pixels.
    pub(super) width: u32,
    /// The height of the full size in pixels.
    pub(super) height: u32,
    /// The alternative text.
    pub(super) alt: &'static str,
    /// The caption.
    pub(super) caption: &'static str,
}

/// A fixed page.
#[derive(Debug, Template)]
#[template(path = "page.html")]
pub(super) struct PageTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
    /// The page.
    pub(super) page: &'a PageSpec,
    /// The practice data.
    pub(super) practice: &'a Practice,
    /// The opening hours.
    pub(super) hours: &'a [OpeningDay],
    /// The business card, used by the contact page.
    pub(super) card: &'a BusinessCardImages,
}

/// The page for unknown paths.
#[derive(Debug, Template)]
#[template(path = "not_found.html")]
pub(super) struct NotFoundTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
}

/// The login form.
#[derive(Debug, Template)]
#[template(path = "admin/login.html")]
pub(super) struct LoginTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
    /// The username to prefill after a failed login.
    pub(super) username: &'a str,
    /// The error message, if any.
    pub(super) error: Option<&'a str>,
    /// The length limit of the username field.
    pub(super) max_username: usize,
    /// The length limit of the password field.
    pub(super) max_password: usize,
}

/// The kind of a notice in the login area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NoticeKind {
    /// Something worked.
    Success,
    /// Something failed.
    Error,
}

/// A message at the top of the editor.
#[derive(Clone, Debug)]
pub(crate) struct Notice {
    /// Whether it reports success or failure.
    pub(crate) kind: NoticeKind,
    /// The German text.
    pub(crate) text: String,
}

impl Notice {
    /// Returns whether it reports a failure.
    pub(crate) const fn is_error(&self) -> bool {
        matches!(self.kind, NoticeKind::Error)
    }

    /// Returns the CSS class for the kind.
    const fn class(&self) -> &'static str {
        match self.kind {
            NoticeKind::Success => "notice-success",
            NoticeKind::Error => "notice-error",
        }
    }

    /// Returns the ARIA role: errors interrupt screen readers, others don't.
    const fn role(&self) -> &'static str {
        match self.kind {
            NoticeKind::Success => "status",
            NoticeKind::Error => "alert",
        }
    }
}

/// The editor for the news.
#[derive(Debug, Template)]
#[template(path = "admin/editor.html")]
pub(super) struct EditorTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
    /// The logged-in account.
    pub(super) username: &'a str,
    /// The content to edit.
    pub(super) html: &'a SanitizedHtml,
    /// The revision the edit is based on.
    pub(super) revision: u64,
    /// When the stored content last changed, formatted for display.
    pub(super) modified_at: &'a str,
    /// Who changed the stored content last.
    pub(super) modified_by: &'a str,
    /// The CSRF token of the session.
    pub(super) csrf: &'a str,
    /// The message to show, if any.
    pub(super) notice: Option<&'a Notice>,
}

/// The XML sitemap.
#[derive(Debug, Template)]
#[template(path = "seo/sitemap.xml")]
pub(super) struct SitemapTemplate<'a> {
    /// The URLs.
    pub(super) entries: &'a [SitemapEntry],
}

/// One day in the access statistics.
#[derive(Debug)]
pub(super) struct StatsDayRow {
    /// The date, e.g., `Sa, 07.11.2026`.
    pub(super) date: String,
    /// The views by people.
    pub(super) views: u64,
    /// The views by crawlers.
    pub(super) bot_views: u64,
    /// The requests to unknown paths.
    pub(super) not_found: u64,
}

/// One page in the access statistics.
#[derive(Debug)]
pub(super) struct StatsPageRow {
    /// The name of the page.
    pub(super) label: &'static str,
    /// The path of the page.
    pub(super) path: &'static str,
    /// The views by people in the period.
    pub(super) views: u64,
}

/// The access statistics.
#[derive(Debug, Template)]
#[template(path = "admin/stats.html")]
pub(super) struct StatsTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
    /// The logged-in account.
    pub(super) username: &'a str,
    /// The CSRF token of the session, for logging out.
    pub(super) csrf: &'a str,
    /// The number of days shown.
    pub(super) period_days: usize,
    /// The views by people in the period.
    pub(super) total_views: u64,
    /// The views by crawlers in the period.
    pub(super) total_bot_views: u64,
    /// The highest daily views, the scale of the bars.
    pub(super) max_views: u64,
    /// The days, newest first.
    pub(super) days: &'a [StatsDayRow],
    /// The pages, in navigation order.
    pub(super) pages: &'a [StatsPageRow],
}
