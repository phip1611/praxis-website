// SPDX-License-Identifier: MIT OR Apache-2.0

//! The content of the website: practice data.

#[expect(dead_code, reason = "used by code that a later commit adds")]
pub(crate) mod practice;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod site;
