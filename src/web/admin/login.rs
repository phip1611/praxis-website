// SPDX-License-Identifier: MIT OR Apache-2.0

//! Login and logout.

use crate::app::AppState;
use crate::content::pages::HOME_PATH;
use crate::security::authenticator::LoginError;
use crate::web::AppError;
use crate::web::admin::forms::{CsrfProtected, SameOriginForm, SessionForm};
use crate::web::admin::{STATS_PATH, current_session};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::fmt::{self, Debug, Formatter};
use tracing::info;

/// The submitted login form.
#[derive(Deserialize)]
pub(super) struct LoginForm {
    /// The username.
    username: String,
    /// The password.
    password: String,
}

impl Debug for LoginForm {
    /// Hides the password.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginForm")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// The logout form, which only carries the CSRF token.
#[derive(Debug, Deserialize)]
pub(super) struct LogoutForm {
    /// The CSRF token of the session.
    csrf: String,
}

impl CsrfProtected for LogoutForm {
    /// Returns the submitted CSRF token.
    fn csrf(&self) -> &str {
        &self.csrf
    }
}

/// Shows the login form, or the statistics if already logged in.
pub(super) async fn show(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if current_session(&state, &headers).is_some() {
        return Ok(Redirect::to(STATS_PATH).into_response());
    }
    Ok(Html(state.renderer.render_login("", None)?).into_response())
}

/// Checks the credentials and starts a session.
pub(super) async fn submit(
    State(state): State<AppState>,
    SameOriginForm(form): SameOriginForm<LoginForm>,
) -> Result<Response, AppError> {
    let username = form.username.trim();
    let (status, message) = match state.auth.login(username, &form.password).await {
        Ok(token) => {
            let cookie = HeaderValue::from_str(&state.cookie.set(token.as_str()))
                .expect("the cookie should be a valid header");
            return Ok((
                StatusCode::SEE_OTHER,
                [
                    (header::LOCATION, HeaderValue::from_static(STATS_PATH)),
                    (header::SET_COOKIE, cookie),
                ],
            )
                .into_response());
        }
        Err(LoginError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            "Benutzername oder Passwort ist falsch.".to_string(),
        ),
        Err(LoginError::Random(err)) => return Err(err.into()),
        Err(LoginError::TaskFailed) => return Err(AppError::Task),
    };
    let html = state.renderer.render_login(username, Some(&message))?;
    Ok((status, Html(html)).into_response())
}

/// Ends the session and returns to the start page.
pub(super) async fn logout(
    State(state): State<AppState>,
    SessionForm {
        admin,
        form: LogoutForm { .. },
    }: SessionForm<LogoutForm>,
) -> Response {
    state.auth.sessions().revoke(&admin.token);
    info!(account = admin.session.username, "logged out");
    let cookie =
        HeaderValue::from_str(&state.cookie.clear()).expect("the cookie should be a valid header");
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, HeaderValue::from_static(HOME_PATH)),
            (header::SET_COOKIE, cookie),
        ],
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use crate::test_support::{TEST_PASSWORD, TEST_USER, TestApp};
    use axum::http::StatusCode;

    /// Valid credentials set a hardened cookie and open the login area.
    #[tokio::test]
    async fn login_sets_session_cookie() {
        let app = TestApp::new();

        let response = app
            .post_form(
                "/admin/login/",
                &[("username", TEST_USER), ("password", TEST_PASSWORD)],
                None,
            )
            .await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/statistik/");
        let cookie = response.header("set-cookie");
        assert!(cookie.starts_with("__Host-praxis-session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
    }

    /// Wrong credentials show the form again, with the username kept.
    #[tokio::test]
    async fn wrong_password_shows_error() {
        let app = TestApp::new();

        let response = app
            .post_form(
                "/admin/login/",
                &[("username", TEST_USER), ("password", "wrong")],
                None,
            )
            .await;

        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert!(
            response
                .text()
                .contains("Benutzername oder Passwort ist falsch.")
        );
        assert!(response.text().contains(&format!("value=\"{TEST_USER}\"")));
        assert!(!response.headers.contains_key("set-cookie"));
    }

    /// Other sites can't log a visitor into an account (login CSRF).
    #[tokio::test]
    async fn login_from_foreign_origin_is_rejected() {
        let app = TestApp::new();

        let response = app
            .post_form_from(
                "https://evil.test",
                "/admin/login/",
                &[("username", TEST_USER), ("password", TEST_PASSWORD)],
                None,
            )
            .await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }

    /// A logged-in user goes straight to the statistics.
    #[tokio::test]
    async fn login_page_redirects_when_logged_in() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session.get("/admin/login/").await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/statistik/");
    }

    /// Logging out ends the session on the server and clears the cookie.
    #[tokio::test]
    async fn logout_ends_session() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session
            .post("/admin/logout/", &[("csrf", session.csrf())])
            .await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert!(response.header("set-cookie").contains("Max-Age=0"));
        assert_eq!(
            session.get("/admin/statistik/").await.status,
            StatusCode::SEE_OTHER
        );
    }

    /// Other sites can't log a visitor out.
    #[tokio::test]
    async fn logout_requires_csrf_token() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session.post("/admin/logout/", &[("csrf", "forged")]).await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert_eq!(
            session.get("/admin/statistik/").await.status,
            StatusCode::OK
        );
    }
}
