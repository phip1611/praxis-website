// SPDX-License-Identifier: MIT OR Apache-2.0

//! The whole service in memory, driven through its router.

use crate::app::AppState;
use crate::cli::{DEFAULT_BIND, DevArgs};
use crate::config::Config;
use crate::security::credentials::CredentialStore;
use crate::test_support::{
    MemoryStore, MockClock, TEST_BASE_URL, TEST_PASSWORD, TEST_USER, accounts_file, test_config,
};
use crate::web;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::fmt::Write;
use std::str;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// The service with mocked clock and store, and the test account.
#[derive(Debug)]
pub(crate) struct TestApp {
    /// The shared state, for checks on the internals.
    pub(crate) state: AppState,
    /// The router under test.
    router: Router,
    /// The clock of the service.
    pub(crate) clock: Arc<MockClock>,
    /// The store of the service.
    pub(crate) store: Arc<MemoryStore>,
    /// The state directory, e.g., for the statistics.
    _state_dir: TempDir,
}

/// A response with the body already read.
#[derive(Debug)]
pub(crate) struct TestResponse {
    /// The status code.
    pub(crate) status: StatusCode,
    /// The headers.
    pub(crate) headers: HeaderMap,
    /// The body.
    body: Bytes,
}

impl TestResponse {
    /// Returns the body as text.
    ///
    /// # Panics
    ///
    /// Panics if the body isn't UTF-8.
    pub(crate) fn text(&self) -> &str {
        str::from_utf8(&self.body).expect("the body should be UTF-8")
    }

    /// Returns the text of an HTML body without tags, with whitespace
    /// collapsed and entities decoded. Lets tests check content independent
    /// of markup and escaping details.
    ///
    /// Only for HTML that askama produced: it escapes `<` and `>` in text
    /// and attributes, so every `<` starts a tag.
    pub(crate) fn visible_text(&self) -> String {
        let mut text = String::new();
        let mut in_tag = false;
        for c in self.text().chars() {
            match c {
                '<' => {
                    in_tag = true;
                    text.push(' ');
                }
                '>' => in_tag = false,
                _ if !in_tag => text.push(c),
                _ => {}
            }
        }
        // `&#38;` has to be last, or "&#38;#60;" would become "<".
        let text = text
            .replace("&#60;", "<")
            .replace("&#62;", ">")
            .replace("&#34;", "\"")
            .replace("&#39;", "'")
            .replace("&#38;", "&");
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Returns the value of header `name`.
    ///
    /// # Panics
    ///
    /// Panics if the header is missing.
    pub(crate) fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .unwrap_or_else(|| panic!("response should have header {name}"))
            .to_str()
            .expect("the header should be ASCII")
    }
}

impl TestApp {
    /// Creates the service with the production configuration of the test site.
    pub(crate) fn new() -> Self {
        Self::with_config(test_config())
    }

    /// Creates the service in development mode.
    pub(crate) fn dev() -> Self {
        Self::with_config(Config::dev(DevArgs { bind: DEFAULT_BIND }))
    }

    /// Creates the service with `config`.
    fn with_config(mut config: Config) -> Self {
        let state_dir = TempDir::new().expect("temp dir should be created");
        config.state_dir = state_dir.path().to_path_buf();
        let clock = Arc::new(MockClock::default());
        let store = Arc::new(MemoryStore::default());
        let credentials = CredentialStore::parse(&accounts_file(&[(TEST_USER, TEST_PASSWORD)]))
            .expect("the test accounts should be valid");
        let state = AppState::new(
            config,
            Arc::<MockClock>::clone(&clock),
            Arc::<MemoryStore>::clone(&store),
            credentials,
        )
        .expect("the service should start");
        Self {
            router: web::router(state.clone()),
            state,
            clock,
            store,
            _state_dir: state_dir,
        }
    }

    /// Sends a request with the given method, path, and headers.
    pub(crate) async fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
    ) -> TestResponse {
        self.send(method, path, headers, Body::empty()).await
    }

    /// Sends a `GET` request.
    pub(crate) async fn get(&self, path: &str) -> TestResponse {
        self.request("GET", path, &[]).await
    }

    /// Sends a `GET` request with extra headers.
    pub(crate) async fn get_with(&self, path: &str, headers: &[(&str, &str)]) -> TestResponse {
        self.request("GET", path, headers).await
    }

    /// Submits a form like a browser on the site would.
    pub(crate) async fn post_form(
        &self,
        path: &str,
        fields: &[(&str, &str)],
        cookie: Option<&str>,
    ) -> TestResponse {
        self.post_form_from(TEST_BASE_URL, path, fields, cookie)
            .await
    }

    /// Submits a form like a browser on `origin` would.
    pub(crate) async fn post_form_from(
        &self,
        origin: &str,
        path: &str,
        fields: &[(&str, &str)],
        cookie: Option<&str>,
    ) -> TestResponse {
        let cookie = cookie.map(|cookie| ("cookie", cookie));
        self.post_form_with(origin, path, fields, cookie.as_slice())
            .await
    }

    /// Submits the login form from the client with address `client_ip`, as
    /// the reverse proxy would report it.
    pub(crate) async fn login_from(&self, client_ip: &str, password: &str) -> TestResponse {
        self.post_form_with(
            TEST_BASE_URL,
            "/admin/login/",
            &[("username", TEST_USER), ("password", password)],
            &[("x-real-ip", client_ip)],
        )
        .await
    }

    /// Submits a form from `origin` with extra headers.
    async fn post_form_with(
        &self,
        origin: &str,
        path: &str,
        fields: &[(&str, &str)],
        extra_headers: &[(&str, &str)],
    ) -> TestResponse {
        let mut headers = vec![
            ("origin", origin),
            ("content-type", "application/x-www-form-urlencoded"),
        ];
        headers.extend_from_slice(extra_headers);
        self.send("POST", path, &headers, Body::from(encode_form(fields)))
            .await
    }

    /// Logs in with the test account.
    ///
    /// # Panics
    ///
    /// Panics if the login fails.
    pub(crate) async fn login(&self) -> LoggedIn<'_> {
        let response = self
            .post_form(
                "/admin/login/",
                &[("username", TEST_USER), ("password", TEST_PASSWORD)],
                None,
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::SEE_OTHER,
            "login should succeed"
        );
        let cookie = response
            .header("set-cookie")
            .split(';')
            .next()
            .expect("the cookie should have a value")
            .to_string();
        let editor = self.get_with("/admin/", &[("cookie", &cookie)]).await;
        let csrf = extract_hidden_field(editor.text(), "csrf");
        LoggedIn {
            app: self,
            cookie,
            csrf,
        }
    }

    /// Sends a request through the router.
    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Body,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request.body(body).expect("the request should be valid");
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("the router should not fail");
        let (parts, body) = response.into_parts();
        TestResponse {
            status: parts.status,
            headers: parts.headers,
            body: body
                .collect()
                .await
                .expect("the body should be readable")
                .to_bytes(),
        }
    }
}

/// A logged-in browser session.
#[derive(Debug)]
pub(crate) struct LoggedIn<'a> {
    /// The service.
    app: &'a TestApp,
    /// The `Cookie` header value.
    cookie: String,
    /// The CSRF token of the session.
    csrf: String,
}

impl LoggedIn<'_> {
    /// Returns the CSRF token of the session.
    pub(crate) fn csrf(&self) -> &str {
        &self.csrf
    }

    /// Sends a `GET` request with the session cookie.
    pub(crate) async fn get(&self, path: &str) -> TestResponse {
        self.app.get_with(path, &[("cookie", &self.cookie)]).await
    }

    /// Submits a form with the session cookie.
    pub(crate) async fn post(&self, path: &str, fields: &[(&str, &str)]) -> TestResponse {
        self.app.post_form(path, fields, Some(&self.cookie)).await
    }

    /// Submits a form with the session cookie from another origin.
    pub(crate) async fn post_from(
        &self,
        origin: &str,
        path: &str,
        fields: &[(&str, &str)],
    ) -> TestResponse {
        self.app
            .post_form_from(origin, path, fields, Some(&self.cookie))
            .await
    }

    /// Saves new news content, based on the current revision.
    pub(crate) async fn save_news(&self, html: &str) -> TestResponse {
        let revision = self.app.state.news.lock().await.revision.to_string();
        self.post(
            "/admin/",
            &[
                ("csrf", &self.csrf),
                ("revision", &revision),
                ("html", html),
            ],
        )
        .await
    }
}

/// Encodes form fields as `application/x-www-form-urlencoded`.
fn encode_form(fields: &[(&str, &str)]) -> String {
    let encode = |text: &str| {
        let mut encoded = String::new();
        for byte in text.bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                encoded.push(char::from(byte));
            } else {
                write!(encoded, "%{byte:02X}").expect("writing to a string should succeed");
            }
        }
        encoded
    };
    fields
        .iter()
        .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Returns the `value` of the input element named `name` in `html`,
/// regardless of the attribute order.
fn extract_hidden_field(html: &str, name: &str) -> String {
    let name_attribute = format!("name=\"{name}\"");
    let position = html
        .find(&name_attribute)
        .unwrap_or_else(|| panic!("the page should have the field {name}"));
    let tag_start = html[..position]
        .rfind('<')
        .expect("the field should be in a tag");
    let tag_end = position + html[position..].find('>').expect("the tag should end");
    let tag = &html[tag_start..tag_end];
    let value_start = tag.find("value=\"").expect("the field should have a value") + 7;
    let value_end = value_start
        + tag[value_start..]
            .find('"')
            .expect("the value should be quoted");
    tag[value_start..value_end].to_string()
}
