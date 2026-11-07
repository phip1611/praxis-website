// SPDX-License-Identifier: MIT OR Apache-2.0

//! Routes of the local development mode.
//!
//! The live reload script polls the boot ID and reloads the page once it
//! changes, i.e., after `bacon` rebuilt and restarted the server.

use crate::app::AppState;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::IntoResponse;
use axum::routing::get;

/// Returns the development routes.
pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/__dev/boot-id", get(boot_id))
}

/// Returns an ID that is unique per server start.
async fn boot_id(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        state.boot_id.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use crate::test_support::TestApp;
    use axum::http::StatusCode;

    /// The dev routes exist in dev mode and return the boot ID.
    #[tokio::test]
    async fn boot_id_is_served_in_dev_mode() {
        let app = TestApp::dev();

        let response = app.get("/__dev/boot-id").await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.text(), app.state.boot_id.as_ref());
    }

    /// Production has no dev routes.
    #[tokio::test]
    async fn boot_id_is_absent_in_production() {
        let app = TestApp::new();

        let response = app.get("/__dev/boot-id").await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
    }

    /// Pages of the dev mode load the live reload script.
    #[tokio::test]
    async fn dev_pages_load_live_reload() {
        let app = TestApp::dev();

        let response = app.get("/").await;

        assert!(response.text().contains("/assets/live-reload."));
    }
}
