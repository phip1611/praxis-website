// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared state of the running service.

use crate::clock::Clock;
use crate::config::Config;
use crate::content::news::NewsDocument;
use crate::delivery::assets::AssetCatalog;
use crate::delivery::snapshot::Snapshot;
use crate::render::{RenderError, SiteRenderer};
use crate::security::headers::SecurityHeaders;
use crate::security::token::{RandomError, SecretToken};
use crate::stats::AccessStats;
use std::io;
use std::sync::Arc;
use thiserror::Error;

/// A failure that prevents the service from starting.
#[derive(Debug, Error)]
pub(crate) enum StartupError {
    /// The pages can't be rendered.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// The random number generator failed.
    #[error(transparent)]
    Random(#[from] RandomError),
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
    /// The configuration.
    pub(crate) config: Arc<Config>,
    /// The precomputed public responses.
    pub(crate) site: Arc<Snapshot>,
    /// The access statistics.
    pub(crate) stats: Arc<AccessStats>,
    /// The security headers of all responses.
    pub(crate) security_headers: Arc<SecurityHeaders>,
    /// A random ID per start, for the live reload of the development mode.
    pub(crate) boot_id: Arc<str>,
}

impl AppState {
    /// Renders the snapshot of the public pages.
    ///
    /// # Errors
    ///
    /// Returns [`StartupError`] if the pages can't be rendered.
    pub(crate) fn new(config: Config, clock: Arc<dyn Clock>) -> Result<Self, StartupError> {
        let news = NewsDocument::seed();
        let assets = Arc::new(AssetCatalog::new());
        let renderer = Arc::new(SiteRenderer::new(&config, assets));
        let site = Arc::new(renderer.render_site(&news)?);
        let https = config.base_url.is_https();
        let security_headers = Arc::new(SecurityHeaders::new(
            https,
            config
                .analytics
                .as_ref()
                .map(|analytics| analytics.origin()),
        ));
        Ok(Self {
            stats: Arc::new(AccessStats::load(clock, &config.state_dir)),
            config: Arc::new(config),
            site,
            security_headers,
            boot_id: SecretToken::generate()?.as_str().into(),
        })
    }
}
