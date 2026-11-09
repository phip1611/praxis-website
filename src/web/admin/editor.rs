// SPDX-License-Identifier: MIT OR Apache-2.0

//! Editing the news on the start page.

use crate::app::AppState;
use crate::content::news::NewsDocument;
use crate::render::EditorView;
use crate::render::templates::{Notice, NoticeKind};
use crate::security::sanitize::SanitizedHtml;
use crate::web::AppError;
use crate::web::admin::forms::{CsrfProtected, SessionForm};
use crate::web::admin::{AdminSession, EDITOR_PATH};
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::OwnedMutexGuard;
use tokio::task;
use tracing::{error, info, warn};

/// The query string after a successful save.
const SAVED_QUERY: &str = "gespeichert";

/// The maximum size of the sanitized news. About 30 pages of text, so only
/// pasted garbage can reach it.
const MAX_NEWS_BYTES: usize = 64 * 1024;

/// The submitted editor form.
#[derive(Debug, Deserialize)]
pub(super) struct NewsForm {
    /// The CSRF token of the session.
    csrf: String,
    /// The revision that the edit is based on.
    revision: u64,
    /// The new content.
    html: String,
}

impl CsrfProtected for NewsForm {
    /// Returns the submitted CSRF token.
    fn csrf(&self) -> &str {
        &self.csrf
    }
}

/// Shows the editor.
pub(super) async fn show(
    State(state): State<AppState>,
    admin: AdminSession,
    uri: Uri,
) -> Result<Response, AppError> {
    let notice = (uri.query() == Some(SAVED_QUERY)).then(|| Notice {
        kind: NoticeKind::Success,
        text: "Gespeichert. Die Startseite zeigt jetzt den neuen Text.".to_string(),
    });
    let stored = state.news.lock().await.clone();
    let html = state.renderer.render_editor(&EditorView {
        username: &admin.session.username,
        html: &stored.html,
        stored: &stored,
        csrf: admin.session.csrf.as_str(),
        notice,
    })?;
    Ok(Html(html).into_response())
}

/// Sanitizes, stores, and publishes new content.
#[expect(
    clippy::significant_drop_tightening,
    reason = "false positive: the news lock moves into the publish task"
)]
pub(super) async fn save(
    State(state): State<AppState>,
    SessionForm { admin, form }: SessionForm<NewsForm>,
) -> Result<Response, AppError> {
    let html = SanitizedHtml::sanitize(&form.html);

    let stored = Arc::clone(&state.news).lock_owned().await;
    let rejection = if html.len() > MAX_NEWS_BYTES {
        Some((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Der Text ist zu lang und wurde nicht gespeichert.",
        ))
    } else if form.revision != stored.revision {
        Some((
            StatusCode::CONFLICT,
            "Der Text wurde inzwischen von jemand anderem geändert. Ihre Fassung steht \
             unten. Erneutes Speichern überschreibt die andere Änderung.",
        ))
    } else {
        None
    };
    if let Some((status, message)) = rejection {
        warn!(account = admin.session.username, %status, "save rejected");
        return render_with_error(&state, &admin, &html, &stored, status, message);
    }

    let next = stored.revise(html, &admin.session.username, state.clock.now());
    let submitted = next.html.clone();
    // A separate task, so that a cancelled request (client disconnect or
    // timeout) can't stop a save halfway.
    let saved = tokio::spawn(publish(state.clone(), stored, next))
        .await
        .map_err(|_| AppError::Task)?;
    if let Err(err) = saved {
        error!(account = admin.session.username, "save failed: {err}");
        let stored = state.news.lock().await.clone();
        return render_with_error(
            &state,
            &admin,
            &submitted,
            &stored,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Der Text konnte wegen eines technischen Fehlers nicht gespeichert werden. \
             Die Startseite ist unverändert. Bitte versuchen Sie es später erneut.",
        );
    }
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, format!("{EDITOR_PATH}?{SAVED_QUERY}"))],
    )
        .into_response())
}

/// Renders, stores, and publishes the revision `next`.
///
/// The order matters: the snapshot is rendered first and the file written
/// second, so that a failure in either leaves the site unchanged. Holding the
/// news lock throughout serializes saves.
async fn publish(
    state: AppState,
    mut stored: OwnedMutexGuard<NewsDocument>,
    next: NewsDocument,
) -> Result<(), AppError> {
    let (renderer, store, document) = (
        Arc::clone(&state.renderer),
        Arc::clone(&state.store),
        next.clone(),
    );
    let snapshot = task::spawn_blocking(move || {
        let snapshot = renderer.render_site(&document)?;
        store.save(&document)?;
        Ok::<_, AppError>(snapshot)
    })
    .await
    .map_err(|_| AppError::Task)??;
    state.site.publish(snapshot);
    info!(
        account = next.modified_by,
        revision = next.revision,
        "news updated"
    );
    *stored = next;
    Ok(())
}

/// Shows the editor again with the submitted content and an error.
fn render_with_error(
    state: &AppState,
    admin: &AdminSession,
    html: &SanitizedHtml,
    stored: &NewsDocument,
    status: StatusCode,
    message: &str,
) -> Result<Response, AppError> {
    let page = state.renderer.render_editor(&EditorView {
        username: &admin.session.username,
        html,
        stored,
        csrf: admin.session.csrf.as_str(),
        notice: Some(Notice {
            kind: NoticeKind::Error,
            text: message.to_string(),
        }),
    })?;
    Ok((status, Html(page)).into_response())
}

#[cfg(test)]
mod tests {
    use crate::content::store::NewsStore;
    use crate::test_support::{SEED_MARKER, TEST_USER, TestApp};
    use axum::http::StatusCode;
    use std::time::Duration;
    use tokio::time::timeout;

    /// The editor shows the current content and the session's CSRF token.
    #[tokio::test]
    async fn editor_shows_current_content() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session.get("/admin/").await;

        assert_eq!(response.status, StatusCode::OK);
        assert!(response.text().contains(SEED_MARKER));
        assert!(response.text().contains(session.csrf()));
        assert!(
            response
                .text()
                .contains(&format!("Angemeldet als <strong>{TEST_USER}"))
        );
    }

    /// Saving publishes the text on the start page and stores it.
    #[tokio::test]
    async fn save_publishes_and_stores() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session.save_news("<p>Praxis im Urlaub</p>").await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/?gespeichert");
        assert!(
            app.get("/")
                .await
                .text()
                .contains("<p>Praxis im Urlaub</p>")
        );
        let stored = app
            .store
            .load()
            .expect("load should work")
            .expect("should be saved");
        assert_eq!(stored.revision, 2);
        assert_eq!(stored.modified_by, TEST_USER);
    }

    /// After saving, the editor confirms success.
    #[tokio::test]
    async fn editor_confirms_save() {
        let app = TestApp::new();
        let session = app.login().await;
        session.save_news("<p>neu</p>").await;

        let response = session.get("/admin/?gespeichert").await;

        assert!(response.text().contains("Gespeichert."));
    }

    /// Scripts in the submitted content never reach visitors.
    #[tokio::test]
    async fn save_sanitizes_content() {
        let app = TestApp::new();
        let session = app.login().await;

        session
            .save_news("<p onclick=\"x()\">Hallo</p><script>alert(1)</script>")
            .await;

        let home = app.get("/").await;
        assert!(home.text().contains("<p>Hallo</p>"));
        assert!(!home.text().contains("alert(1)"));
        assert!(!home.text().contains("onclick"));
    }

    /// A forged form without the session's token changes nothing.
    #[tokio::test]
    async fn save_requires_csrf_token() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session
            .post(
                "/admin/",
                &[("csrf", "forged"), ("revision", "1"), ("html", "<p>x</p>")],
            )
            .await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert!(app.store.load().expect("load should work").is_none());
    }

    /// A request from another site changes nothing, even with a valid token.
    #[tokio::test]
    async fn save_rejects_foreign_origin() {
        let app = TestApp::new();
        let session = app.login().await;

        let response = session
            .post_from(
                "https://evil.test",
                "/admin/",
                &[
                    ("csrf", session.csrf()),
                    ("revision", "1"),
                    ("html", "<p>x</p>"),
                ],
            )
            .await;

        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }

    /// When two people edit at once, the second save doesn't silently
    /// overwrite the first, and the second text isn't lost either.
    #[tokio::test]
    async fn concurrent_edit_is_detected() {
        let app = TestApp::new();
        let first = app.login().await;
        let second = app.login().await;
        first.save_news("<p>erste Fassung</p>").await;

        let response = second
            .post(
                "/admin/",
                &[
                    ("csrf", second.csrf()),
                    ("revision", "1"),
                    ("html", "<p>zweite Fassung</p>"),
                ],
            )
            .await;

        assert_eq!(response.status, StatusCode::CONFLICT);
        assert!(response.text().contains("zweite Fassung"));
        assert!(response.text().contains("name=\"revision\" value=\"2\""));
        assert!(app.get("/").await.text().contains("erste Fassung"));
    }

    /// If the disk fails, the site stays unchanged and the text isn't lost.
    #[tokio::test]
    async fn failed_save_keeps_site_unchanged() {
        let app = TestApp::new();
        let session = app.login().await;
        app.store.fail_saves();

        let response = session.save_news("<p>neu</p>").await;

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        // The submitted text stays in the editor, so it isn't lost.
        assert!(response.visible_text().contains("<p>neu</p>"));
        assert!(app.get("/").await.text().contains(SEED_MARKER));
        assert_eq!(app.state.news.lock().await.revision, 1);
    }

    /// A save completes even if the client goes away in the middle, so that
    /// disk, memory, and the published site never disagree.
    #[tokio::test]
    async fn cancelled_save_still_completes() {
        let app = TestApp::new();
        let session = app.login().await;

        // Rendering takes far longer than 1 ms in tests, so this cancels the
        // request while the save is in progress.
        let cancelled = timeout(
            Duration::from_millis(1),
            session.save_news("<p>trotzdem gespeichert</p>"),
        )
        .await;

        cancelled.expect_err("the save should still run after 1 ms");
        assert_eq!(app.state.news.lock().await.revision, 2);
        assert!(app.get("/").await.text().contains("trotzdem gespeichert"));
    }

    /// Huge content is rejected and the site stays unchanged.
    #[tokio::test]
    async fn oversized_content_is_rejected() {
        let app = TestApp::new();
        let session = app.login().await;
        let huge = format!("<p>{}</p>", "x".repeat(70 * 1024));

        let response = session.save_news(&huge).await;

        assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(app.get("/").await.text().contains(SEED_MARKER));
    }

    /// After the idle timeout, saving requires a new login.
    #[tokio::test]
    async fn expired_session_cannot_save() {
        let app = TestApp::new();
        let session = app.login().await;
        app.clock.advance(jiff::SignedDuration::from_hours(2));

        let response = session.save_news("<p>neu</p>").await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/admin/login/");
    }
}
