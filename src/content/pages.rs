// SPDX-License-Identifier: MIT OR Apache-2.0

//! The structure of the public pages in `content/site.toml`.

use crate::content::site::SITE;
use serde::Deserialize;

/// The path of the start page.
pub(crate) const HOME_PATH: &str = "/";

/// The texts of the start page.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HomePage {
    /// The label in the navigation and breadcrumbs.
    pub(crate) label: String,
    /// The heading of the news.
    pub(crate) news_heading: String,
    /// The description for search results and previews.
    pub(crate) description: String,
}

/// The content of a fixed page.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PageBody {
    /// The opening hours table.
    OpeningHours,
    /// Contact data and business card.
    Contact,
    /// Directions to the practice.
    Directions,
    /// The legal notice.
    Imprint,
    /// The privacy policy.
    Privacy,
}

/// An embedded image with its metadata, e.g., for previews in social media
/// and search.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageImage {
    /// The name of the embedded asset.
    pub(crate) asset: String,
    /// The width in pixels.
    pub(crate) width: u32,
    /// The height in pixels.
    pub(crate) height: u32,
    /// The caption, as on the WordPress site.
    pub(crate) caption: String,
}

/// A fixed page.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageSpec {
    /// The URL path with trailing slash, as on the WordPress site.
    pub(crate) path: String,
    /// The heading, also used in the navigation and the title.
    pub(crate) heading: String,
    /// The description for search results and previews.
    pub(crate) description: String,
    /// Whether the WordPress site announced a reading time for this page.
    pub(crate) reading_time: bool,
    /// The content.
    pub(crate) body: PageBody,
}

/// Returns the title of the start page, as on the WordPress site.
pub(crate) fn home_title() -> String {
    format!("{} – Thalheim", SITE.practice.site_name)
}

impl PageSpec {
    /// Returns the HTML title, e.g., `Kontakt – Hausarztpraxis Dirk Schuster`.
    pub(crate) fn title(&self) -> String {
        format!("{} – {}", self.heading, SITE.practice.site_name)
    }

    /// Returns whether this is the privacy policy, which links get marked as.
    pub(crate) fn is_privacy_policy(&self) -> bool {
        self.body == PageBody::Privacy
    }

    /// Returns the main image, if any. Only the contact page has one, the
    /// business card that it shows.
    pub(crate) fn image(&self) -> Option<&'static PageImage> {
        (self.body == PageBody::Contact).then(|| &SITE.business_card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::practice::OpeningDay;

    /// The paths are those of the WordPress site, with trailing slash.
    #[test]
    fn paths_match_wordpress() {
        let paths: Vec<_> = SITE.pages.iter().map(|page| page.path.as_str()).collect();

        assert_eq!(
            paths,
            [
                "/oeffnungszeiten/",
                "/kontakt/",
                "/anfahrt/",
                "/impressum/",
                "/datenschutz/"
            ]
        );
    }

    /// The titles follow the scheme of the WordPress site.
    #[test]
    fn titles_match_wordpress() {
        assert_eq!(home_title(), "Hausarztpraxis Dirk Schuster – Thalheim");
        assert_eq!(
            SITE.pages[1].title(),
            "Kontakt – Hausarztpraxis Dirk Schuster"
        );
    }

    /// Returns the page with `body`.
    fn page(body: PageBody) -> &'static PageSpec {
        SITE.pages
            .iter()
            .find(|page| page.body == body)
            .expect("every body should have a page")
    }

    /// The description repeats the opening hours in short form ("8-12"), so
    /// it must change together with them.
    #[test]
    fn opening_hours_description_matches_hours() {
        let short = |time: &str| {
            time.trim_end_matches(":00")
                .trim_start_matches('0')
                .to_string()
        };
        let description = &page(PageBody::OpeningHours).description;

        for range in SITE.opening_hours.iter().flat_map(OpeningDay::ranges) {
            let text = format!("{}-{}", short(&range.opens), short(&range.closes));
            assert!(description.contains(&text), "{text}");
        }
    }

    /// The description repeats address and phone number.
    #[test]
    fn contact_description_matches_practice() {
        let description = &page(PageBody::Contact).description;

        assert!(description.contains(&SITE.practice.street));
        assert!(description.contains(&SITE.practice.postal_code));
        assert!(description.contains(&SITE.practice.phone.display));
    }

    /// The logo is captioned with the name of the site.
    #[test]
    fn logo_caption_is_site_name() {
        assert_eq!(SITE.logo.caption, SITE.practice.site_name);
    }

    /// Search engines cut descriptions at about 160 characters, see
    /// <https://developers.google.com/search/docs/appearance/snippet>.
    #[test]
    fn descriptions_fit_search_results() {
        for page in &SITE.pages {
            assert!(page.description.chars().count() <= 160, "{}", page.path);
            assert!(!page.description.contains("  "), "{}", page.path);
        }
    }
}
