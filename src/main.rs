// SPDX-License-Identifier: MIT OR Apache-2.0

//! # praxis-website
//!
//! The website of the general practice Dipl.-Med. Dirk Schuster in
//! Thalheim/Erzgebirge.

mod cli;
mod logging;

use crate::cli::Cli;
use clap::Parser;

/// Parses the command line and sets up logging.
fn main() {
    let _cli = Cli::parse();
    logging::init();
}
