// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders the public pages.

use crate::content::news::NewsDocument;
use crate::content::pages::{HOME_PATH, PageImage, home_title};
use crate::content::site::SITE;
use crate::render::layout::{Layout, OpenGraph, OpenGraphImage, OpenGraphKind, ROBOTS_PUBLIC};
use crate::render::templates::HomeTemplate;
use crate::render::{RenderError, SiteRenderer};
use askama::Template;

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
}
