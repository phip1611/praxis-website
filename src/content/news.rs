// SPDX-License-Identifier: MIT OR Apache-2.0

//! The editable "Aktuelle Informationen" on the start page.

use crate::security::sanitize::SanitizedHtml;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// The content of the start page as of the migration from WordPress.
const SEED_HTML: &str = include_str!("../../content/aktuelle-informationen.html");

/// When the WordPress post was first published.
const SEED_PUBLISHED_AT: &str = "2026-10-24T19:00:00Z";

/// When the WordPress post was last modified.
const SEED_MODIFIED_AT: &str = "2026-11-07T08:31:14Z";

/// The current news on the start page, as stored in the state directory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct NewsDocument {
    /// Increases with every change. Detects concurrent edits.
    pub(crate) revision: u64,
    /// The content.
    pub(crate) html: SanitizedHtml,
    /// When the news was first published.
    pub(crate) published_at: Timestamp,
    /// When the content last changed.
    pub(crate) modified_at: Timestamp,
    /// Who changed the content last.
    pub(crate) modified_by: String,
}

impl NewsDocument {
    /// Returns the content as of the migration from WordPress, used until the
    /// first edit.
    pub(crate) fn seed() -> Self {
        Self {
            revision: 1,
            html: SanitizedHtml::sanitize(SEED_HTML),
            published_at: SEED_PUBLISHED_AT
                .parse()
                .expect("the seed timestamp should be valid"),
            modified_at: SEED_MODIFIED_AT
                .parse()
                .expect("the seed timestamp should be valid"),
            modified_by: "Dirk Schuster".to_string(),
        }
    }

    /// Returns the next revision with new content.
    #[must_use]
    pub(crate) fn revise(&self, html: SanitizedHtml, modified_by: &str, now: Timestamp) -> Self {
        Self {
            revision: self.revision + 1,
            html,
            published_at: self.published_at,
            modified_at: now,
            modified_by: modified_by.to_string(),
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

        assert_eq!(seed.revision, 1);
        assert!(seed.html.as_str().contains(SEED_MARKER));
        assert_eq!(seed.published_at.to_string(), SEED_PUBLISHED_AT);
    }

    /// A revision keeps the publication date and records the editor.
    #[test]
    fn revise_updates_metadata() {
        let seed = NewsDocument::seed();
        let now: Timestamp = "2026-11-08T10:00:00Z".parse().expect("should parse");

        let next = seed.revise(SanitizedHtml::sanitize("<p>neu</p>"), "dirk", now);

        assert_eq!(next.revision, 2);
        assert_eq!(next.html.as_str(), "<p>neu</p>");
        assert_eq!(next.published_at, seed.published_at);
        assert_eq!(next.modified_at, now);
        assert_eq!(next.modified_by, "dirk");
    }

    /// The document survives a round trip through its storage format.
    #[test]
    fn serde_round_trip_preserves_document() {
        let seed = NewsDocument::seed();

        let json = serde_json::to_string(&seed).expect("should serialize");
        let parsed: NewsDocument = serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(parsed, seed);
    }
}
