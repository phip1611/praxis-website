// SPDX-License-Identifier: MIT OR Apache-2.0

//! The content of the website: practice data, pages, and the editable news.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod news;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod pages;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod practice;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod site;
