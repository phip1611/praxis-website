// SPDX-License-Identifier: MIT OR Apache-2.0

//! Precomputed HTTP responses.
//!
//! All public pages are rendered and compressed once, at startup, and are
//! then served from memory. Requests never run templates
//! or compression, which keeps them fast and their behavior predictable.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod assets;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod cached;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod compression;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod encoding;
#[expect(dead_code, reason = "used by code that a later commit adds")]
pub(crate) mod snapshot;
