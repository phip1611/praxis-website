// SPDX-License-Identifier: MIT OR Apache-2.0

//! The data of the page frame: head, header, navigation, and footer.

use crate::content::practice::PhoneNumber;
use serde_json::Value;

/// The `robots` value of public pages, as on the WordPress site.
pub(super) const ROBOTS_PUBLIC: &str =
    "index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1";

/// The `robots` value of pages that must not appear in search results.
pub(super) const ROBOTS_PRIVATE: &str = "noindex, nofollow";

/// The image of an Open Graph preview.
#[derive(Clone, Debug)]
pub(super) struct OpenGraphImage {
    /// The absolute URL.
    pub(super) url: String,
    /// The width in pixels.
    pub(super) width: u32,
    /// The height in pixels.
    pub(super) height: u32,
    /// The alternative text.
    pub(super) alt: &'static str,
}

/// The Open Graph type of a page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OpenGraphKind {
    /// The start page.
    Website,
    /// All other pages, as on the WordPress site.
    Article,
}

impl OpenGraphKind {
    /// Returns the value of `og:type`.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Website => "website",
            Self::Article => "article",
        }
    }
}

/// The Open Graph metadata for previews in messengers and social media.
#[derive(Clone, Debug)]
pub(super) struct OpenGraph {
    /// The type of the page.
    pub(super) kind: OpenGraphKind,
    /// The title of the preview.
    pub(super) title: String,
    /// The canonical URL.
    pub(super) url: String,
    /// The preview image.
    pub(super) image: OpenGraphImage,
}

/// An entry of the main navigation.
#[derive(Clone, Debug)]
pub(super) struct NavLink {
    /// The URL path.
    pub(super) href: &'static str,
    /// The visible text.
    pub(super) label: &'static str,
    /// Whether this is the current page.
    pub(super) current: bool,
    /// Whether the link leads to the privacy policy.
    pub(super) privacy_policy: bool,
}

/// The URLs of the stylesheet and icons.
#[derive(Clone, Debug)]
pub(super) struct LayoutAssets {
    /// The stylesheet.
    pub(super) style: String,
    /// The 32x32 favicon.
    pub(super) icon_32: String,
    /// The 192x192 icon for Android.
    pub(super) icon_192: String,
    /// The 180x180 icon for iOS.
    pub(super) apple_touch: String,
    /// The 270x270 tile for Windows.
    pub(super) tile: String,
}

/// The data that the analytics script needs.
#[derive(Clone, Debug)]
pub(super) struct AnalyticsView {
    /// The URL of the local bootstrap script.
    pub(super) script_url: String,
    /// The tracking endpoint.
    pub(super) tracker_url: String,
    /// The tracking library.
    pub(super) library_url: String,
    /// The site ID.
    pub(super) site_id: u32,
    /// The tracking pixel for browsers without JavaScript.
    pub(super) noscript_url: String,
}

/// Everything that the base template needs.
#[derive(Clone, Debug)]
pub(super) struct Layout {
    /// The content of `<title>`.
    pub(super) title: String,
    /// The meta description.
    pub(super) description: String,
    /// The `robots` meta value.
    pub(super) robots: &'static str,
    /// The canonical URL, for public pages.
    pub(super) canonical: Option<String>,
    /// The Open Graph data, for public pages.
    pub(super) open_graph: Option<OpenGraph>,
    /// The JSON-LD structured data.
    pub(super) json_ld: Option<Value>,
    /// Whether the site name is the `<h1>`. Only on the start page, where the
    /// news heading is the `<h2>`.
    pub(super) site_title_is_heading: bool,
    /// The name of the site.
    pub(super) site_name: &'static str,
    /// The tagline below the site name.
    pub(super) tagline: &'static str,
    /// The address of the practice, for the footer.
    pub(super) address: String,
    /// The phone number of the practice, for the footer.
    pub(super) phone: &'static PhoneNumber,
    /// The main navigation.
    pub(super) nav: Vec<NavLink>,
    /// The stylesheet and icons.
    pub(super) assets: LayoutAssets,
    /// Scripts to load, e.g., the editor.
    pub(super) scripts: Vec<String>,
    /// The analytics configuration, if enabled for this page.
    pub(super) analytics: Option<AnalyticsView>,
}
