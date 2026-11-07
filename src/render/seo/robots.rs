// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `robots.txt`.

use crate::config::BaseUrl;
use crate::content::pages::SITEMAP_PATH;

/// Returns the `robots.txt`: everything may be crawled, and the sitemap is
/// announced. The login area is kept out of search results by `noindex`
/// headers instead, as a `Disallow` would prevent crawlers from seeing them.
pub(crate) fn robots_txt(base: &BaseUrl) -> String {
    format!(
        "User-agent: *\nDisallow:\n\nSitemap: {}\n",
        base.join(SITEMAP_PATH)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file allows everything and points to the sitemap.
    #[test]
    fn allows_all_and_announces_sitemap() {
        let base = BaseUrl::parse("https://praxis.test").expect("URL should be valid");

        assert_eq!(
            robots_txt(&base),
            "User-agent: *\nDisallow:\n\nSitemap: https://praxis.test/sitemap.xml\n"
        );
    }
}
