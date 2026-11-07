// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serves the precomputed public pages.

use crate::app::AppState;
use crate::stats::bots::is_bot;
use crate::stats::{self, Hit};
use crate::web::legacy::{self, LegacyTarget};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

/// Answers all requests that no other route handles.
///
/// The order: current pages, old WordPress URLs, the missing trailing slash
/// (WordPress redirected these as well), and finally the 404 page.
pub(super) async fn serve(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, HeaderValue::from_static("GET, HEAD"))],
        )
            .into_response();
    }
    let path = uri.path();
    let snapshot = &state.site;
    if let Some(page) = snapshot.get(path) {
        if method == Method::GET && stats::is_tracked(path) {
            let user_agent = headers
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok());
            state.stats.record(Hit::Page {
                path,
                bot: is_bot(user_agent),
            });
        }
        return page.respond(&headers);
    }
    match legacy::resolve(path) {
        Some(LegacyTarget::Moved(target)) => return moved_permanently(target),
        Some(LegacyTarget::Gone) => return StatusCode::GONE.into_response(),
        None => {}
    }
    if !path.ends_with('/') {
        let with_slash = format!("{path}/");
        if snapshot.get(&with_slash).is_some() {
            return moved_permanently(&with_slash);
        }
    }
    if method == Method::GET {
        state.stats.record(Hit::NotFound);
    }
    snapshot.not_found().respond(&headers)
}

/// Returns a `301 Moved Permanently` to `target`.
///
/// Not `308`, as `301` is what search engines and the WordPress site used.
/// The query string is dropped, as no current page uses one.
fn moved_permanently(target: &str) -> Response {
    let location = HeaderValue::from_str(target).expect("redirect targets should be valid paths");
    (
        StatusCode::MOVED_PERMANENTLY,
        [(header::LOCATION, location)],
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use crate::content::pages::HOME_PATH;
    use crate::content::site::SITE;
    use crate::test_support::{SEED_MARKER, TestApp};
    use axum::http::StatusCode;
    use std::iter;

    /// All public pages are reachable and have the right type.
    #[tokio::test]
    async fn serves_all_pages() {
        let app = TestApp::new();

        let paths = iter::once(HOME_PATH).chain(SITE.pages.iter().map(|page| page.path.as_str()));
        for path in paths {
            let response = app.get(path).await;

            assert_eq!(response.status, StatusCode::OK, "{path}");
            assert_eq!(
                response.header("content-type"),
                "text/html; charset=utf-8",
                "{path}"
            );
        }
    }

    /// The start page shows the current news.
    #[tokio::test]
    async fn home_shows_news() {
        let app = TestApp::new();

        let response = app.get("/").await;

        assert!(response.text().contains(SEED_MARKER));
    }

    /// Browsers get the precompressed variant.
    #[tokio::test]
    async fn serves_compressed_pages() {
        let app = TestApp::new();

        let response = app
            .get_with("/kontakt/", &[("accept-encoding", "br, gzip")])
            .await;

        assert_eq!(response.header("content-encoding"), "br");
        assert_eq!(response.header("vary"), "accept-encoding");
    }

    /// Revalidation with the ETag answers `304`.
    #[tokio::test]
    async fn answers_not_modified() {
        let app = TestApp::new();
        let etag = app.get("/").await.header("etag").to_string();

        let response = app.get_with("/", &[("if-none-match", &etag)]).await;

        assert_eq!(response.status, StatusCode::NOT_MODIFIED);
    }

    /// `HEAD` has the headers of `GET` without the body.
    #[tokio::test]
    async fn head_has_headers_only() {
        let app = TestApp::new();

        let response = app.request("HEAD", "/kontakt/", &[]).await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.header("content-type"), "text/html; charset=utf-8");
    }

    /// Unknown paths get the 404 page, which links to the start page.
    #[tokio::test]
    async fn unknown_path_is_not_found() {
        let app = TestApp::new();

        let response = app.get("/gibt-es-nicht/").await;

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert!(response.text().contains("Seite nicht gefunden"));
        assert!(response.text().contains("noindex"));
    }

    /// Paths without trailing slash redirect, as on the WordPress site.
    #[tokio::test]
    async fn adds_missing_trailing_slash() {
        let app = TestApp::new();

        let response = app.get("/kontakt").await;

        assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
        assert_eq!(response.header("location"), "/kontakt/");
    }

    /// The indexed URL of the old news post leads to the start page.
    #[tokio::test]
    async fn redirects_old_news_url() {
        let app = TestApp::new();

        let response = app.get("/2026/10/aktuelle-informationen/").await;

        assert_eq!(response.status, StatusCode::MOVED_PERMANENTLY);
        assert_eq!(response.header("location"), "/");
    }

    /// Old image URLs keep working without redirect.
    #[tokio::test]
    async fn serves_old_image_urls() {
        let app = TestApp::new();

        let response = app
            .get("/wp-content/uploads/2015/09/Visitenkarte-Arztpraxis-Dirk-Schuster.png")
            .await;

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.header("content-type"), "image/png");
        assert_eq!(response.header("cache-control"), "public, no-cache");
    }

    /// Hashed asset URLs may be cached forever.
    #[tokio::test]
    async fn assets_are_immutable() {
        let app = TestApp::new();
        let url = app.state.renderer.assets().url("style.css").to_string();

        let response = app.get(&url).await;

        assert_eq!(response.status, StatusCode::OK);
        assert!(response.header("cache-control").contains("immutable"));
    }

    /// Only reading methods are allowed on public pages.
    #[tokio::test]
    async fn rejects_other_methods() {
        let app = TestApp::new();

        let response = app.request("POST", "/", &[]).await;

        assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.header("allow"), "GET, HEAD");
    }

    /// The query string doesn't change the page, e.g., old `?p=258` links.
    #[tokio::test]
    async fn ignores_query_strings() {
        let app = TestApp::new();

        let response = app.get("/?p=258").await;

        assert_eq!(response.status, StatusCode::OK);
        assert!(response.text().contains("Aktuelle Informationen"));
    }

    /// Page views are counted per page, crawlers and errors separately, and
    /// assets not at all.
    #[tokio::test]
    async fn counts_page_views() {
        let app = TestApp::new();
        let browser = [(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64) Firefox/145.0",
        )];

        app.get_with("/kontakt/", &browser).await;
        app.get_with("/kontakt/", &[("user-agent", "Googlebot/2.1")])
            .await;
        app.get_with("/gibt-es-nicht/", &browser).await;
        app.get_with("/robots.txt", &browser).await;
        app.request("HEAD", "/kontakt/", &browser).await;

        let today = app.state.stats.day(app.state.stats.today());
        assert_eq!(today.views.get("/kontakt/"), Some(&1));
        assert_eq!(today.total_views(), 1);
        assert_eq!(today.bot_views, 1);
        assert_eq!(today.not_found, 1);
    }

    /// Every response has the security headers, including errors and
    /// redirects. Only HTML pages have a CSP, see `security::headers`.
    #[tokio::test]
    async fn all_responses_have_security_headers() {
        let app = TestApp::new();

        for (path, html) in [
            ("/", true),
            ("/gibt-es-nicht", true),
            ("/kontakt", false),
            ("/favicon.ico", false),
        ] {
            let response = app.get(path).await;

            let csp = response.headers.get("content-security-policy");
            assert_eq!(csp.is_some(), html, "{path}");
            assert_eq!(
                response.header("x-content-type-options"),
                "nosniff",
                "{path}"
            );
            assert_eq!(
                response.header("strict-transport-security"),
                "max-age=31536000",
                "{path}"
            );
        }
    }
}
