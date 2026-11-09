// SPDX-License-Identifier: MIT OR Apache-2.0

//! Logging to stdout. The level can be changed with `RUST_LOG`, e.g.,
//! `RUST_LOG=debug`.
//!
//! Client IP addresses and user agents are never logged.

use std::env;
use std::io::{self, IsTerminal};
use tracing_subscriber::EnvFilter;

/// The default filter.
const DEFAULT_FILTER: &str = "info";

/// Installs the global logger.
///
/// Must be called once, before any log output.
pub(crate) fn init() {
    // Unlike `try_from_default_env`, reports invalid directives on stderr
    // instead of silently using the default.
    let directives =
        env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| DEFAULT_FILTER.to_string());
    let filter = EnvFilter::builder().parse_lossy(directives);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(io::stdout().is_terminal())
        .with_target(false)
        .init();
}
