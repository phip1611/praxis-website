// SPDX-License-Identifier: MIT OR Apache-2.0

//! The facts of the website from `content/site.toml`.
//!
//! All content that isn't editable at runtime is in that file, while the code
//! only defines its structure.

use crate::content::practice::{OpeningDay, Practice};
use serde::Deserialize;
use std::sync::LazyLock;

/// The text of `content/site.toml`.
const SITE_FILE: &str = include_str!("../../content/site.toml");

/// The parsed `content/site.toml`.
///
/// The file is compiled in and a test parses it, so parsing can't fail in a
/// released binary.
pub(crate) static SITE: LazyLock<Site> =
    LazyLock::new(|| toml::from_str(SITE_FILE).expect("content/site.toml should be valid"));

/// The content of `content/site.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Site {
    /// The contact data and location of the practice.
    pub(crate) practice: Practice,
    /// The regular opening hours.
    pub(crate) opening_hours: Vec<OpeningDay>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A broken file would otherwise only show up when the site starts.
    #[test]
    fn site_file_is_valid() {
        LazyLock::force(&SITE);
    }
}
