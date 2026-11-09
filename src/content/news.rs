// SPDX-License-Identifier: MIT OR Apache-2.0

//! The "Aktuelle Informationen" on the start page.

use crate::security::sanitize::SanitizedHtml;
use jiff::Timestamp;

/// The content of the start page as of the migration from WordPress.
const SEED_HTML: &str = include_str!("../../content/aktuelle-informationen.html");

/// When the WordPress post was first published.
const SEED_PUBLISHED_AT: &str = "2026-10-24T19:00:00Z";

/// When the WordPress post was last modified.
const SEED_MODIFIED_AT: &str = "2026-11-07T08:31:14Z";

/// The news on the start page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewsDocument {
    /// The content.
    pub(crate) html: SanitizedHtml,
    /// When the news was first published.
    pub(crate) published_at: Timestamp,
    /// When the content last changed.
    pub(crate) modified_at: Timestamp,
}

impl NewsDocument {
    /// Returns the content as of the migration from WordPress.
    pub(crate) fn seed() -> Self {
        Self {
            html: SanitizedHtml::sanitize(SEED_HTML),
            published_at: SEED_PUBLISHED_AT
                .parse()
                .expect("the seed timestamp should be valid"),
            modified_at: SEED_MODIFIED_AT
                .parse()
                .expect("the seed timestamp should be valid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::SEED_MARKER;

    /// The seed is the last content of the WordPress site.
    #[test]
    fn seed_contains_migrated_content() {
        let seed = NewsDocument::seed();

        assert!(seed.html.as_str().contains(SEED_MARKER));
        assert_eq!(seed.published_at.to_string(), SEED_PUBLISHED_AT);
    }
}
