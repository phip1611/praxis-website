// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared state of the running service.

use crate::config::Config;
use crate::content::news::NewsDocument;
use crate::delivery::assets::AssetCatalog;
use crate::delivery::snapshot::Snapshot;
use crate::render::{RenderError, SiteRenderer};
use crate::security::headers::SecurityHeaders;
use std::io;
use std::sync::Arc;
use thiserror::Error;

/// A failure that prevents the service from starting.
#[derive(Debug, Error)]
pub(crate) enum StartupError {
    /// The pages can't be rendered.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// An I/O operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What failed.
        context: String,
        /// The underlying error.
        source: io::Error,
    },
}

/// The state that all request handlers share.
#[derive(Clone, Debug)]
pub(crate) struct AppState {
    /// The precomputed public responses.
    pub(crate) site: Arc<Snapshot>,
    /// The security headers of all responses.
    pub(crate) security_headers: Arc<SecurityHeaders>,
}

impl AppState {
    /// Renders the snapshot of the public pages.
    ///
    /// # Errors
    ///
    /// Returns [`StartupError`] if the pages can't be rendered.
    pub(crate) fn new(config: &Config) -> Result<Self, StartupError> {
        let news = NewsDocument::seed();
        let assets = Arc::new(AssetCatalog::new());
        let renderer = Arc::new(SiteRenderer::new(config, assets));
        let site = Arc::new(renderer.render_site(&news)?);
        let https = config.base_url.is_https();
        let security_headers = Arc::new(SecurityHeaders::new(https));
        Ok(Self {
            site,
            security_headers,
        })
    }
}
