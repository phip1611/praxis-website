// SPDX-License-Identifier: MIT OR Apache-2.0

//! The configuration of the Matomo web analytics.
//!
//! The site uses a self-hosted Matomo without cookies. The tracker files on
//! the server have custom names, so that ad blockers don't block them.

use crate::config::base_url::{BaseUrlError, parse_origin_and_path};

/// The file name of the tracking endpoint on the Matomo server.
const TRACKER_FILE: &str = "yikes.php";

/// The file name of the tracking library on the Matomo server.
const LIBRARY_FILE: &str = "yikes.js";

/// The URL of a Matomo installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatomoUrl {
    /// The origin of the Matomo server, for the Content Security Policy.
    origin: String,
    /// The URL of the Matomo installation, ending with `/`.
    base: String,
}

impl MatomoUrl {
    /// Parses and normalizes the URL of a Matomo installation.
    ///
    /// # Errors
    ///
    /// Returns [`BaseUrlError`] if the URL isn't an absolute `http(s)` URL.
    pub(crate) fn parse(url: &str) -> Result<Self, BaseUrlError> {
        let (origin, path) = parse_origin_and_path(url)?;
        Ok(Self {
            base: format!("{origin}{path}"),
            origin,
        })
    }
}

/// The Matomo configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Analytics {
    /// The Matomo installation.
    url: MatomoUrl,
    /// The ID of this site in Matomo.
    site_id: u32,
}

impl Analytics {
    /// Creates the configuration.
    pub(super) const fn new(url: MatomoUrl, site_id: u32) -> Self {
        Self { url, site_id }
    }

    /// Returns the origin of the Matomo server.
    pub(crate) fn origin(&self) -> &str {
        &self.url.origin
    }

    /// Returns the ID of this site in Matomo.
    pub(crate) const fn site_id(&self) -> u32 {
        self.site_id
    }

    /// Returns the URL of the tracking endpoint.
    pub(crate) fn tracker_url(&self) -> String {
        format!("{}{TRACKER_FILE}", self.url.base)
    }

    /// Returns the URL of the tracking library.
    pub(crate) fn library_url(&self) -> String {
        format!("{}{LIBRARY_FILE}", self.url.base)
    }

    /// Returns the URL of the tracking pixel for browsers without JavaScript.
    pub(crate) fn noscript_url(&self) -> String {
        format!(
            "{}js/index.php?idsite={}&rec=1",
            self.url.base, self.site_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The URLs match the snippet of the previous website.
    #[test]
    fn urls_match_previous_snippet() {
        let url = MatomoUrl::parse("https://stats.test").expect("URL should be valid");
        let analytics = Analytics::new(url, 9);

        assert_eq!(analytics.origin(), "https://stats.test");
        assert_eq!(analytics.tracker_url(), "https://stats.test/yikes.php");
        assert_eq!(analytics.library_url(), "https://stats.test/yikes.js");
        assert_eq!(
            analytics.noscript_url(),
            "https://stats.test/js/index.php?idsite=9&rec=1"
        );
        assert_eq!(analytics.site_id(), 9);
    }

    /// Matomo may live in a subdirectory.
    #[test]
    fn keeps_installation_path() {
        let url = MatomoUrl::parse("https://stats.test/matomo").expect("URL should be valid");
        let analytics = Analytics::new(url, 1);

        assert_eq!(
            analytics.tracker_url(),
            "https://stats.test/matomo/yikes.php"
        );
        assert_eq!(analytics.origin(), "https://stats.test");
    }
}
