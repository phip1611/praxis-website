// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `hash-password` subcommand, which creates accounts file entries.

use crate::security::credentials::{UsernameError, validate_username};
use crate::security::password::{PasswordError, PasswordHash};
use std::io::{self, BufRead, IsTerminal};
use thiserror::Error;

/// A failure of the `hash-password` subcommand.
#[derive(Debug, Error)]
pub(crate) enum HashPasswordError {
    /// The username is invalid.
    #[error(transparent)]
    Username(#[from] UsernameError),
    /// The password is invalid or can't be hashed.
    #[error(transparent)]
    Password(#[from] PasswordError),
    /// The two entered passwords differ.
    #[error("the passwords don't match")]
    Mismatch,
    /// Reading the password failed.
    #[error("can't read the password: {0}")]
    Io(#[from] io::Error),
}

/// Reads a password and prints the accounts file entry to stdout.
///
/// On a terminal, the password is read twice without echo. Otherwise, the
/// first line of stdin is the password, for scripts.
///
/// # Errors
///
/// Returns [`HashPasswordError`] if an input is invalid or reading fails.
pub(crate) fn run(username: &str) -> Result<(), HashPasswordError> {
    validate_username(username)?;
    let password = if io::stdin().is_terminal() {
        let first = rpassword::prompt_password("Password: ")?;
        let second = rpassword::prompt_password("Repeat password: ")?;
        if first != second {
            return Err(HashPasswordError::Mismatch);
        }
        first
    } else {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_string()
    };
    println!("{}", entry(username, &password)?);
    Ok(())
}

/// Returns the accounts file line for `username` and `password`.
///
/// # Errors
///
/// Returns [`HashPasswordError`] if the username or password is invalid.
fn entry(username: &str, password: &str) -> Result<String, HashPasswordError> {
    validate_username(username)?;
    let hash = PasswordHash::create(password)?;
    Ok(format!("{username}:{}", hash.to_phc_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::credentials::CredentialStore;
    use std::assert_matches;

    /// The printed line is accepted by the accounts file parser.
    #[test]
    fn entry_is_valid_accounts_line() {
        let password = "correct horse battery staple";

        let line = entry("dirk", password).expect("entry should be created");

        let store = CredentialStore::parse(&line).expect("line should parse");
        assert_eq!(store.verify("dirk", password).as_deref(), Some("dirk"));
    }

    /// Invalid input is rejected before hashing.
    #[test]
    fn entry_rejects_invalid_input() {
        assert_matches!(
            entry("a:b", "long enough password"),
            Err(HashPasswordError::Username(_))
        );
        assert_matches!(
            entry("dirk", "short"),
            Err(HashPasswordError::Password(PasswordError::TooShort))
        );
    }
}
