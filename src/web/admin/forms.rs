// SPDX-License-Identifier: MIT OR Apache-2.0

//! Form extractors that apply the request forgery checks.
//!
//! Every state-changing handler takes one of them, so that a new handler
//! can't forget a check: [`SameOriginForm`] for the login, [`SessionForm`]
//! for everything that needs a session.

use crate::app::AppState;
use crate::security::origin::is_same_origin;
use crate::web::admin::AdminSession;
use axum::Form;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use tracing::warn;

/// A form that carries the CSRF token of the session.
pub(super) trait CsrfProtected {
    /// Returns the submitted CSRF token.
    fn csrf(&self) -> &str;
}

/// A form submitted from a page of this site.
#[derive(Debug)]
pub(super) struct SameOriginForm<T>(pub T);

impl<T> FromRequest<AppState> for SameOriginForm<T>
where
    T: DeserializeOwned + Send,
{
    type Rejection = Response;

    /// Rejects foreign origins before the body is parsed.
    async fn from_request(request: Request, state: &AppState) -> Result<Self, Response> {
        if !is_same_origin(request.headers(), &state.config.allowed_origins) {
            warn!("rejected cross-origin request to the login area");
            return Err(forbidden());
        }
        let Form(form) = Form::<T>::from_request(request, state)
            .await
            .map_err(IntoResponse::into_response)?;
        Ok(Self(form))
    }
}

/// A form of a logged-in session: from this site and with the session's
/// CSRF token. Requests without session are redirected to the login form.
#[derive(Debug)]
pub(super) struct SessionForm<T> {
    /// The session that submitted the form.
    pub(super) admin: AdminSession,
    /// The form.
    pub(super) form: T,
}

impl<T> FromRequest<AppState> for SessionForm<T>
where
    T: DeserializeOwned + CsrfProtected + Send,
{
    type Rejection = Response;

    /// Checks session, origin, and CSRF token, in this order.
    async fn from_request(request: Request, state: &AppState) -> Result<Self, Response> {
        let (mut parts, body) = request.into_parts();
        let admin = AdminSession::from_request_parts(&mut parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let request = Request::from_parts(parts, body);
        let SameOriginForm(form) = SameOriginForm::<T>::from_request(request, state).await?;
        if !admin.session.csrf.verify(form.csrf()) {
            warn!(
                account = admin.session.username,
                "rejected request with invalid CSRF token"
            );
            return Err(forbidden());
        }
        Ok(Self { admin, form })
    }
}

/// Returns the response for rejected requests.
fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "Anfrage abgelehnt.").into_response()
}
