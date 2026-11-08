// SPDX-License-Identifier: MIT OR Apache-2.0

//! Keeps a deployment out of search results, e.g., a test instance.
//!
//! The header also covers responses without HTML, such as images, which
//! can't carry a `robots` meta tag.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

/// The header that tells search engines how to treat a response.
const X_ROBOTS_TAG: HeaderName = HeaderName::from_static("x-robots-tag");

/// Asks search engines not to index the response or follow its links.
pub(crate) async fn forbid_indexing(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(X_ROBOTS_TAG, HeaderValue::from_static("noindex, nofollow"));
    response
}
