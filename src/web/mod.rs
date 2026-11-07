// SPDX-License-Identifier: MIT OR Apache-2.0

//! The HTTP interface.

mod middleware;
mod public;

use crate::app::AppState;
use axum::Router;
use axum::middleware::{from_fn, from_fn_with_state};

/// Builds the router with all routes and middleware.
pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .fallback(public::serve)
        // The last layer is the outermost, so timeouts get security headers
        // and are logged.
        .layer(from_fn(middleware::timeout::limit))
        .layer(from_fn_with_state(
            state.clone(),
            middleware::security_headers::apply,
        ))
        .layer(from_fn(middleware::request_log::log))
        .with_state(state)
}
