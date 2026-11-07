// SPDX-License-Identifier: MIT OR Apache-2.0

//! The askama templates. The markup lives in the `templates` directory.

use crate::render::layout::Layout;
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

/// The page for unknown paths.
#[derive(Debug, Template)]
#[template(path = "not_found.html")]
pub(super) struct NotFoundTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
}
