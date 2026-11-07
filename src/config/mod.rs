// SPDX-License-Identifier: MIT OR Apache-2.0

//! The validated runtime configuration.

mod analytics;
mod base_url;

pub(crate) use analytics::{Analytics, MatomoUrl};
pub(crate) use base_url::BaseUrl;

use crate::cli::ServeArgs;
use std::net::SocketAddr;

/// The validated configuration of the service.
#[derive(Clone, Debug)]
pub(crate) struct Config {
    /// The address to listen on.
    pub(crate) bind: SocketAddr,
    /// The public URL of the site, as seen by visitors.
    pub(crate) base_url: BaseUrl,
    /// The Matomo configuration, if analytics are enabled.
    pub(crate) analytics: Option<Analytics>,
}

impl Config {
    /// Creates the configuration of the server from its options, which clap
    /// has already validated.
    pub(crate) fn production(args: ServeArgs) -> Self {
        Self {
            bind: args.bind,
            base_url: args.base_url,
            analytics: args
                .matomo_url
                .zip(args.matomo_site_id)
                .map(|(url, site_id)| Analytics::new(url, site_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::DEFAULT_BIND;

    /// Returns the options of a minimal production setup.
    fn production_args() -> ServeArgs {
        ServeArgs {
            bind: DEFAULT_BIND,
            base_url: BaseUrl::parse("https://praxis.test").expect("URL should be valid"),
            matomo_url: None,
            matomo_site_id: None,
        }
    }

    /// The options end up in the configuration.
    #[test]
    fn production_uses_options() {
        let config = Config::production(production_args());

        assert_eq!(config.base_url.origin(), "https://praxis.test");
        assert!(config.bind.ip().is_loopback());
        assert!(config.analytics.is_none());
    }

    /// The URL and the site ID make up the analytics configuration.
    #[test]
    fn analytics_combine_url_and_site_id() {
        let args = ServeArgs {
            matomo_url: Some(MatomoUrl::parse("https://stats.test/").expect("URL should be valid")),
            matomo_site_id: Some(9),
            ..production_args()
        };

        assert!(Config::production(args).analytics.is_some());
    }
}
