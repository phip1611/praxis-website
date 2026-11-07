// SPDX-License-Identifier: MIT OR Apache-2.0

//! The whole service in memory, driven through its router.

use crate::app::AppState;
use crate::cli::{DEFAULT_BIND, DevArgs};
use crate::config::Config;
use crate::test_support::{MockClock, test_config};
use crate::web;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::str;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// The service with a mocked clock.
#[derive(Debug)]
pub(crate) struct TestApp {
    /// The shared state, for checks on the internals.
    pub(crate) state: AppState,
    /// The router under test.
    router: Router,
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
        let state = AppState::new(config, clock).expect("the service should start");
        Self {
            router: web::router(state.clone()),
            state,
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
