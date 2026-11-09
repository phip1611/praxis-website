// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adds the security headers of [`crate::security::headers`] to all responses.

use crate::app::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

/// Runs the request and adds the security headers to the response.
pub(crate) async fn apply(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    state.security_headers.apply(response.headers_mut());
    response
}
