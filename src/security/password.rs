// SPDX-License-Identifier: MIT OR Apache-2.0

//! Argon2id password hashing and verification.
//!
//! Verification takes the parameters from the stored PHC string, so hashes
//! stay valid when the defaults of the `argon2` crate change.

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash as PhcString;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use std::fmt::{self, Debug, Formatter};
use thiserror::Error;

/// The only accepted algorithm. Argon2id resists both GPU and side-channel
/// attacks. Other PHC strings are rejected, to not silently accept weak hashes.
const ALGORITHM: &str = "argon2id";

/// The minimum length of new passwords, in characters.
///
/// Follows the guidance for accounts without a second factor in
/// <https://pages.nist.gov/800-63-4/sp800-63b.html#passwordver>.
pub(crate) const MIN_PASSWORD_CHARS: usize = 15;

/// The maximum accepted password length in bytes.
///
/// This bounds the work per request. Anything longer can't be a password from
/// the accounts file in practice.
pub(crate) const MAX_PASSWORD_BYTES: usize = 1024;

/// A failure while processing a password or its hash.
#[derive(Debug, Error)]
pub(crate) enum PasswordError {
    /// The string isn't a valid PHC string.
    #[error("invalid password hash: {0}")]
    InvalidHash(String),
    /// The PHC string uses another algorithm than Argon2id.
    #[error("unsupported password hash algorithm: {0}")]
    UnsupportedAlgorithm(String),
    /// The new password is too short.
    #[error("the password must have at least {MIN_PASSWORD_CHARS} characters")]
    TooShort,
    /// The new password is too long.
    #[error("the password must have at most {MAX_PASSWORD_BYTES} bytes")]
    TooLong,
    /// Computing the hash failed.
    #[error("hashing failed: {0}")]
    Hashing(String),
}

/// A validated Argon2id password hash.
///
/// The [`Debug`] implementation hides the value, as hashes allow offline
/// guessing attacks.
pub(crate) struct PasswordHash(PhcString);

impl PasswordHash {
    /// Parses and validates a PHC string (`$argon2id$v=19$m=...`).
    ///
    /// # Errors
    ///
    /// Returns [`PasswordError`] if the string is malformed or if it isn't an
    /// Argon2id hash.
    pub(super) fn parse(phc: &str) -> Result<Self, PasswordError> {
        let parsed =
            PhcString::new(phc).map_err(|err| PasswordError::InvalidHash(err.to_string()))?;
        let algorithm = parsed.algorithm.as_str();
        if algorithm != ALGORITHM {
            return Err(PasswordError::UnsupportedAlgorithm(algorithm.to_string()));
        }
        Ok(Self(parsed))
    }

    /// Hashes a new password with a random salt and the default parameters.
    ///
    /// # Errors
    ///
    /// Returns [`PasswordError`] if the password violates the length limits or
    /// if hashing fails.
    pub(crate) fn create(password: &str) -> Result<Self, PasswordError> {
        validate_new(password)?;
        Self::create_unchecked(password)
    }

    /// Hashes a password without the minimum length check.
    ///
    /// Only for the fixed account of the local development mode.
    ///
    /// # Errors
    ///
    /// Returns [`PasswordError`] if the password is too long or hashing fails.
    pub(crate) fn create_unchecked(password: &str) -> Result<Self, PasswordError> {
        if password.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordError::TooLong);
        }
        let hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|err| PasswordError::Hashing(err.to_string()))?;
        Ok(Self(hash))
    }

    /// Checks `password` against the hash.
    ///
    /// The comparison of the derived key inside `argon2` is constant-time.
    pub(super) fn verify(&self, password: &str) -> bool {
        password.len() <= MAX_PASSWORD_BYTES
            && Argon2::default()
                .verify_password(password.as_bytes(), &self.0)
                .is_ok()
    }

    /// Returns the PHC string, as stored in the accounts file.
    pub(crate) fn to_phc_string(&self) -> String {
        self.0.to_string()
    }
}

/// Checks the length limits of a new password, without hashing it.
///
/// # Errors
///
/// Returns [`PasswordError::TooShort`] or [`PasswordError::TooLong`].
pub(crate) fn validate_new(password: &str) -> Result<(), PasswordError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(PasswordError::TooShort);
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(PasswordError::TooLong);
    }
    Ok(())
}

impl Debug for PasswordHash {
    /// Prints a placeholder instead of the hash.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordHash(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TEST_PASSWORD, cheap_password_hash};
    use std::assert_matches;

    /// A hash accepts the original password and nothing else.
    #[test]
    fn verify_accepts_only_original_password() {
        let hash = PasswordHash::parse(&cheap_password_hash(TEST_PASSWORD))
            .expect("fixture should be a valid hash");

        assert!(hash.verify(TEST_PASSWORD));
        assert!(!hash.verify("wrong-password"));
        assert!(!hash.verify(""));
    }

    /// New hashes use Argon2id, a random salt, and verify the password.
    #[test]
    fn create_uses_argon2id_with_random_salt() {
        let password = "correct horse battery staple";

        let first = PasswordHash::create(password).expect("hashing should succeed");
        let second = PasswordHash::create(password).expect("hashing should succeed");

        assert!(first.to_phc_string().starts_with("$argon2id$"));
        assert_ne!(first.to_phc_string(), second.to_phc_string());
        assert!(first.verify(password));
    }

    /// Weak passwords are rejected before they can be stored.
    #[test]
    fn create_rejects_short_password() {
        assert_matches!(PasswordHash::create("short"), Err(PasswordError::TooShort));
    }

    /// Non-ASCII characters count as one character each, not by their bytes.
    #[test]
    fn create_counts_characters_not_bytes() {
        assert_matches!(
            PasswordHash::create("äöüäöüäö"),
            Err(PasswordError::TooShort)
        );
    }

    /// Overlong passwords are rejected to bound the work per request.
    #[test]
    fn create_and_verify_reject_overlong_password() {
        let overlong = "x".repeat(MAX_PASSWORD_BYTES + 1);
        let hash = PasswordHash::parse(&cheap_password_hash(&overlong))
            .expect("fixture should be a valid hash");

        assert_matches!(PasswordHash::create(&overlong), Err(PasswordError::TooLong));
        assert!(!hash.verify(&overlong));
    }

    /// Malformed strings fail when the accounts file loads, not at login.
    #[test]
    fn parse_rejects_malformed_string() {
        assert_matches!(
            PasswordHash::parse("plaintext"),
            Err(PasswordError::InvalidHash(_))
        );
    }

    /// Other algorithms, such as bcrypt or Argon2i, aren't accepted.
    #[test]
    fn parse_rejects_other_algorithm() {
        let argon2i = "$argon2i$v=19$m=8,t=1,p=1$c2FsdHNhbHQ$AAAAAAAAAAAAAAAAAAAAAA";

        assert_matches!(
            PasswordHash::parse(argon2i),
            Err(PasswordError::UnsupportedAlgorithm(_))
        );
    }

    /// Hashes must not end up in logs through `{:?}`.
    #[test]
    fn debug_output_hides_hash() {
        let hash = PasswordHash::parse(&cheap_password_hash(TEST_PASSWORD))
            .expect("fixture should be a valid hash");

        assert!(!format!("{hash:?}").contains("argon2id"));
    }
}
