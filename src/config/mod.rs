// SPDX-License-Identifier: MIT OR Apache-2.0

//! The validated runtime configuration.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod base_url;
