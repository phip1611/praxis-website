// SPDX-License-Identifier: MIT OR Apache-2.0

//! # praxis-website
//!
//! The website of the general practice Dipl.-Med. Dirk Schuster in
//! Thalheim/Erzgebirge.
//!
//! The service speaks plain HTTP and runs behind a reverse proxy that
//! terminates TLS. All public pages are rendered and compressed into memory
//! at startup, see the `delivery` module. Security-relevant
//! code lives in the `security` module.

mod app;
mod cli;
mod clock;
mod config;
mod content;
mod delivery;
mod hex;
mod logging;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod persist;
mod render;
mod security;
mod server;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod stats;
#[cfg(test)]
mod test_support;
mod web;

use crate::cli::{Cli, Command};
use crate::config::Config;
use clap::Parser;
use std::process::ExitCode;
use tokio::runtime::Builder;
use tracing::error;

/// Parses the command line and runs the server.
fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Serve(args) => serve(Config::production(args)),
        Command::Dev(args) => serve(Config::dev(args)),
    }
}

/// Runs the web server until it's stopped.
fn serve(config: Config) -> ExitCode {
    logging::init(config.dev);
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("the async runtime should start");
    match runtime.block_on(server::run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}
