// SPDX-License-Identifier: MIT OR Apache-2.0

//! # praxis-website
//!
//! The website of the general practice Dipl.-Med. Dirk Schuster in
//! Thalheim/Erzgebirge.

mod cli;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod config;
mod content;
mod logging;

use crate::cli::{Cli, Command, ServeArgs};
use crate::config::Config;
use clap::Parser;
use std::process::ExitCode;

/// Parses the command line and checks the configuration.
fn main() -> ExitCode {
    let Command::Serve(args) = Cli::parse().command;
    serve(args)
}

/// Checks the configuration. Serving the site needs the web server, which a
/// later commit adds.
fn serve(args: ServeArgs) -> ExitCode {
    logging::init();
    Config::production(args);
    ExitCode::SUCCESS
}
