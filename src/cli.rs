// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command-line interface.
//!
//! Every option can also be set with an environment variable, which suits
//! systemd units. `STATE_DIRECTORY` is set by systemd's `StateDirectory=`.

use crate::config::{BaseUrl, MatomoUrl};
use clap::{Args, Parser, Subcommand};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// The default listen address. Only reachable locally, as the service runs
/// behind a reverse proxy.
pub(crate) const DEFAULT_BIND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);

/// Website of the general practice Dipl.-Med. Dirk Schuster.
#[derive(Debug, Parser)]
#[command(version, about)]
pub(crate) struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The commands of the binary.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Serves the website.
    Serve(ServeArgs),
    /// Local development: no configuration needed, and the page reloads
    /// after code changes.
    Dev(DevArgs),
    /// Reads a password and prints an entry for the accounts file.
    ///
    /// Example: `praxis-website hash-password dirk >> accounts`
    HashPassword {
        /// The login name of the account.
        username: String,
    },
}

/// The options of the server.
#[derive(Clone, Debug, Args)]
pub(crate) struct ServeArgs {
    /// The address to listen on.
    #[arg(long, env = "PRAXIS_BIND", default_value_t = DEFAULT_BIND)]
    pub(crate) bind: SocketAddr,

    /// The public URL of the site, e.g., `https://praxis-dirkschuster.de`.
    #[arg(long, env = "PRAXIS_BASE_URL", value_parser = BaseUrl::parse)]
    pub(crate) base_url: BaseUrl,

    /// The directory for the access statistics.
    #[arg(long, env = "STATE_DIRECTORY")]
    pub(crate) state_dir: PathBuf,

    /// The URL of the Matomo installation, e.g., `https://stats.example.com/`.
    #[arg(
        long,
        env = "PRAXIS_MATOMO_URL",
        value_parser = MatomoUrl::parse,
        requires = "matomo_site_id"
    )]
    pub(crate) matomo_url: Option<MatomoUrl>,

    /// The ID of this site in Matomo.
    #[arg(long, env = "PRAXIS_MATOMO_SITE_ID", requires = "matomo_url")]
    pub(crate) matomo_site_id: Option<u32>,
}

/// The options of the development server.
#[derive(Clone, Copy, Debug, Args)]
pub(crate) struct DevArgs {
    /// The address to listen on.
    #[arg(long, env = "PRAXIS_BIND", default_value_t = DEFAULT_BIND)]
    pub(crate) bind: SocketAddr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    /// The minimal command line of the server.
    const SERVE: &[&str] = &[
        "praxis-website",
        "serve",
        "--base-url",
        "https://praxis.test",
        "--state-dir",
        "/state",
    ];

    /// The clap definition is consistent (no conflicting names etc.).
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// Returns the options of the `serve` command line `args`.
    fn serve_args(args: &[&str]) -> ServeArgs {
        let command = Cli::try_parse_from(args).expect("should parse").command;
        let Command::Serve(args) = command else {
            panic!("the value should match `Command::Serve(args)`");
        };
        args
    }

    /// Options that aren't required have defaults.
    #[test]
    fn parses_defaults() {
        let args = serve_args(SERVE);

        assert_eq!(args.bind, DEFAULT_BIND);
    }

    /// The development server needs no options.
    #[test]
    fn parses_dev() {
        let cli = Cli::try_parse_from(["praxis-website", "dev"]).expect("should parse");

        let Command::Dev(args) = cli.command else {
            panic!("the value should match `Command::Dev(args)`");
        };
        assert_eq!(args.bind, DEFAULT_BIND);
    }

    /// The subcommand takes the username.
    #[test]
    fn parses_hash_password() {
        let cli =
            Cli::try_parse_from(["praxis-website", "hash-password", "dirk"]).expect("should parse");

        let Command::HashPassword { username } = cli.command else {
            panic!("the value should match `Command::HashPassword {{ username }}`");
        };
        assert_eq!(username, "dirk");
    }

    /// Returns the kind of the error that parsing `args` fails with.
    fn parse_error(args: &[&str]) -> ErrorKind {
        Cli::try_parse_from(args)
            .expect_err("parsing should fail")
            .kind()
    }

    /// Invalid URLs stop the start.
    #[test]
    fn rejects_invalid_base_url() {
        assert_eq!(
            parse_error(&["praxis-website", "serve", "--base-url", "praxis.test"]),
            ErrorKind::ValueValidation
        );
    }

    /// The server refuses to start without its settings.
    #[test]
    fn serve_requires_explicit_settings() {
        assert_eq!(
            parse_error(&["praxis-website", "serve", "--state-dir", "/state"]),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse_error(&[
                "praxis-website",
                "serve",
                "--base-url",
                "https://praxis.test"
            ]),
            ErrorKind::MissingRequiredArgument
        );
    }

    /// Analytics need both the URL and the site ID.
    #[test]
    fn analytics_options_come_in_pairs() {
        assert_eq!(
            parse_error(&[SERVE, &["--matomo-url", "https://stats.test/"]].concat()),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse_error(&[SERVE, &["--matomo-site-id", "9"]].concat()),
            ErrorKind::MissingRequiredArgument
        );
    }
}
