// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rejects cross-origin state-changing requests.
//!
//! The allowed origins come from the configuration, never from the `Host`
//! header, which a client controls.

use axum::http::{HeaderMap, HeaderValue, header};

/// The name of the `Sec-Fetch-Site` header.
const SEC_FETCH_SITE: &str = "sec-fetch-site";

/// Checks if a state-changing request comes from one of the `allowed` origins.
///
/// Browsers always send `Origin` on `POST`. Requests without `Origin` and
/// `Sec-Fetch-Site` don't come from a browser, so they have no ambient
/// credentials to abuse and are let through to the remaining checks.
pub(crate) fn is_same_origin(headers: &HeaderMap, allowed: &[String]) -> bool {
    if let Some(origin) = headers.get(header::ORIGIN) {
        return allowed.iter().any(|candidate| candidate == origin);
    }
    headers
        .get(SEC_FETCH_SITE)
        .is_none_or(is_trusted_fetch_site)
}

/// Checks if a `Sec-Fetch-Site` value is a same-origin or direct navigation.
fn is_trusted_fetch_site(value: &HeaderValue) -> bool {
    value == "same-origin" || value == "none"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The origins used in the tests.
    fn allowed() -> Vec<String> {
        vec!["https://praxis.test".to_string()]
    }

    /// Builds a request header map with the given single header.
    fn headers(name: &'static str, value: &'static str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(name, HeaderValue::from_static(value));
        map
    }

    /// The configured origin passes.
    #[test]
    fn accepts_configured_origin() {
        assert!(is_same_origin(
            &headers("origin", "https://praxis.test"),
            &allowed()
        ));
    }

    /// Other sites, other schemes, and sandboxed frames are rejected.
    #[test]
    fn rejects_foreign_origins() {
        for origin in [
            "https://evil.test",
            "http://praxis.test",
            "https://praxis.test.evil.test",
            "null",
        ] {
            assert!(!is_same_origin(&headers("origin", origin), &allowed()));
        }
    }

    /// Without `Origin`, the fetch metadata decides.
    #[test]
    fn falls_back_to_fetch_metadata() {
        assert!(is_same_origin(
            &headers(SEC_FETCH_SITE, "same-origin"),
            &allowed()
        ));
        assert!(!is_same_origin(
            &headers(SEC_FETCH_SITE, "cross-site"),
            &allowed()
        ));
        assert!(!is_same_origin(
            &headers(SEC_FETCH_SITE, "same-site"),
            &allowed()
        ));
    }

    /// Non-browser clients have no ambient credentials, so they may proceed.
    #[test]
    fn allows_requests_without_browser_headers() {
        assert!(is_same_origin(&HeaderMap::new(), &allowed()));
    }
}
