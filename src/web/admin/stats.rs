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

    /// An iPhone with Safari.
    const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

    /// A Windows computer with Edge.
    const WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0";

    /// The page shows today's counters and the totals per page.
    #[tokio::test]
    async fn shows_counters() {
        let app = TestApp::new();
        let session = app.login().await;
        for _ in 0..3 {
            app.state.stats.record(Hit::page("/kontakt/", Some(IPHONE)));
        }
        app.state
            .stats
            .record(Hit::page("/", Some("Googlebot/2.1")));

        let response = session.get("/admin/statistik/").await;

        let text = response.visible_text();
        assert_eq!(response.status, StatusCode::OK);
        assert!(text.contains("Seiten 3 Mal aufgerufen, dazu 1 Mal"));
        assert!(text.contains("Kontakt 3"));
        assert!(text.contains("Sa, 07.11.2026 3"));
    }

    /// The page shows the share of each device class, browser, and
    /// operating system, the most frequent first.
    #[tokio::test]
    async fn shows_clients() {
        let app = TestApp::new();
        let session = app.login().await;
        for _ in 0..3 {
            app.state.stats.record(Hit::page("/", Some(IPHONE)));
        }
        app.state.stats.record(Hit::page("/", Some(WINDOWS)));

        let text = session.get("/admin/statistik/").await.visible_text();

        assert!(text.contains("Smartphone 3 75 % Computer 1 25 % Tablet 0 0 %"));
        assert!(text.contains("Safari 3 75 % Edge 1 25 % Chrome 0 0 %"));
        assert!(text.contains("iOS/iPadOS 3 75 % Windows 1 25 % macOS 0 0 %"));
    }
}
