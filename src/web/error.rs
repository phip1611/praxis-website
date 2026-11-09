// SPDX-License-Identifier: MIT OR Apache-2.0

//! Unexpected failures in request handlers.

use crate::render::RenderError;
use crate::security::token::RandomError;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use thiserror::Error;
use tracing::error;

/// The page for unexpected errors. Static, so it works even if rendering is
/// what failed.
const INTERNAL_ERROR_PAGE: &str = "<!DOCTYPE html><html lang=\"de\"><head>\
    <meta charset=\"UTF-8\"><title>Fehler – Hausarztpraxis Dirk Schuster</title>\
    </head><body><h1>Ein Fehler ist aufgetreten</h1><p>Bitte versuchen Sie es \
    später erneut. <a href=\"/\">Zur Startseite</a></p></body></html>";

/// An unexpected failure. Details go to the log, the client sees a generic
/// page.
#[derive(Debug, Error)]
pub(super) enum AppError {
    /// Rendering a page failed.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// The random number generator failed.
    #[error(transparent)]
    Random(#[from] RandomError),
    /// A blocking task panicked.
    #[error("a background task failed")]
    Task,
}

impl IntoResponse for AppError {
    /// Logs the error and returns the generic error page.
    fn into_response(self) -> Response {
        error!("request failed: {self}");
        (StatusCode::INTERNAL_SERVER_ERROR, Html(INTERNAL_ERROR_PAGE)).into_response()
    }
}
