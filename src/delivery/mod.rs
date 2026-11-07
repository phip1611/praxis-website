// SPDX-License-Identifier: MIT OR Apache-2.0

//! Precomputed HTTP responses.
//!
//! All public pages are rendered and compressed once, at startup, and are
//! then served from memory. Requests never run templates
//! or compression, which keeps them fast and their behavior predictable.

pub(crate) mod assets;
pub(crate) mod cached;
mod compression;
mod encoding;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod snapshot;
