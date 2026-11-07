// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command-line interface.
//!
//! Every option can also be set with an environment variable, which suits
//! systemd units.

use clap::{Args, Parser, Subcommand};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
}

/// The options of the server.
#[derive(Clone, Debug, Args)]
pub(crate) struct ServeArgs {
    /// The address to listen on.
    #[arg(long, env = "PRAXIS_BIND", default_value_t = DEFAULT_BIND)]
    pub(crate) bind: SocketAddr,

    /// The public URL of the site, e.g., `https://praxis-dirkschuster.de`.
    #[arg(long, env = "PRAXIS_BASE_URL")]
    pub(crate) base_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The minimal command line of the server.
    const SERVE: &[&str] = &[
        "praxis-website",
        "serve",
        "--base-url",
        "https://praxis.test",
    ];

    /// The clap definition is consistent (no conflicting names etc.).
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// Returns the options of the `serve` command line `args`.
    fn serve_args(args: &[&str]) -> ServeArgs {
        let Command::Serve(args) = Cli::try_parse_from(args).expect("should parse").command;
        args
    }

    /// Options that aren't required have defaults.
    #[test]
    fn parses_defaults() {
        let args = serve_args(SERVE);

        assert_eq!(args.bind, DEFAULT_BIND);
    }
}
