// SPDX-License-Identifier: MIT OR Apache-2.0

//! The entries of the XML sitemap. The markup is in `templates/seo`.

/// One URL in the sitemap.
#[derive(Clone, Debug)]
pub(crate) struct SitemapEntry {
    /// The absolute URL.
    pub(crate) loc: String,
    /// When the content last changed, in W3C datetime format, if it's known.
    pub(crate) lastmod: Option<String>,
    /// The absolute URL of the main image, if any.
    pub(crate) image: Option<String>,
}
