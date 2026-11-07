// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders the public snapshot of all pages.

use crate::content::news::NewsDocument;
use crate::content::pages::{HOME_PATH, PageImage, PageSpec, home_title};
use crate::content::site::SITE;
use crate::delivery::cached::{CachePolicy, CachedResponse, Compression};
use crate::delivery::snapshot::Snapshot;
use crate::render::layout::{Layout, OpenGraph, OpenGraphImage, OpenGraphKind, ROBOTS_PUBLIC};
use crate::render::templates::{BusinessCardImages, HomeTemplate, NotFoundTemplate, PageTemplate};
use crate::render::{RenderError, SiteRenderer};
use askama::Template;
use axum::http::StatusCode;
use std::collections::HashMap;
use std::iter;
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

/// The `Content-Type` of HTML pages.
const HTML: &str = "text/html; charset=utf-8";

/// The smaller variants of the business card as `(file name suffix, width)`.
/// The full size is the business card of `content/site.toml`.
const CARD_VARIANTS: [(&str, u32); 3] = [("-300x200", 300), ("-450x300", 450), ("-1024x683", 1024)];

/// The data of a public page that differs between pages.
#[derive(Debug)]
struct PublicPage<'a> {
    /// The URL path.
    path: &'static str,
    /// The `<title>`.
    title: String,
    /// The title of previews.
    og_title: String,
    /// The meta description.
    description: &'a str,
    /// The Open Graph type.
    og_kind: OpenGraphKind,
    /// When the page last changed, if it's known.
    modified: Option<&'a str>,
    /// The main image.
    image: Option<&'static PageImage>,
}

impl SiteRenderer {
    /// Renders and compresses all public responses.
    ///
    /// CPU-intensive: don't call it on the async runtime.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if a template fails.
    pub(crate) fn render_site(&self, news: &NewsDocument) -> Result<Snapshot, RenderError> {
        let start = Instant::now();
        let mut routes: HashMap<String, Arc<CachedResponse>> = self
            .assets
            .routes()
            .map(|(path, response)| (path.to_string(), Arc::clone(response)))
            .collect();
        let mut add = |path: &str, content_type, body: String| {
            let response = CachedResponse::new(
                StatusCode::OK,
                content_type,
                CachePolicy::Revalidate,
                body.into_bytes(),
                Compression::Precompress,
            );
            routes.insert(path.to_string(), Arc::new(response));
        };

        add(HOME_PATH, HTML, self.render_home(news)?);
        for page in &SITE.pages {
            add(&page.path, HTML, self.render_page(page)?);
        }

        let not_found = CachedResponse::new(
            StatusCode::NOT_FOUND,
            HTML,
            CachePolicy::Revalidate,
            self.render_not_found()?.into_bytes(),
            Compression::Precompress,
        );
        debug!(elapsed = ?start.elapsed(), "rendered site");
        Ok(Snapshot::new(routes, Arc::new(not_found)))
    }

    /// Renders the start page.
    fn render_home(&self, news: &NewsDocument) -> Result<String, RenderError> {
        let modified = news.modified_at.to_string();
        let layout = self.public_layout(&PublicPage {
            path: HOME_PATH,
            title: home_title(),
            og_title: SITE.practice.site_name.to_string(),
            description: &SITE.home.description,
            og_kind: OpenGraphKind::Website,
            modified: Some(&modified),
            image: None,
        });
        Ok(HomeTemplate {
            layout: &layout,
            heading: &SITE.home.news_heading,
            news: &news.html,
        }
        .render()?)
    }

    /// Renders a fixed page.
    fn render_page(&self, page: &'static PageSpec) -> Result<String, RenderError> {
        let layout = self.public_layout(&PublicPage {
            path: &page.path,
            title: page.title(),
            og_title: page.title(),
            description: &page.description,
            og_kind: OpenGraphKind::Article,
            modified: None,
            image: page.image(),
        });
        Ok(PageTemplate {
            layout: &layout,
            page,
            practice: &SITE.practice,
            hours: &SITE.opening_hours,
            card: &self.business_card(),
        }
        .render()?)
    }

    /// Renders the page for unknown paths.
    fn render_not_found(&self) -> Result<String, RenderError> {
        let layout = self.private_layout("Seite nicht gefunden");
        Ok(NotFoundTemplate { layout: &layout }.render()?)
    }

    /// Returns the frame of a public page with all search engine metadata.
    fn public_layout(&self, page: &PublicPage<'_>) -> Layout {
        let url = self.base_url.join(page.path);
        let preview = page.image.unwrap_or(&SITE.logo);
        let image = OpenGraphImage {
            url: self.asset_url(&preview.asset),
            width: preview.width,
            height: preview.height,
            alt: &preview.caption,
        };
        let mut layout = self.base_layout(page.title.clone(), page.description, Some(page.path));
        layout.robots = ROBOTS_PUBLIC;
        layout.canonical = Some(url.clone());
        layout.open_graph = Some(OpenGraph {
            kind: page.og_kind,
            title: page.og_title.clone(),
            url,
            image,
        });
        layout.site_title_is_heading = page.path == HOME_PATH;
        layout
    }

    /// Returns the URLs of the business card variants.
    fn business_card(&self) -> BusinessCardImages {
        let srcset = |extension: &str| {
            CARD_VARIANTS
                .iter()
                .chain(iter::once(&("", SITE.business_card.width)))
                .map(|(suffix, width)| {
                    let name =
                        format!("images/Visitenkarte-Arztpraxis-Dirk-Schuster{suffix}.{extension}");
                    format!("{} {width}w", self.assets.url(&name))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        BusinessCardImages {
            full_png: self.assets.url(&SITE.business_card.asset).to_string(),
            png_srcset: srcset("png"),
            webp_srcset: srcset("webp"),
            width: SITE.business_card.width,
            height: SITE.business_card.height,
            alt: "Visitenkarte der Arztpraxis Dipl.-Med. Dirk Schuster, Praktischer Arzt - \
                  Hausarztpraxis, mit Öffnungszeiten, Adresse, E-Mail, Telefon, Fax, \
                  Mobilnummer, Webadresse und QR-Code",
            caption: &SITE.business_card.caption,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::assets::AssetCatalog;
    use crate::test_support::test_config;

    /// Creates a renderer for the test configuration.
    fn renderer() -> SiteRenderer {
        SiteRenderer::new(&test_config(), Arc::new(AssetCatalog::new()))
    }

    /// The site has the start page, all fixed pages, and assets.
    #[test]
    fn render_site_contains_all_routes() {
        let snapshot = renderer()
            .render_site(&NewsDocument::seed())
            .expect("rendering should succeed");

        let mut expected = vec![HOME_PATH, "/favicon.ico"];
        expected.extend(SITE.pages.iter().map(|page| page.path.as_str()));
        for path in expected {
            assert!(snapshot.get(path).is_some(), "{path}");
        }
        assert!(
            snapshot
                .paths()
                .any(|path| path.starts_with("/assets/style."))
        );
    }

    /// Every business card variant exists, so the srcset has no broken URL.
    #[test]
    fn business_card_lists_all_variants() {
        let card = renderer().business_card();

        assert_eq!(
            card.png_srcset.matches(".png ").count(),
            CARD_VARIANTS.len() + 1
        );
        assert_eq!(
            card.webp_srcset.matches(".webp ").count(),
            CARD_VARIANTS.len() + 1
        );
        assert!(
            card.png_srcset
                .ends_with(&format!("{}w", SITE.business_card.width))
        );
    }
}
