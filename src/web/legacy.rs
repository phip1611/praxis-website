// SPDX-License-Identifier: MIT OR Apache-2.0

//! URLs of the WordPress site that no longer exist as pages.
//!
//! Search engines, bookmarks, and other sites may still point to them, so
//! they redirect to the matching new page. Old image URLs are handled by
//! [`crate::delivery::assets::AssetCatalog::legacy_image_url`].

use crate::content::pages::{HOME_PATH, SITEMAP_PATH};
use crate::web::admin::{EDITOR_PATH, LOGIN_PATH};

/// What happens with an old URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LegacyTarget {
    /// Redirect permanently to this path.
    Moved(&'static str),
    /// The content is gone for good.
    Gone,
}

/// Old paths and their new location.
const REDIRECTS: [(&str, &str); 15] = [
    // The news was a WordPress post; it lives on the start page now.
    ("/2026/10/aktuelle-informationen/", HOME_PATH),
    ("/category/aktuelles/", HOME_PATH),
    ("/author/boeser-admin/", HOME_PATH),
    ("/author/webmaster/", HOME_PATH),
    // Attachment pages of the media library.
    (
        "/kontakt/visitenkarte-arztpraxis-dirk-schuster/",
        "/kontakt/",
    ),
    ("/logo-und-favicon/", HOME_PATH),
    ("/cropped-logo-und-favicon-png/", HOME_PATH),
    // Yoast SEO split the sitemap into several files.
    ("/sitemap_index.xml", SITEMAP_PATH),
    ("/post-sitemap.xml", SITEMAP_PATH),
    ("/page-sitemap.xml", SITEMAP_PATH),
    ("/attachment-sitemap.xml", SITEMAP_PATH),
    ("/wp-sitemap.xml", SITEMAP_PATH),
    // Bookmarks of the old login.
    ("/wp-admin/", EDITOR_PATH),
    ("/wp-login.php", LOGIN_PATH),
    ("/admin", EDITOR_PATH),
];

/// Old paths of content that was removed on purpose.
const GONE: [&str; 4] = [
    // There never were comments, and comments are disabled.
    "/comments/feed/",
    // The site has no feeds anymore. Gone, so feed readers stop asking.
    "/feed/",
    "/2026/10/aktuelle-informationen/feed/",
    "/category/aktuelles/feed/",
];

/// Returns what to do with the old URL `path`, if it is one.
///
/// Matches with and without trailing slash, as WordPress accepted both.
pub(super) fn resolve(path: &str) -> Option<LegacyTarget> {
    let with_slash = if path.ends_with('/') || path.contains('.') {
        None
    } else {
        Some(format!("{path}/"))
    };
    let matches = |candidate: &str| candidate == path || with_slash.as_deref() == Some(candidate);
    if let Some((_, target)) = REDIRECTS.iter().find(|(old, _)| matches(old)) {
        return Some(LegacyTarget::Moved(target));
    }
    GONE.iter()
        .any(|old| matches(old))
        .then_some(LegacyTarget::Gone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestApp;
    use axum::http::StatusCode;

    /// The old news post and its archive pages lead to the start page.
    #[test]
    fn news_urls_redirect_to_start_page() {
        assert_eq!(
            resolve("/2026/10/aktuelle-informationen/"),
            Some(LegacyTarget::Moved("/"))
        );
        assert_eq!(
            resolve("/2026/10/aktuelle-informationen"),
            Some(LegacyTarget::Moved("/"))
        );
        assert_eq!(
            resolve("/category/aktuelles/"),
            Some(LegacyTarget::Moved("/"))
        );
    }

    /// All old sitemaps lead to the single new one.
    #[test]
    fn sitemaps_redirect_to_new_sitemap() {
        for old in ["/sitemap_index.xml", "/post-sitemap.xml", "/wp-sitemap.xml"] {
            assert_eq!(
                resolve(old),
                Some(LegacyTarget::Moved("/sitemap.xml")),
                "{old}"
            );
        }
    }

    /// File-like paths don't get a slash appended.
    #[test]
    fn files_match_exactly() {
        assert_eq!(resolve("/sitemap_index.xml/"), None);
        assert_eq!(
            resolve("/wp-login.php"),
            Some(LegacyTarget::Moved("/admin/login/"))
        );
    }

    /// Removed content is reported as gone, so crawlers stop asking.
    #[test]
    fn feeds_are_gone() {
        assert_eq!(resolve("/comments/feed/"), Some(LegacyTarget::Gone));
        assert_eq!(resolve("/comments/feed"), Some(LegacyTarget::Gone));
        assert_eq!(resolve("/feed/"), Some(LegacyTarget::Gone));
        assert_eq!(
            resolve("/category/aktuelles/feed/"),
            Some(LegacyTarget::Gone)
        );
    }

    /// Current and unknown paths aren't affected.
    #[test]
    fn other_paths_are_not_legacy() {
        assert_eq!(resolve("/kontakt/"), None);
        assert_eq!(resolve("/wp-content/plugins/x.php"), None);
    }

    /// Every redirect leads to an existing page and not to another redirect,
    /// which would cost crawlers a request.
    #[tokio::test]
    async fn redirect_targets_exist() {
        let app = TestApp::new();

        for (_, target) in REDIRECTS {
            let status = app.get(target).await.status;

            assert_eq!(resolve(target), None, "{target}");
            // The login area answers with a redirect to its login form.
            assert!(
                status == StatusCode::OK || status == StatusCode::SEE_OTHER,
                "{target}"
            );
        }
    }
}
