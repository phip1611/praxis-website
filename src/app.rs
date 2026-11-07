// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared state of the running service.

use crate::clock::Clock;
use crate::config::Config;
use crate::content::news::NewsDocument;
use crate::content::store::NewsStore;
use crate::delivery::assets::AssetCatalog;
use crate::delivery::snapshot::SiteCache;
use crate::persist::StoreError;
use crate::render::{RenderError, SiteRenderer};
use crate::security::authenticator::Authenticator;
use crate::security::cookie::SessionCookie;
use crate::security::credentials::{CredentialStore, CredentialsError};
use crate::security::headers::SecurityHeaders;
use crate::security::password::PasswordError;
use crate::security::password_overrides::PasswordOverrides;
use crate::security::token::{RandomError, SecretToken};
use crate::stats::AccessStats;
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

/// A failure that prevents the service from starting.
#[derive(Debug, Error)]
pub(crate) enum StartupError {
    /// The stored news can't be loaded.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The pages can't be rendered.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// The accounts file is invalid.
    #[error(transparent)]
    Credentials(#[from] CredentialsError),
    /// The development account can't be created.
    #[error(transparent)]
    Password(#[from] PasswordError),
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
    /// The source of the current time.
    pub(crate) clock: Arc<dyn Clock>,
    /// Renders pages.
    pub(crate) renderer: Arc<SiteRenderer>,
    /// The precomputed public responses.
    pub(crate) site: Arc<SiteCache>,
    /// The current news. The lock also serializes saves.
    pub(crate) news: Arc<Mutex<NewsDocument>>,
    /// The persistence of the news.
    pub(crate) store: Arc<dyn NewsStore>,
    /// Logins and sessions.
    pub(crate) auth: Arc<Authenticator>,
    /// The access statistics.
    pub(crate) stats: Arc<AccessStats>,
    /// The session cookie policy.
    pub(crate) cookie: SessionCookie,
    /// The security headers of all responses.
    pub(crate) security_headers: Arc<SecurityHeaders>,
    /// A random ID per start, for the live reload of the development mode.
    pub(crate) boot_id: Arc<str>,
}

impl AppState {
    /// Loads the news and renders the initial snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StartupError`] if the news can't be loaded or rendered.
    pub(crate) fn new(
        config: Config,
        clock: Arc<dyn Clock>,
        store: Arc<dyn NewsStore>,
        credentials: CredentialStore,
    ) -> Result<Self, StartupError> {
        let news = store.load()?.unwrap_or_else(NewsDocument::seed);
        let credentials = Arc::new(credentials);
        let overrides = PasswordOverrides::load(&config.state_dir, &credentials);
        let assets = Arc::new(AssetCatalog::new());
        let renderer = Arc::new(SiteRenderer::new(&config, assets));
        let site = Arc::new(SiteCache::new(renderer.render_site(&news)?));
        let https = config.base_url.is_https();
        let security_headers = Arc::new(SecurityHeaders::new(
            https,
            config
                .analytics
                .as_ref()
                .map(|analytics| analytics.origin()),
        ));
        Ok(Self {
            stats: Arc::new(AccessStats::load(Arc::clone(&clock), &config.state_dir)),
            auth: Arc::new(Authenticator::new(credentials, overrides, &clock)?),
            config: Arc::new(config),
            clock,
            renderer,
            site,
            news: Arc::new(Mutex::new(news)),
            store,
            cookie: SessionCookie::new(https),
            security_headers,
            boot_id: SecretToken::generate()?.as_str().into(),
        })
    }
}
