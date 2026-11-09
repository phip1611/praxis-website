// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-session tokens against cross-site request forgery.
//!
//! This is a second layer next to `SameSite=Strict` cookies and the check in
//! [`crate::security::origin`].

use crate::security::token::{RandomError, SecretToken};

/// The token that state-changing requests of a session must contain.
#[derive(Clone, Debug)]
pub(crate) struct CsrfToken(SecretToken);

impl CsrfToken {
    /// Creates a new random token.
    ///
    /// # Errors
    ///
    /// Returns [`RandomError`] if the random number generator fails.
    pub(super) fn generate() -> Result<Self, RandomError> {
        SecretToken::generate().map(Self)
    }

    /// Returns the token for embedding into an HTML form.
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Checks the token of a submitted form in constant time.
    pub(crate) fn verify(&self, provided: &str) -> bool {
        self.0.matches(provided)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the exact token of the session is accepted.
    #[test]
    fn verify_accepts_only_own_token() {
        let token = CsrfToken::generate().expect("OS RNG should work in tests");
        let other = CsrfToken::generate().expect("OS RNG should work in tests");

        assert!(token.verify(token.as_str()));
        assert!(!token.verify(other.as_str()));
        assert!(!token.verify(""));
    }

    /// The value must not end up in logs through `{:?}`.
    #[test]
    fn debug_output_hides_token() {
        let token = CsrfToken::generate().expect("OS RNG should work in tests");

        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
