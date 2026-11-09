// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fixtures and mocks for the unit tests.
//!
//! They keep the tests short: a test creates a [`TestApp`] or a mock and then
//! only contains the interesting steps and checks.

mod app;
mod clock;

pub(crate) use app::TestApp;
pub(crate) use clock::MockClock;

use crate::cli::{DEFAULT_BIND, ServeArgs};
use crate::config::{BaseUrl, Config};
use std::path::PathBuf;

/// A text that only the seed news contains. Tests use it to tell whether
/// the start page still shows the original news.
pub(crate) const SEED_MARKER: &str = "Vom 19.11. bis 20.11.2026";

/// The public URL of the test site.
const TEST_BASE_URL: &str = "https://praxis.test";

/// Returns a production configuration for the test site.
pub(crate) fn test_config() -> Config {
    Config::production(ServeArgs {
        bind: DEFAULT_BIND,
        base_url: BaseUrl::parse(TEST_BASE_URL).expect("the test URL should be valid"),
        state_dir: PathBuf::from("/nonexistent/state"),
        matomo_url: None,
        matomo_site_id: None,
    })
}
