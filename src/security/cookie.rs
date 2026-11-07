// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attributes and parsing of the session cookie.

use crate::security::session::ABSOLUTE_TIMEOUT;
use axum::http::{HeaderMap, header};

/// The cookie name over HTTPS.
///
/// The `__Host-` prefix makes browsers require `Secure`, `Path=/`, and no
/// `Domain`, so a subdomain can't plant a session cookie.
const SECURE_NAME: &str = "__Host-praxis-session";

/// The cookie name for local development over plain HTTP.
const INSECURE_NAME: &str = "praxis-session";

/// Creates and reads the session cookie.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SessionCookie {
    /// Whether the site runs over HTTPS.
    secure: bool,
}

impl SessionCookie {
    /// Creates the cookie policy. `secure` must be true for public deployments.
    pub(crate) const fn new(secure: bool) -> Self {
        Self { secure }
    }

    /// Returns the name of the cookie.
    const fn name(self) -> &'static str {
        if self.secure {
            SECURE_NAME
        } else {
            INSECURE_NAME
        }
    }

    /// Returns the attributes shared by setting and clearing the cookie.
    const fn attributes(self) -> &'static str {
        if self.secure {
            "Path=/; HttpOnly; Secure; SameSite=Strict"
        } else {
            "Path=/; HttpOnly; SameSite=Strict"
        }
    }

    /// Returns the `Set-Cookie` value that stores `token` in the browser.
    pub(crate) fn set(self, token: &str) -> String {
        format!(
            "{}={token}; Max-Age={}; {}",
            self.name(),
            ABSOLUTE_TIMEOUT.as_secs(),
            self.attributes()
        )
    }

    /// Returns the `Set-Cookie` value that removes the cookie from the browser.
    pub(crate) fn clear(self) -> String {
        format!("{}=; Max-Age=0; {}", self.name(), self.attributes())
    }

    /// Extracts the session token from the request's `Cookie` headers.
    pub(crate) fn read(self, headers: &HeaderMap) -> Option<&str> {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(name, _)| *name == self.name())
            .map(|(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// Builds request headers with the given `Cookie` header.
    fn cookie_headers(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, HeaderValue::from_static(value));
        headers
    }

    /// The production cookie has all protective attributes.
    #[test]
    fn secure_cookie_has_all_attributes() {
        let cookie = SessionCookie::new(true).set("abc");

        assert!(cookie.starts_with("__Host-praxis-session=abc;"));
        for attribute in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/"] {
            assert!(cookie.contains(attribute));
        }
        assert!(!cookie.contains("Domain"));
    }

    /// The local cookie omits `Secure`, as dev runs on plain HTTP.
    #[test]
    fn insecure_cookie_has_no_secure_attribute() {
        let cookie = SessionCookie::new(false).set("abc");

        assert!(cookie.starts_with("praxis-session=abc;"));
        assert!(!cookie.contains("Secure"));
        assert!(cookie.contains("HttpOnly"));
    }

    /// Clearing uses the same name and path, or browsers keep the cookie.
    #[test]
    fn clear_expires_cookie_immediately() {
        let cookie = SessionCookie::new(true).clear();

        assert!(cookie.starts_with("__Host-praxis-session=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Path=/"));
    }

    /// The value is found among other cookies, and only by its exact name.
    #[test]
    fn read_finds_exact_cookie_name() {
        let policy = SessionCookie::new(false);

        let found = cookie_headers("a=1; praxis-session=token; b=2");
        let other = cookie_headers("xpraxis-session=token; a=1");

        assert_eq!(policy.read(&found), Some("token"));
        assert_eq!(policy.read(&other), None);
        assert_eq!(policy.read(&HeaderMap::new()), None);
    }

    /// A cookie without the prefix can't be used on the secure site, since it
    /// lacks the guarantees of `__Host-`.
    #[test]
    fn secure_policy_ignores_unprefixed_cookie() {
        let headers = cookie_headers("praxis-session=token");

        assert_eq!(SessionCookie::new(true).read(&headers), None);
    }
}
