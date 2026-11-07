// SPDX-License-Identifier: MIT OR Apache-2.0

//! The set of all public responses at one point in time.

use crate::delivery::cached::CachedResponse;
use std::collections::HashMap;
use std::sync::Arc;

/// All public responses by path, plus the error page.
#[derive(Debug)]
pub(crate) struct Snapshot {
    /// The responses by request path.
    routes: HashMap<String, Arc<CachedResponse>>,
    /// The response for unknown paths.
    not_found: Arc<CachedResponse>,
}

impl Snapshot {
    /// Creates a snapshot.
    pub(crate) const fn new(
        routes: HashMap<String, Arc<CachedResponse>>,
        not_found: Arc<CachedResponse>,
    ) -> Self {
        Self { routes, not_found }
    }

    /// Returns the response for `path`, if there is one.
    pub(crate) fn get(&self, path: &str) -> Option<&CachedResponse> {
        self.routes.get(path).map(Arc::as_ref)
    }

    /// Returns the response for unknown paths.
    pub(crate) fn not_found(&self) -> &CachedResponse {
        &self.not_found
    }

    /// Returns all paths.
    #[cfg(test)]
    pub(crate) fn paths(&self) -> impl Iterator<Item = &str> {
        self.routes.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::cached::{CachePolicy, Compression};
    use axum::http::StatusCode;

    /// Creates a snapshot with one page that has `body` at `/`.
    fn snapshot(body: &str) -> Snapshot {
        let response = |body: &str| {
            Arc::new(CachedResponse::new(
                StatusCode::OK,
                "text/plain",
                CachePolicy::Revalidate,
                body.as_bytes().to_vec(),
                Compression::Never,
            ))
        };
        Snapshot::new(
            HashMap::from([("/".to_string(), response(body))]),
            response("404"),
        )
    }

    /// Paths resolve exactly, unknown paths don't.
    #[test]
    fn get_resolves_known_paths_only() {
        let snapshot = snapshot("start");

        assert_eq!(
            snapshot.get("/").map(CachedResponse::body),
            Some(&b"start"[..])
        );
        assert!(snapshot.get("/unknown").is_none());
        assert_eq!(snapshot.not_found().body(), b"404");
    }
}
