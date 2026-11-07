// SPDX-License-Identifier: MIT OR Apache-2.0

//! # praxis-website
//!
//! The website of the general practice Dipl.-Med. Dirk Schuster in
//! Thalheim/Erzgebirge.

mod cli;

use crate::cli::Cli;
use clap::Parser;

/// Parses the command line.
fn main() {
    let _cli = Cli::parse();
}
