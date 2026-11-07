// SPDX-License-Identifier: MIT OR Apache-2.0

//! The access statistics page.

use crate::app::AppState;
use crate::web::AppError;
use crate::web::admin::AdminSession;
use axum::extract::State;
use axum::response::Html;

/// How many days the page shows.
const PERIOD_DAYS: i32 = 30;

/// Shows the statistics of the last [`PERIOD_DAYS`] days.
pub(super) async fn show(
    State(state): State<AppState>,
    admin: AdminSession,
) -> Result<Html<String>, AppError> {
    let days = state.stats.recent(PERIOD_DAYS);
    let html =
        state
            .renderer
            .render_stats(&admin.session.username, admin.session.csrf.as_str(), &days)?;
    Ok(Html(html))
}

#[cfg(test)]
mod tests {
    use crate::stats::Hit;
    use crate::test_support::TestApp;
    use axum::http::StatusCode;

    /// The statistics require a login, like the editor.
    #[tokio::test]
    async fn requires_login() {
        let app = TestApp::new();

        let response = app.get("/admin/statistik/").await;

        assert_eq!(response.status, StatusCode::SEE_OTHER);
    }

    /// The page shows today's counters and the totals per page.
    #[tokio::test]
    async fn shows_counters() {
        let app = TestApp::new();
        let session = app.login().await;
        for _ in 0..3 {
            app.state.stats.record(Hit::Page {
                path: "/kontakt/",
                bot: false,
            });
        }
        app.state.stats.record(Hit::Page {
            path: "/",
            bot: true,
        });

        let response = session.get("/admin/statistik/").await;

        let text = response.visible_text();
        assert_eq!(response.status, StatusCode::OK);
        assert!(text.contains("Seiten 3 Mal aufgerufen, dazu 1 Mal"));
        assert!(text.contains("Kontakt 3"));
        assert!(text.contains("Sa, 07.11.2026 3"));
    }
}
