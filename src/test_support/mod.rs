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
use argon2::password_hash::PasswordHasher;
use argon2::{Algorithm, Argon2, Params, Version};
use std::path::PathBuf;

/// The account of the test fixtures.
pub(crate) const TEST_USER: &str = "dirk";

/// The password of [`TEST_USER`].
pub(crate) const TEST_PASSWORD: &str = "correct horse battery staple";

/// A text that only the seed news contains. Tests use it to tell whether
/// the start page still shows the original news.
pub(crate) const SEED_MARKER: &str = "Vom 19.11. bis 20.11.2026";

/// The public URL of the test site.
const TEST_BASE_URL: &str = "https://praxis.test";

/// Returns an Argon2id hash of `password` with the lowest possible cost.
///
/// Production parameters take tens of milliseconds per hash, which adds up
/// over many tests. Verification reads the parameters from the hash, so the
/// tested code paths are the same.
pub(crate) fn cheap_password_hash(password: &str) -> String {
    let params = Params::new(
        Params::MIN_M_COST,
        Params::MIN_T_COST,
        Params::MIN_P_COST,
        None,
    )
    .expect("the minimum parameters should be valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes())
        .expect("hashing should succeed")
        .to_string()
}

/// Returns the content of an accounts file with cheap hashes.
pub(crate) fn accounts_file(accounts: &[(&str, &str)]) -> String {
    accounts
        .iter()
        .map(|(username, password)| format!("{username}:{}", cheap_password_hash(password)))
        .collect::<Vec<_>>()
        .join("\n")
}

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
