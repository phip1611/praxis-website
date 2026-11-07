// SPDX-License-Identifier: MIT OR Apache-2.0

//! Search engine metadata: structured data, sitemap, and `robots.txt`.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(super) mod json_ld;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(super) mod robots;
pub(super) mod sitemap;
