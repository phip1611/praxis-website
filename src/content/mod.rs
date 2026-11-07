// SPDX-License-Identifier: MIT OR Apache-2.0

//! The content of the website: practice data, pages, and the editable news.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod news;
pub(crate) mod pages;
pub(crate) mod practice;
pub(crate) mod site;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod store;
