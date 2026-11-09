// SPDX-License-Identifier: MIT OR Apache-2.0

//! Login and logout.

use crate::app::AppState;
use crate::content::pages::HOME_PATH;
use crate::security::authenticator::LoginError;
use crate::security::client_ip;
use crate::web::AppError;
use crate::web::admin::forms::{CsrfProtected, SameOriginForm, SessionForm};
use crate::web::admin::{STATS_PATH, current_session};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;
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
    headers: HeaderMap,
    SameOriginForm(form): SameOriginForm<LoginForm>,
) -> Result<Response, AppError> {
    let username = form.username.trim();
    let client = client_ip::from_headers(&headers, state.config.client_ip_header.as_ref());
    // A separate task, so that a failed login counts even if the client
    // disconnects while the password is checked.
    let auth = Arc::clone(&state.auth);
    let (name, password) = (username.to_string(), form.password);
    let result = tokio::spawn(async move { auth.login(&name, &password, client).await })
        .await
        .map_err(|_| AppError::Task)?;
    let (status, message) = match result {
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
        Err(LoginError::Throttled { retry_after }) => {
            let seconds = retry_after.as_secs().unsigned_abs().max(1);
            let minutes = seconds.div_ceil(60);
            let html = state.renderer.render_login(
                username,
                Some(&format!(
                    "Zu viele fehlgeschlagene Anmeldungen. Bitte versuchen Sie es in \
                     {minutes} Minuten erneut."
                )),
            )?;
            let retry = HeaderValue::from(seconds);
            return Ok((
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, retry)],
                Html(html),
            )
                .into_response());
        }
        Err(LoginError::Busy) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Der Server ist gerade ausgelastet. Bitte versuchen Sie es gleich noch einmal."
                .to_string(),
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
    use crate::security::authenticator::MAX_FAILURES_PER_CLIENT;
    use crate::security::throttle::FAILURE_WINDOW;
    use crate::test_support::{TEST_PASSWORD, TEST_USER, TestApp, poll_once_and_drop};
    use axum::http::StatusCode;
    use std::time::Duration;
    use tokio::time;

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

    /// A client that disconnects while the password is checked still uses up
    /// its attempts, or it could exhaust the global limit for everyone.
    #[tokio::test]
    async fn aborted_logins_count_as_failures() {
        let app = TestApp::new();
        for _ in 0..MAX_FAILURES_PER_CLIENT {
            poll_once_and_drop(app.login_from("198.51.100.66", "wrong")).await;
            // Boring: gives the check time to finish, which takes well below 1 ms.
            time::sleep(Duration::from_millis(100)).await;
        }

        let response = app.login_from("198.51.100.66", TEST_PASSWORD).await;

        assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
    }

    /// A guessing client is blocked and told how long, while the real user on
    /// another client can still log in.
    #[tokio::test]
    async fn guessing_client_is_blocked_but_others_can_log_in() {
        let app = TestApp::new();
        for _ in 0..MAX_FAILURES_PER_CLIENT {
            app.login_from("198.51.100.66", "wrong").await;
        }

        let blocked = app.login_from("198.51.100.66", TEST_PASSWORD).await;
        let other = app.login_from("192.0.2.10", TEST_PASSWORD).await;

        let window = FAILURE_WINDOW.as_secs();
        assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(blocked.header("retry-after"), window.to_string());
        assert!(
            blocked
                .text()
                .contains(&format!("in {} Minuten erneut", window / 60))
        );
        assert_eq!(other.status, StatusCode::SEE_OTHER);
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
