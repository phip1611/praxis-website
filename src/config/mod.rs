// SPDX-License-Identifier: MIT OR Apache-2.0

//! The validated runtime configuration.

mod base_url;

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
}

impl Config {
    /// Creates the configuration of the server from its options, which clap
    /// has already validated.
    pub(crate) fn production(args: ServeArgs) -> Self {
        Self {
            bind: args.bind,
            base_url: args.base_url,
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
        }
    }

    /// The options end up in the configuration.
    #[test]
    fn production_uses_options() {
        let config = Config::production(production_args());

        assert_eq!(config.base_url.origin(), "https://praxis.test");
        assert!(config.bind.ip().is_loopback());
    }
}
