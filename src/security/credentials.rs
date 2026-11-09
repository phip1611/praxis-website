// SPDX-License-Identifier: MIT OR Apache-2.0

//! The accounts file and account lookup.
//!
//! The file has one `username:phc-hash` entry per line, similar to `htpasswd`.
//! Empty lines and lines starting with `#` are ignored. It is created with the
//! `hash-password` subcommand and is read-only for this service.

use crate::security::password::{PasswordError, PasswordHash};
use crate::security::token::constant_time_eq;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, LazyLock, RwLock};
use thiserror::Error;

/// The maximum length of a username in bytes.
pub(crate) const MAX_USERNAME_BYTES: usize = 64;

/// An Argon2id hash of a random, discarded password with the default
/// parameters, used to verify logins of unknown users.
///
/// Without this, a login with an unknown username would answer much faster,
/// which reveals which usernames exist.
const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$m1SD9StXnTyOseLIb5x+Cg$WokQDrCsnWEOhw71wqcCbrfapCMpJkgzHqUtpgAWpDI";

/// The parsed form of [`DUMMY_HASH`].
static DUMMY: LazyLock<PasswordHash> =
    LazyLock::new(|| PasswordHash::parse(DUMMY_HASH).expect("the dummy hash should be valid"));

/// A failure while loading the accounts file.
#[derive(Debug, Error)]
pub(crate) enum CredentialsError {
    /// The file can't be read.
    #[error("can't read accounts file {path}: {source}")]
    Read {
        /// The path of the file.
        path: String,
        /// The underlying error.
        source: io::Error,
    },
    /// A line isn't a `username:hash` entry.
    #[error("accounts file line {line}: expected 'username:hash'")]
    MalformedLine {
        /// The one-based line number.
        line: usize,
    },
    /// A username has an invalid form.
    #[error("accounts file line {line}: {reason}")]
    InvalidUsername {
        /// The one-based line number.
        line: usize,
        /// Why the username is invalid.
        reason: UsernameError,
    },
    /// A username occurs twice.
    #[error("accounts file line {line}: duplicate username")]
    DuplicateUsername {
        /// The one-based line number.
        line: usize,
    },
    /// A password hash can't be used.
    #[error("accounts file line {line}: {source}")]
    InvalidHash {
        /// The one-based line number.
        line: usize,
        /// The underlying error.
        source: PasswordError,
    },
    /// The file has no accounts, so nobody could log in.
    #[error("the accounts file doesn't contain any accounts")]
    Empty,
}

/// Why a username is invalid.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum UsernameError {
    /// The username is empty.
    #[error("the username is empty")]
    Empty,
    /// The username exceeds [`MAX_USERNAME_BYTES`].
    #[error("the username is too long")]
    TooLong,
    /// The username contains `:`, whitespace, or control characters.
    #[error("the username contains ':', whitespace, or control characters")]
    InvalidCharacter,
}

/// Checks that `username` can be stored in the accounts file and logged safely.
///
/// # Errors
///
/// Returns [`UsernameError`] with the violated rule.
pub(crate) fn validate_username(username: &str) -> Result<(), UsernameError> {
    if username.is_empty() {
        return Err(UsernameError::Empty);
    }
    if username.len() > MAX_USERNAME_BYTES {
        return Err(UsernameError::TooLong);
    }
    if username
        .chars()
        .any(|c| c == ':' || c.is_whitespace() || c.is_control())
    {
        return Err(UsernameError::InvalidCharacter);
    }
    Ok(())
}

/// One configured account.
#[derive(Debug)]
struct Account {
    /// The login name.
    username: String,
    /// The hash from the configuration, as PHC string. A password changed in
    /// the login area replaces only [`Self::password_hash`], and records which
    /// configured hash it replaced, see `password_overrides`.
    configured_hash: String,
    /// The current hash, from the configuration or a password change.
    password_hash: Arc<PasswordHash>,
}

/// All accounts that may log in. The set of accounts is fixed by the
/// configuration; only their passwords can change at runtime.
pub(crate) struct CredentialStore {
    /// The accounts in file order.
    accounts: RwLock<Vec<Account>>,
}

impl CredentialStore {
    /// Loads the accounts file from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError`] if the file can't be read or is invalid.
    pub(crate) fn from_file(path: &Path) -> Result<Self, CredentialsError> {
        let content = fs::read_to_string(path).map_err(|source| CredentialsError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Self::parse(&content)
    }

    /// Parses the content of an accounts file.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError`] for the first invalid line, or if there
    /// are no accounts at all.
    pub(crate) fn parse(content: &str) -> Result<Self, CredentialsError> {
        let mut accounts: Vec<Account> = Vec::new();
        for (index, raw_line) in content.lines().enumerate() {
            let line = index + 1;
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (username, hash) = trimmed
                .split_once(':')
                .ok_or(CredentialsError::MalformedLine { line })?;
            validate_username(username)
                .map_err(|reason| CredentialsError::InvalidUsername { line, reason })?;
            if accounts.iter().any(|account| account.username == username) {
                return Err(CredentialsError::DuplicateUsername { line });
            }
            let password_hash = PasswordHash::parse(hash)
                .map_err(|source| CredentialsError::InvalidHash { line, source })?;
            accounts.push(Account {
                username: username.to_string(),
                configured_hash: hash.to_string(),
                password_hash: Arc::new(password_hash),
            });
        }
        if accounts.is_empty() {
            return Err(CredentialsError::Empty);
        }
        Ok(Self {
            accounts: RwLock::new(accounts),
        })
    }

    /// Creates a store with a single account.
    pub(crate) fn single(username: &str, password_hash: PasswordHash) -> Self {
        Self {
            accounts: RwLock::new(vec![Account {
                username: username.to_string(),
                configured_hash: password_hash.to_phc_string(),
                password_hash: Arc::new(password_hash),
            }]),
        }
    }

    /// Returns the number of accounts.
    pub(crate) fn len(&self) -> usize {
        self.accounts.read().unwrap().len()
    }

    /// Returns the canonical name of the account `username`, if there is one.
    pub(super) fn find(&self, username: &str) -> Option<String> {
        let accounts = self.accounts.read().unwrap();
        Self::position(&accounts, username).map(|index| accounts[index].username.clone())
    }

    /// Returns the configured hash of the account `username`.
    pub(super) fn configured_hash(&self, username: &str) -> Option<String> {
        let accounts = self.accounts.read().unwrap();
        Self::position(&accounts, username).map(|index| accounts[index].configured_hash.clone())
    }

    /// Replaces the current password hash of the account `username`. Returns
    /// `false` if there is no such account.
    pub(super) fn set_password(&self, username: &str, password_hash: PasswordHash) -> bool {
        let mut accounts = self.accounts.write().unwrap();
        let Some(index) = Self::position(&accounts, username) else {
            return false;
        };
        accounts[index].password_hash = Arc::new(password_hash);
        drop(accounts);
        true
    }

    /// Verifies a login and returns the canonical account name on success.
    ///
    /// Unknown users are checked against a dummy hash, so that both cases take
    /// the same time. This is CPU-intensive: don't call it on the async runtime.
    pub(crate) fn verify(&self, username: &str, password: &str) -> Option<String> {
        let account = {
            let accounts = self.accounts.read().unwrap();
            Self::position(&accounts, username).map(|index| {
                let account = &accounts[index];
                (account.username.clone(), Arc::clone(&account.password_hash))
            })
        };
        match account {
            Some((username, hash)) => hash.verify(password).then_some(username),
            None => {
                let _ = DUMMY.verify(password);
                None
            }
        }
    }

    /// Returns the index of the account `username`, comparing all accounts
    /// without an early exit, so that the duration doesn't reveal a match.
    fn position(accounts: &[Account], username: &str) -> Option<usize> {
        let mut found = None;
        for (index, account) in accounts.iter().enumerate() {
            if constant_time_eq(account.username.as_bytes(), username.as_bytes()) {
                found = Some(index);
            }
        }
        found
    }
}

impl Debug for CredentialStore {
    /// Prints only the number of accounts.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialStore")
            .field("accounts", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TEST_PASSWORD, TEST_USER, accounts_file, cheap_password_hash};
    use argon2::Params;
    use std::assert_matches;
    use tempfile::NamedTempFile;

    /// Valid entries are found, comments and empty lines are skipped.
    #[test]
    fn parse_reads_accounts_and_skips_comments() {
        let content = format!("# comment\n\n{}\n", accounts_file(&[(TEST_USER, "pw")]));

        let store = CredentialStore::parse(&content).expect("file should be valid");

        assert_eq!(store.len(), 1);
        assert_eq!(store.find(TEST_USER).as_deref(), Some(TEST_USER));
    }

    /// Each kind of broken line produces its own error with the line number.
    #[test]
    fn parse_reports_invalid_lines() {
        let valid = accounts_file(&[(TEST_USER, "pw")]);
        let duplicate = format!("{valid}\n{valid}");
        let bad_name = format!("bad name:{}", valid.split_once(':').expect("has colon").1);

        assert_matches!(
            CredentialStore::parse("nohash"),
            Err(CredentialsError::MalformedLine { line: 1 })
        );
        assert_matches!(
            CredentialStore::parse("a:b"),
            Err(CredentialsError::InvalidHash { line: 1, .. })
        );
        assert_matches!(
            CredentialStore::parse(&duplicate),
            Err(CredentialsError::DuplicateUsername { line: 2 })
        );
        assert_matches!(
            CredentialStore::parse(&bad_name),
            Err(CredentialsError::InvalidUsername { line: 1, .. })
        );
    }

    /// A file without accounts is a configuration error, not an empty login.
    #[test]
    fn parse_rejects_file_without_accounts() {
        assert_matches!(
            CredentialStore::parse("# only\n"),
            Err(CredentialsError::Empty)
        );
    }

    /// The right password logs in and returns the canonical account name.
    #[test]
    fn verify_accepts_correct_password() {
        let store = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("file should be valid");

        assert_eq!(
            store.verify(TEST_USER, TEST_PASSWORD).as_deref(),
            Some(TEST_USER)
        );
    }

    /// Wrong passwords and unknown users fail in the same way.
    #[test]
    fn verify_rejects_wrong_password_and_unknown_user() {
        let store = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("file should be valid");

        assert!(store.verify(TEST_USER, "wrong").is_none());
        assert!(store.verify("unknown", TEST_PASSWORD).is_none());
    }

    /// Usernames are case-sensitive, so there is exactly one form per account.
    #[test]
    fn find_is_case_sensitive() {
        let store = CredentialStore::parse(&accounts_file(&[("dirk", "pw")]))
            .expect("file should be valid");

        assert!(store.find("Dirk").is_none());
    }

    /// The timing protection only works if the dummy hash costs the same as
    /// real hashes, so it must follow the defaults of the `argon2` crate.
    #[test]
    fn dummy_hash_uses_default_parameters() {
        let defaults = Params::default();
        let expected = format!(
            "$argon2id$v=19$m={},t={},p={}$",
            defaults.m_cost(),
            defaults.t_cost(),
            defaults.p_cost()
        );

        assert!(DUMMY_HASH.starts_with(&expected));
    }

    /// Usernames with characters that break the file format are rejected.
    #[test]
    fn validate_username_enforces_rules() {
        assert_eq!(validate_username("dirk.schuster"), Ok(()));
        assert_eq!(validate_username(""), Err(UsernameError::Empty));
        assert_eq!(
            validate_username("a:b"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            validate_username("a b"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            validate_username("a\nb"),
            Err(UsernameError::InvalidCharacter)
        );
        assert_eq!(
            validate_username(&"x".repeat(65)),
            Err(UsernameError::TooLong)
        );
    }

    /// Loading works from a real file, and missing files name the path.
    #[test]
    fn from_file_reads_file_and_reports_missing_path() {
        let file = NamedTempFile::new().expect("temp file should be created");
        fs::write(file.path(), accounts_file(&[(TEST_USER, "pw")]))
            .expect("temp file should be writable");

        let store = CredentialStore::from_file(file.path()).expect("file should be valid");
        let missing = CredentialStore::from_file(Path::new("/does/not/exist"));

        assert_eq!(store.len(), 1);
        let Err(CredentialsError::Read { path, .. }) = missing else {
            panic!("the value should match `Err(CredentialsError::Read {{ path, .. }})`");
        };
        assert_eq!(path, "/does/not/exist");
    }

    /// A changed password replaces the old one, and the configured hash stays
    /// known for later comparison.
    #[test]
    fn set_password_replaces_current_hash() {
        let store = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("file should be valid");
        let configured = store
            .configured_hash(TEST_USER)
            .expect("account should exist");
        let new = PasswordHash::parse(&cheap_password_hash("a new long password"))
            .expect("fixture should be valid");

        assert!(store.set_password(TEST_USER, new));

        assert!(store.verify(TEST_USER, "a new long password").is_some());
        assert!(store.verify(TEST_USER, TEST_PASSWORD).is_none());
        assert_eq!(store.configured_hash(TEST_USER), Some(configured));
    }

    /// Only configured accounts can get a password.
    #[test]
    fn set_password_rejects_unknown_account() {
        let store = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("file should be valid");
        let hash = PasswordHash::parse(&cheap_password_hash("x")).expect("fixture should be valid");

        assert!(!store.set_password("intruder", hash));
        assert!(store.find("intruder").is_none());
    }

    /// Password hashes must not end up in logs through `{:?}`.
    #[test]
    fn debug_output_hides_accounts() {
        let store = CredentialStore::parse(&accounts_file(&[(TEST_USER, "pw")]))
            .expect("file should be valid");

        let output = format!("{store:?}");

        assert!(!output.contains("argon2id"));
        assert!(!output.contains(TEST_USER));
    }
}
