// SPDX-License-Identifier: MIT OR Apache-2.0

//! The access log.
//!
//! Logs method, path, status, and duration. Client IP addresses, user agents,
//! and query strings are left out on purpose: they are personal data or may
//! contain it, and the privacy policy promises that visits aren't logged.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Longer paths are cut, so that bots can't flood the log with huge lines.
const MAX_LOGGED_PATH_CHARS: usize = 200;

/// Logs one line per request.
///
/// Static assets are logged at debug level, as each page view loads several
/// and they would drown the interesting lines.
pub(crate) async fn log(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = shorten(request.uri().path());
    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed();
    let status = response.status();
    if status.is_server_error() {
        warn!("{method} {path} -> {} ({elapsed:.1?})", status.as_u16());
    } else if path.starts_with("/assets/") {
        debug!("{method} {path} -> {} ({elapsed:.1?})", status.as_u16());
    } else {
        info!("{method} {path} -> {} ({elapsed:.1?})", status.as_u16());
    }
    response
}

/// Cuts `path` to [`MAX_LOGGED_PATH_CHARS`] characters.
fn shorten(path: &str) -> String {
    if path.chars().count() <= MAX_LOGGED_PATH_CHARS {
        return path.to_string();
    }
    let short: String = path.chars().take(MAX_LOGGED_PATH_CHARS).collect();
    format!("{short}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Normal paths stay as they are.
    #[test]
    fn shorten_keeps_short_paths() {
        assert_eq!(shorten("/kontakt/"), "/kontakt/");
    }

    /// Overlong paths are cut and marked.
    #[test]
    fn shorten_cuts_long_paths() {
        let long = format!("/{}", "a".repeat(500));

        let short = shorten(&long);

        assert_eq!(short.chars().count(), MAX_LOGGED_PATH_CHARS + 1);
        assert!(short.ends_with('…'));
    }
}
