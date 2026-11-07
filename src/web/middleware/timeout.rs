// SPDX-License-Identifier: MIT OR Apache-2.0

//! Limits how long a request may take, including reading its body.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::time::Duration;
use tokio::time;

/// The longest a request may take. Generous, as saving includes a disk sync.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Aborts requests that take longer than [`REQUEST_TIMEOUT`].
///
/// Answers 503, not 408: the latter means the client sent its request too
/// slowly, and browsers resend such requests automatically.
pub(crate) async fn limit(request: Request, next: Next) -> Response {
    time::timeout(REQUEST_TIMEOUT, next.run(request))
        .await
        .unwrap_or_else(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())
}
