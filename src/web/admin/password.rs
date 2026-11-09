// SPDX-License-Identifier: MIT OR Apache-2.0

//! Changing the own password.

use crate::app::AppState;
use crate::render::templates::{Notice, NoticeKind};
use crate::security::authenticator::PasswordChangeError;
use crate::security::client_ip;
use crate::security::password::{MIN_PASSWORD_CHARS, PasswordError, validate_new};
use crate::web::AppError;
use crate::web::admin::forms::{CsrfProtected, SessionForm};
use crate::web::admin::{AdminSession, PASSWORD_PATH};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;
use tracing::error;

/// The query string after a successful change.
const CHANGED_QUERY: &str = "geaendert";

/// The submitted form.
#[derive(Deserialize)]
pub(super) struct PasswordForm {
    /// The CSRF token of the session.
    csrf: String,
    /// The current password.
    current: String,
    /// The new password.
    new: String,
    /// The new password again, against typos.
    repeat: String,
}

impl Debug for PasswordForm {
    /// Hides the passwords.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordForm").finish_non_exhaustive()
    }
}

impl CsrfProtected for PasswordForm {
    /// Returns the submitted CSRF token.
    fn csrf(&self) -> &str {
        &self.csrf
    }
}

/// Shows the form.
pub(super) async fn show(
    State(state): State<AppState>,
    admin: AdminSession,
    uri: Uri,
) -> Result<Html<String>, AppError> {
    let notice = (uri.query() == Some(CHANGED_QUERY)).then(|| Notice {
        kind: NoticeKind::Success,
        text: "Ihr Passwort wurde geändert.".to_string(),
    });
    let html = state.renderer.render_password(
        &admin.session.username,
        admin.session.csrf.as_str(),
        notice.as_ref(),
    )?;
    Ok(Html(html))
}

/// Checks the form, changes the password, and ends the other sessions.
pub(super) async fn change(
    State(state): State<AppState>,
    headers: HeaderMap,
    SessionForm { admin, form }: SessionForm<PasswordForm>,
) -> Result<Response, AppError> {
    let rejection = if form.new != form.repeat {
        Some("Die beiden neuen Passwörter stimmen nicht überein.".to_string())
    } else if form.new == form.current {
        Some("Das neue Passwort muss sich vom bisherigen unterscheiden.".to_string())
    } else {
        validate_new(&form.new)
            .err()
            .map(|err| invalid_message(&err))
    };
    if let Some(message) = rejection {
        return render_error(
            &state,
            &admin,
            StatusCode::UNPROCESSABLE_ENTITY,
            message,
            None,
        );
    }

    let account = &admin.session.username;
    let client = client_ip::from_headers(&headers, state.config.client_ip_header.as_ref());
    // A separate task, so that a cancelled request (client disconnect or
    // timeout) can't change the password but keep the other sessions alive.
    let (auth, owner, token) = (
        Arc::clone(&state.auth),
        account.clone(),
        admin.token.clone(),
    );
    let result = tokio::spawn(async move {
        let result = auth
            .change_password(&owner, &form.current, &form.new, client)
            .await;
        if result.is_ok() {
            auth.sessions().revoke_others(&owner, &token);
        }
        result
    })
    .await
    .map_err(|_| AppError::Task)?;
    let (status, message, retry_after) = match result {
        Ok(()) => {
            let location = format!("{PASSWORD_PATH}?{CHANGED_QUERY}");
            return Ok((StatusCode::SEE_OTHER, [(header::LOCATION, location)]).into_response());
        }
        Err(PasswordChangeError::WrongPassword) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Das bisherige Passwort ist falsch.".to_string(),
            None,
        ),
        Err(PasswordChangeError::Invalid(err)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            invalid_message(&err),
            None,
        ),
        Err(PasswordChangeError::Throttled { retry_after }) => {
            let seconds = retry_after.as_secs().unsigned_abs().max(1);
            let message = format!(
                "Zu viele Fehlversuche. Bitte versuchen Sie es in {} Minuten erneut.",
                seconds.div_ceil(60)
            );
            (StatusCode::TOO_MANY_REQUESTS, message, Some(seconds))
        }
        Err(PasswordChangeError::Busy) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Der Server ist gerade ausgelastet. Bitte versuchen Sie es gleich noch einmal."
                .to_string(),
            None,
        ),
        Err(PasswordChangeError::Store(err)) => {
            error!(account, "can't store the new password: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Das neue Passwort konnte wegen eines technischen Fehlers nicht gespeichert \
                 werden. Das bisherige Passwort gilt weiter."
                    .to_string(),
                None,
            )
        }
        Err(PasswordChangeError::TaskFailed) => return Err(AppError::Task),
    };
    render_error(&state, &admin, status, message, retry_after)
}

/// Returns the message for a new password that violates the limits.
fn invalid_message(err: &PasswordError) -> String {
    match err {
        PasswordError::TooShort => {
            format!("Das neue Passwort muss mindestens {MIN_PASSWORD_CHARS} Zeichen lang sein.")
        }
        _ => "Das neue Passwort ist zu lang.".to_string(),
    }
}

/// Shows the form again with an error.
fn render_error(
    state: &AppState,
    admin: &AdminSession,
    status: StatusCode,
    message: String,
    retry_after: Option<u64>,
) -> Result<Response, AppError> {
    let notice = Notice {
        kind: NoticeKind::Error,
        text: message,
    };
    let html = state.renderer.render_password(
        &admin.session.username,
        admin.session.csrf.as_str(),
        Some(&notice),
    )?;
    let mut response = (status, Html(html)).into_response();
    if let Some(seconds) = retry_after {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use crate::test_support::{TEST_PASSWORD, TEST_USER, TestApp, poll_once_and_drop};
    use axum::http::StatusCode;
    use std::time::Duration;
    use tokio::time;

    /// A long enough new password of the tests.
    const NEW_PASSWORD: &str = "a completely new password";

    /// The page explains that there are no e-mails, e.g., for resets.
    #[tokio::test]
    async fn page_states_that_no_emails_are_sent() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session.get("/admin/passwort/").await;

        assert_eq!(response.status, StatusCode::OK);
        assert!(
            response
                .visible_text()
                .contains("Diese Website kann keine E-Mails versenden")
        );
    }

    /// A successful change applies to the next login and ends other sessions,
    /// but not the current one.
    #[tokio::test]
    async fn change_applies_and_ends_other_sessions() {
        let app = TestApp::new();
        let session = app.login().await;
        let other = app.login().await;

        let response = session
            .post(
                "/admin/passwort/",
                &[
                    ("csrf", session.csrf()),
                    ("current", TEST_PASSWORD),
                    ("new", NEW_PASSWORD),
                    ("repeat", NEW_PASSWORD),
                ],
            )
            .await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/passwort/?geaendert");
        assert_eq!(session.get("/admin/").await.status, StatusCode::OK);
        assert_eq!(other.get("/admin/").await.status, StatusCode::SEE_OTHER);
        assert_eq!(
            app.login_with(TEST_USER, NEW_PASSWORD).await.status,
            StatusCode::SEE_OTHER
        );
    }

    /// A cancelled request can't change the password and keep other sessions.
    #[tokio::test]
    async fn cancelled_change_still_ends_other_sessions() {
        let app = TestApp::new();
        let session = app.login().await;
        let other = app.login().await;
        let fields = [
            ("csrf", session.csrf()),
            ("current", TEST_PASSWORD),
            ("new", NEW_PASSWORD),
            ("repeat", NEW_PASSWORD),
        ];

        poll_once_and_drop(session.post("/admin/passwort/", &fields)).await;

        // The change runs on, so this waits for its end, for up to 5 seconds.
        for _ in 0..100 {
            if other.get("/admin/").await.status == StatusCode::SEE_OTHER {
                return;
            }
            time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the other session should end");
    }

    /// Typos and a wrong current password are reported, and nothing changes.
    #[tokio::test]
    async fn invalid_input_is_reported() {
        let app = TestApp::new();
        let session = app.login().await;
        let cases = [
            (
                TEST_PASSWORD,
                NEW_PASSWORD,
                "something else entirely",
                "stimmen nicht überein",
            ),
            (TEST_PASSWORD, "short", "short", "mindestens 15 Zeichen"),
            (
                TEST_PASSWORD,
                TEST_PASSWORD,
                TEST_PASSWORD,
                "vom bisherigen unterscheiden",
            ),
            (
                "wrong password",
                NEW_PASSWORD,
                NEW_PASSWORD,
                "bisherige Passwort ist falsch",
            ),
        ];

        for (current, new, repeat, message) in cases {
            let response = session
                .post(
                    "/admin/passwort/",
                    &[
                        ("csrf", session.csrf()),
                        ("current", current),
                        ("new", new),
                        ("repeat", repeat),
                    ],
                )
                .await;

            assert_eq!(
                response.status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{message}"
            );
            assert!(response.visible_text().contains(message));
        }
        assert_eq!(
            app.login_with(TEST_USER, TEST_PASSWORD).await.status,
            StatusCode::SEE_OTHER
        );
    }

    /// The change needs the session's CSRF token.
    #[tokio::test]
    async fn change_requires_csrf_token() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session
            .post(
                "/admin/passwort/",
                &[
                    ("csrf", "forged"),
                    ("current", TEST_PASSWORD),
                    ("new", NEW_PASSWORD),
                    ("repeat", NEW_PASSWORD),
                ],
            )
            .await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
}
