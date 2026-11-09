// SPDX-License-Identifier: MIT OR Apache-2.0

//! Passwords changed in the login area.
//!
//! The accounts come from the read-only configuration. A changed password is
//! stored as hash in the state directory and takes precedence over the
//! configured one. Each stored hash records a digest of the configured hash
//! it replaced: if the operator configures a new hash, e.g., because someone
//! forgot their password, the stored one no longer applies.
//!
//! The service can only change passwords of configured accounts, never add
//! accounts.

use crate::hex;
use crate::persist::{self, StoreError};
use crate::security::credentials::CredentialStore;
use crate::security::password::PasswordHash;
use crate::security::token::digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};

/// The name of the file in the state directory.
const FILE_NAME: &str = "passwoerter.json";

/// A changed password of one account.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Override {
    /// The new hash as PHC string.
    hash: String,
    /// The hex SHA-256 of the configured hash that this one replaces.
    replaces: String,
}

/// The changed passwords and their file.
#[derive(Debug)]
pub(crate) struct PasswordOverrides {
    /// The file in the state directory.
    file: PathBuf,
    /// The overrides by username. The lock also serializes writes.
    overrides: Mutex<BTreeMap<String, Override>>,
}

impl PasswordOverrides {
    /// Loads the changed passwords and applies the valid ones to
    /// `credentials`.
    ///
    /// Outdated or broken entries are logged and ignored, so that the
    /// configured password applies.
    pub(crate) fn load(state_dir: &Path, credentials: &CredentialStore) -> Self {
        let file = state_dir.join(FILE_NAME);
        let overrides: BTreeMap<String, Override> = persist::read_json(&file)
            .unwrap_or_else(|err| {
                warn!("ignoring changed passwords: {err}");
                None
            })
            .unwrap_or_default();
        for (username, entry) in &overrides {
            let current = credentials
                .configured_hash(username)
                .is_some_and(|configured| fingerprint(&configured) == entry.replaces);
            match PasswordHash::parse(&entry.hash) {
                Ok(hash) if current => {
                    credentials.set_password(username, hash);
                    info!(
                        account = username,
                        "using the password changed in the login area"
                    );
                }
                Ok(_) => {
                    warn!(
                        account = username,
                        "ignoring a changed password, as the configured one changed"
                    );
                }
                Err(err) => warn!(account = username, "ignoring a changed password: {err}"),
            }
        }
        Self {
            file,
            overrides: Mutex::new(overrides),
        }
    }

    /// Stores `hash` as new password of `username`, replacing `configured`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the file can't be written. Nothing changes
    /// then.
    pub(super) fn save(
        &self,
        username: &str,
        hash: &PasswordHash,
        configured: &str,
    ) -> Result<(), StoreError> {
        let mut overrides = self.overrides.lock().unwrap();
        let mut updated = overrides.clone();
        updated.insert(
            username.to_string(),
            Override {
                hash: hash.to_phc_string(),
                replaces: fingerprint(configured),
            },
        );
        persist::write_json(&self.file, &updated)?;
        *overrides = updated;
        drop(overrides);
        Ok(())
    }
}

/// Returns the hex SHA-256 of a configured hash.
fn fingerprint(configured: &str) -> String {
    hex::encode(&digest(configured))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TEST_PASSWORD, TEST_USER, accounts_file, cheap_password_hash};
    use std::fs;
    use tempfile::TempDir;

    /// The new password of the tests.
    const NEW_PASSWORD: &str = "a completely new password";

    /// The service's state at one start: the state directory, the accounts
    /// file, and what the service loaded from them.
    struct Start {
        /// The state directory, kept across restarts.
        dir: TempDir,
        /// The content of the accounts file.
        accounts: String,
        /// The loaded accounts.
        credentials: CredentialStore,
        /// The loaded changed passwords.
        overrides: PasswordOverrides,
    }

    /// Starts with the test account and an empty state directory.
    fn start() -> Start {
        let dir = TempDir::new().expect("temp dir should be created");
        let accounts = accounts_file(&[(TEST_USER, TEST_PASSWORD)]);
        let credentials = CredentialStore::parse(&accounts).expect("fixture should be valid");
        let overrides = PasswordOverrides::load(dir.path(), &credentials);
        Start {
            dir,
            accounts,
            credentials,
            overrides,
        }
    }

    /// Loads the accounts file `accounts` again, as after a restart.
    fn restart(dir: &TempDir, accounts: &str) -> CredentialStore {
        let credentials = CredentialStore::parse(accounts).expect("fixture should be valid");
        PasswordOverrides::load(dir.path(), &credentials);
        credentials
    }

    /// Stores a new password for the test account.
    fn change_password(credentials: &CredentialStore, overrides: &PasswordOverrides) {
        let hash = PasswordHash::parse(&cheap_password_hash(NEW_PASSWORD)).expect("valid fixture");
        let configured = credentials
            .configured_hash(TEST_USER)
            .expect("account exists");
        overrides
            .save(TEST_USER, &hash, &configured)
            .expect("save should succeed");
    }

    /// A changed password applies after a restart.
    #[test]
    fn saved_password_applies_after_restart() {
        let first = start();
        change_password(&first.credentials, &first.overrides);

        let restarted = restart(&first.dir, &first.accounts);

        assert!(restarted.verify(TEST_USER, NEW_PASSWORD).is_some());
        assert!(restarted.verify(TEST_USER, TEST_PASSWORD).is_none());
    }

    /// If the operator configures a new hash, the changed password no longer
    /// applies. This is how a forgotten password is reset.
    #[test]
    fn new_configured_hash_wins_over_saved_password() {
        let first = start();
        change_password(&first.credentials, &first.overrides);

        let reset = restart(&first.dir, &accounts_file(&[(TEST_USER, "reset by admin")]));

        assert!(reset.verify(TEST_USER, "reset by admin").is_some());
        assert!(reset.verify(TEST_USER, NEW_PASSWORD).is_none());
    }

    /// Entries for accounts that aren't configured don't create accounts.
    #[test]
    fn saved_password_cannot_add_accounts() {
        let first = start();
        let entry = Override {
            hash: cheap_password_hash(NEW_PASSWORD),
            replaces: String::new(),
        };
        persist::write_json(
            &first.dir.path().join(FILE_NAME),
            &BTreeMap::from([("intruder".to_string(), entry)]),
        )
        .expect("write should succeed");

        let restarted = restart(&first.dir, &first.accounts);

        assert!(restarted.find("intruder").is_none());
    }

    /// A damaged file doesn't prevent the start; the configured passwords
    /// apply.
    #[test]
    fn corrupt_file_is_ignored() {
        let first = start();
        fs::write(first.dir.path().join(FILE_NAME), "{ broken").expect("write should succeed");

        let restarted = restart(&first.dir, &first.accounts);

        assert!(restarted.verify(TEST_USER, TEST_PASSWORD).is_some());
    }
}
