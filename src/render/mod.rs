// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders all pages and the sitemap.
//!
//! [`SiteRenderer`] is split by audience: `public` renders the precomputed
//! snapshot for visitors, `admin` renders the login area per request. This
//! file has the page frame that both share.

mod admin;
mod layout;
mod public;
mod seo;
pub(crate) mod templates;

pub(crate) use admin::EditorView;

use crate::config::{Analytics, BaseUrl, Config};
use crate::content::pages::HOME_PATH;
use crate::content::site::SITE;
use crate::delivery::assets::{self, AssetCatalog};
use crate::render::layout::{Layout, LayoutAssets, NavLink, ROBOTS_PRIVATE};
use std::iter;
use std::sync::Arc;
use thiserror::Error;

/// A failure while rendering.
#[derive(Debug, Error)]
pub(crate) enum RenderError {
    /// A template failed.
    #[error("template rendering failed: {0}")]
    Template(#[from] askama::Error),
}

/// Renders pages for one deployment.
#[derive(Debug)]
pub(crate) struct SiteRenderer {
    /// The public URL of the site.
    base_url: BaseUrl,
    /// The analytics configuration.
    analytics: Option<Analytics>,
    /// Whether to include the live reload script of the development mode.
    live_reload: bool,
    /// The embedded files.
    assets: Arc<AssetCatalog>,
}

impl SiteRenderer {
    /// Creates a renderer for the given configuration.
    pub(crate) fn new(config: &Config, assets: Arc<AssetCatalog>) -> Self {
        Self {
            base_url: config.base_url.clone(),
            analytics: config.analytics.clone(),
            live_reload: config.dev,
            assets,
        }
    }

    /// Returns the embedded files.
    pub(crate) fn assets(&self) -> &AssetCatalog {
        &self.assets
    }

    /// Returns the absolute URL of an embedded file.
    fn asset_url(&self, name: &str) -> String {
        self.base_url.join(self.assets.url(name))
    }

    /// Returns the frame of a page that search engines must not index.
    fn private_layout(&self, heading: &str, mut scripts: Vec<String>) -> Layout {
        let mut layout = self.base_layout(
            format!("{heading} – {}", SITE.practice.site_name),
            &SITE.practice.tagline,
            None,
        );
        layout.scripts.append(&mut scripts);
        layout
    }

    /// Returns the frame that all pages share. It's private by default;
    /// public pages add their search engine metadata.
    fn base_layout(&self, title: String, description: &str, current: Option<&str>) -> Layout {
        let home = iter::once((HOME_PATH, SITE.home.label.as_str(), false));
        let pages = SITE.pages.iter().map(|page| {
            (
                page.path.as_str(),
                page.heading.as_str(),
                page.is_privacy_policy(),
            )
        });
        let nav = home
            .chain(pages)
            .map(|(href, label, privacy_policy)| NavLink {
                href,
                label,
                current: current == Some(href),
                privacy_policy,
            })
            .collect();
        let mut scripts = Vec::new();
        if self.live_reload {
            scripts.push(self.assets.url(assets::LIVE_RELOAD_SCRIPT).to_string());
        }
        Layout {
            title,
            description: description.to_string(),
            robots: ROBOTS_PRIVATE,
            canonical: None,
            open_graph: None,
            json_ld: None,
            site_title_is_heading: false,
            site_name: &SITE.practice.site_name,
            tagline: &SITE.practice.tagline,
            address: format!(
                "{}, {} {}",
                SITE.practice.street, SITE.practice.postal_code, SITE.practice.locality
            ),
            phone: &SITE.practice.phone,
            nav,
            assets: LayoutAssets {
                style: self.assets.url(assets::STYLESHEET).to_string(),
                icon_32: self.assets.url(assets::ICON_32).to_string(),
                icon_192: self.assets.url(assets::ICON_192).to_string(),
                apple_touch: self.assets.url(assets::ICON_180).to_string(),
                // Windows needs an absolute URL here.
                tile: self.asset_url(assets::ICON_270),
            },
            scripts,
            analytics: None,
        }
    }

    /// Returns the logo for the structured data and previews.
    fn logo_url(&self) -> String {
        self.asset_url(&SITE.logo.asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_config;

    /// The login area pages are not indexed and have no analytics.
    #[test]
    fn private_pages_are_not_indexed() {
        let renderer = SiteRenderer::new(&test_config(), Arc::new(AssetCatalog::new()));

        let html = renderer
            .render_login("", None)
            .expect("rendering should succeed");

        assert!(html.contains("<meta name=\"robots\" content=\"noindex, nofollow\">"));
        assert!(!html.contains("rel=\"canonical\""));
    }
}
