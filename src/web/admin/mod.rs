// SPDX-License-Identifier: MIT OR Apache-2.0

//! The login area with the access statistics.

mod forms;
mod login;
mod stats;

use crate::app::AppState;
use crate::security::session::Session;
use axum::Router;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::{Next, from_fn};
use axum::response::{Redirect, Response};
use axum::routing::{get, post};

/// The path of the login form.
pub(crate) const LOGIN_PATH: &str = "/admin/login/";

/// The path of the access statistics, the start of the login area.
pub(super) const STATS_PATH: &str = "/admin/statistik/";

/// The path for logging out.
const LOGOUT_PATH: &str = "/admin/logout/";

/// The largest accepted form body. Far more than any news text, but small
/// enough that floods can't exhaust the memory.
const MAX_FORM_BYTES: usize = 256 * 1024;

/// Returns the routes of the login area.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(LOGIN_PATH, get(login::show).post(login::submit))
        .route(LOGOUT_PATH, post(login::logout))
        .route(STATS_PATH, get(stats::show))
        .layer(DefaultBodyLimit::max(MAX_FORM_BYTES))
        .layer(from_fn(private_headers))
}

/// Keeps the login area out of caches and search results.
async fn private_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    response
}

/// Returns the session of the request, if it has a valid one.
fn current_session(state: &AppState, headers: &HeaderMap) -> Option<(Session, String)> {
    let token = state.cookie.read(headers)?;
    let session = state.auth.sessions().authenticate(token)?;
    Some((session, token.to_string()))
}

/// An authenticated session. Requests without one are redirected to the
/// login form.
#[derive(Debug)]
struct AdminSession {
    /// The session data.
    session: Session,
    /// The session token, for logging out.
    token: String,
}

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = Redirect;

    /// Looks up the session of the cookie.
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Redirect> {
        current_session(state, &parts.headers)
            .map(|(session, token)| Self { session, token })
            .ok_or_else(|| Redirect::to(LOGIN_PATH))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::TestApp;
    use axum::http::StatusCode;

    /// Without session, the statistics send to the login form.
    #[tokio::test]
    async fn statistics_require_login() {
        let app = TestApp::new();

        let response = app.get("/admin/statistik/").await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/login/");
    }

    /// Pages of the login area are neither cached nor indexed.
    #[tokio::test]
    async fn pages_are_private() {
        let app = TestApp::new();

        let response = app.get("/admin/login/").await;

        assert_eq!(response.header("cache-control"), "no-store");
        assert_eq!(response.header("x-robots-tag"), "noindex, nofollow");
    }

    /// Forged or expired cookies count as logged out.
    #[tokio::test]
    async fn invalid_cookie_is_rejected() {
        let app = TestApp::new();

        let response = app
            .get_with(
                "/admin/statistik/",
                &[("cookie", "__Host-praxis-session=forged")],
            )
            .await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
    }

    /// Oversized bodies are rejected before they're processed.
    #[tokio::test]
    async fn rejects_oversized_bodies() {
        let app = TestApp::new();
        let huge = "x".repeat(300 * 1024);

        let response = app
            .post_form(
                "/admin/login/",
                &[("username", &huge), ("password", "x")],
                None,
            )
            .await;

        assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    }
}
