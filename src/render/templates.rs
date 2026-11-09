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

/// The XML sitemap.
#[derive(Debug, Template)]
#[template(path = "seo/sitemap.xml")]
pub(super) struct SitemapTemplate<'a> {
    /// The URLs.
    pub(super) entries: &'a [SitemapEntry],
}
