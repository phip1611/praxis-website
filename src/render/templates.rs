// SPDX-License-Identifier: MIT OR Apache-2.0

//! The askama templates. The markup lives in the `templates` directory.

use crate::render::layout::Layout;
use askama::Template;

/// The page for unknown paths.
#[derive(Debug, Template)]
#[template(path = "not_found.html")]
pub(super) struct NotFoundTemplate<'a> {
    /// The page frame.
    pub(super) layout: &'a Layout,
}
