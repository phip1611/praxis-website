// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders the page frame.

#[expect(dead_code, reason = "used by code that a later commit adds")]
mod layout;
pub(crate) mod templates;

use crate::config::{BaseUrl, Config};
use crate::content::pages::HOME_PATH;
use crate::content::site::SITE;
use crate::delivery::assets::{self, AssetCatalog};
use crate::render::layout::{Layout, LayoutAssets, NavLink, ROBOTS_PRIVATE};
use crate::render::templates::NotFoundTemplate;
use askama::Template;
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
    /// The embedded files.
    assets: Arc<AssetCatalog>,
}

impl SiteRenderer {
    /// Creates a renderer for the given configuration.
    pub(crate) fn new(config: &Config, assets: Arc<AssetCatalog>) -> Self {
        Self {
            base_url: config.base_url.clone(),
            assets,
        }
    }

    /// Returns the absolute URL of an embedded file.
    fn asset_url(&self, name: &str) -> String {
        self.base_url.join(self.assets.url(name))
    }

    /// Returns the frame of a page that search engines must not index.
    fn private_layout(&self, heading: &str) -> Layout {
        self.base_layout(
            format!("{heading} – {}", SITE.practice.site_name),
            &SITE.practice.tagline,
            None,
        )
    }

    /// Renders the page for unknown paths.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the template fails.
    pub(crate) fn render_not_found(&self) -> Result<String, RenderError> {
        let layout = self.private_layout("Seite nicht gefunden");
        Ok(NotFoundTemplate { layout: &layout }.render()?)
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
        Layout {
            title,
            description: description.to_string(),
            robots: ROBOTS_PRIVATE,
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
        }
    }
}
