// SPDX-License-Identifier: MIT OR Apache-2.0

//! The validated runtime configuration.

mod analytics;
mod base_url;

pub(crate) use analytics::{Analytics, MatomoUrl};
pub(crate) use base_url::BaseUrl;

use crate::cli::{DevArgs, ServeArgs};
use std::net::SocketAddr;
use std::path::PathBuf;

/// The state directory of the development mode, relative to the working
/// directory. Ignored by Git.
const DEV_STATE_DIR: &str = ".dev-state";

/// Where the accounts come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AccountsSource {
    /// An accounts file, see [`crate::security::credentials`].
    File(PathBuf),
    /// The fixed account of the development mode.
    DevAccount,
}

/// The validated configuration of the service.
#[derive(Clone, Debug)]
pub(crate) struct Config {
    /// Whether the local development mode is active.
    pub(crate) dev: bool,
    /// The address to listen on.
    pub(crate) bind: SocketAddr,
    /// The public URL of the site, as seen by visitors.
    pub(crate) base_url: BaseUrl,
    /// The directory for persistent state.
    pub(crate) state_dir: PathBuf,
    /// Where the accounts come from.
    pub(crate) accounts: AccountsSource,
    /// The Matomo configuration, if analytics are enabled.
    pub(crate) analytics: Option<Analytics>,
    /// The origins that may send state-changing requests.
    pub(crate) allowed_origins: Vec<String>,
}

impl Config {
    /// Creates the configuration of the server from its options, which clap
    /// has already validated.
    pub(crate) fn production(args: ServeArgs) -> Self {
        let allowed_origins = vec![args.base_url.origin().to_string()];

        Self {
            dev: false,
            bind: args.bind,
            base_url: args.base_url,
            state_dir: args.state_dir,
            accounts: AccountsSource::File(args.accounts_file),
            analytics: args
                .matomo_url
                .zip(args.matomo_site_id)
                .map(|(url, site_id)| Analytics::new(url, site_id)),
            allowed_origins,
        }
    }

    /// Creates the configuration of the development server.
    pub(crate) fn dev(args: DevArgs) -> Self {
        let base_url = BaseUrl::parse(&format!("http://localhost:{}", args.bind.port()))
            .expect("the dev URL should be valid");
        // Browsers use the host name of the URL bar, which may be either.
        let allowed_origins = vec![
            base_url.origin().to_string(),
            format!("http://127.0.0.1:{}", args.bind.port()),
        ];

        Self {
            dev: true,
            bind: args.bind,
            base_url,
            state_dir: PathBuf::from(DEV_STATE_DIR),
            accounts: AccountsSource::DevAccount,
            analytics: None,
            allowed_origins,
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
            state_dir: PathBuf::from("/var/lib/praxis"),
            accounts_file: PathBuf::from("/etc/accounts"),
            matomo_url: None,
            matomo_site_id: None,
        }
    }

    /// The options end up in the configuration.
    #[test]
    fn production_uses_options() {
        let config = Config::production(production_args());

        assert!(!config.dev);
        assert_eq!(config.base_url.origin(), "https://praxis.test");
        assert!(config.bind.ip().is_loopback());
        assert_eq!(config.state_dir, PathBuf::from("/var/lib/praxis"));
        assert_eq!(
            config.accounts,
            AccountsSource::File(PathBuf::from("/etc/accounts"))
        );
        assert_eq!(config.allowed_origins, ["https://praxis.test"]);
        assert!(config.analytics.is_none());
    }

    /// `cargo dev` works without any options.
    #[test]
    fn dev_mode_needs_no_options() {
        let config = Config::dev(DevArgs { bind: DEFAULT_BIND });

        assert!(config.dev);
        assert_eq!(config.base_url.origin(), "http://localhost:8080");
        assert_eq!(config.state_dir, PathBuf::from(DEV_STATE_DIR));
        assert_eq!(config.accounts, AccountsSource::DevAccount);
        assert!(
            config
                .allowed_origins
                .contains(&"http://127.0.0.1:8080".to_string())
        );
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
