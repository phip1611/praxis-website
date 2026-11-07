// SPDX-License-Identifier: MIT OR Apache-2.0

//! The data of the page frame: head, header, navigation, and footer.

use crate::content::practice::PhoneNumber;

/// The `robots` value of pages that must not appear in search results.
pub(super) const ROBOTS_PRIVATE: &str = "noindex, nofollow";

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

/// Everything that the base template needs.
#[derive(Clone, Debug)]
pub(super) struct Layout {
    /// The content of `<title>`.
    pub(super) title: String,
    /// The meta description.
    pub(super) description: String,
    /// The `robots` meta value.
    pub(super) robots: &'static str,
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
}
