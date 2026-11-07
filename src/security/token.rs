// SPDX-License-Identifier: MIT OR Apache-2.0

//! Random secrets and constant-time comparison.

use crate::hex;
use std::fmt::{self, Debug, Formatter};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// The entropy of a token in bytes. 256 bits can't be guessed online.
const TOKEN_BYTES: usize = 32;

/// The operating system's random number generator failed.
#[derive(Clone, Copy, Debug, Error)]
#[error("the operating system's random number generator failed")]
pub(crate) struct RandomError;

/// A random secret in hex encoding, safe to use in cookies and HTML forms.
///
/// The [`Debug`] implementation hides the value to keep it out of logs.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SecretToken(String);

impl SecretToken {
    /// Creates a new token from the operating system's random number generator.
    ///
    /// # Errors
    ///
    /// Returns [`RandomError`] if the random number generator fails.
    pub(crate) fn generate() -> Result<Self, RandomError> {
        let mut bytes = [0; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| RandomError)?;
        Ok(Self(hex::encode(&bytes)))
    }

    /// Returns the token as text.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Compares the token with `provided` in constant time.
    ///
    /// The duration of the comparison reveals nothing about how many leading
    /// characters match, so that an attacker can't guess the token piecewise.
    pub(super) fn matches(&self, provided: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), provided.as_bytes())
    }
}

impl Debug for SecretToken {
    /// Prints a placeholder instead of the secret.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken(<redacted>)")
    }
}

/// Compares two byte strings without an early exit on the first difference.
///
/// Only the length of the inputs may influence the duration, which is fine as
/// the length of tokens is public.
pub(super) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.ct_eq(right).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokens have the documented length and alphabet.
    #[test]
    fn generate_returns_hex_of_expected_length() {
        let token = SecretToken::generate().expect("OS RNG should work in tests");

        assert_eq!(token.as_str().len(), TOKEN_BYTES * 2);
        assert!(token.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Two generated tokens differ, so the generator isn't constant.
    #[test]
    fn generate_returns_unique_tokens() {
        let first = SecretToken::generate().expect("OS RNG should work in tests");
        let second = SecretToken::generate().expect("OS RNG should work in tests");

        assert_ne!(first, second);
    }

    /// `matches` accepts only the exact token.
    #[test]
    fn matches_accepts_only_exact_value() {
        let token = SecretToken("abcd".to_string());

        assert!(token.matches("abcd"));
        assert!(!token.matches("abce"));
        assert!(!token.matches("abc"));
        assert!(!token.matches("abcde"));
        assert!(!token.matches(""));
    }

    /// Secrets must not end up in logs through `{:?}`.
    #[test]
    fn debug_output_hides_secret() {
        let token = SecretToken("very-secret".to_string());

        assert!(!format!("{token:?}").contains("very-secret"));
    }

    /// Different lengths and contents are unequal, identical input is equal.
    #[test]
    fn constant_time_eq_compares_content_and_length() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"same", b"same"));
        assert!(!constant_time_eq(b"same", b"sane"));
        assert!(!constant_time_eq(b"same", b"same "));
    }
}
