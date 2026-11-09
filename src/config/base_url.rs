// SPDX-License-Identifier: MIT OR Apache-2.0

//! The public URL of the site.

use thiserror::Error;
use url::{ParseError, Url};

/// Why a URL can't be used as base URL.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum BaseUrlError {
    /// The URL doesn't parse.
    #[error("not a valid URL: {0}")]
    Invalid(#[from] ParseError),
    /// The scheme isn't `http` or `https`.
    #[error("the URL must start with http:// or https://")]
    Scheme,
    /// The URL has credentials, a query, or a fragment.
    #[error("the URL must not contain credentials, a query, or a fragment")]
    ExtraParts,
    /// The URL has a path.
    #[error("the URL must not contain a path")]
    Path,
}

/// An absolute `http(s)` URL without path, such as `https://example.com`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaseUrl {
    /// The normalized `scheme://authority` form.
    origin: String,
}

impl BaseUrl {
    /// Parses and normalizes a URL without path (a trailing `/` is fine).
    ///
    /// # Errors
    ///
    /// Returns [`BaseUrlError`] if the URL isn't a plain origin.
    pub(crate) fn parse(url: &str) -> Result<Self, BaseUrlError> {
        let (origin, path) = parse_origin_and_path(url)?;
        if path != "/" {
            return Err(BaseUrlError::Path);
        }
        Ok(Self { origin })
    }

    /// Returns the origin, e.g., `https://example.com`.
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    /// Returns the absolute URL of `path`, which must start with `/`.
    pub(crate) fn join(&self, path: &str) -> String {
        debug_assert!(path.starts_with('/'), "paths should be absolute");
        format!("{}{path}", self.origin)
    }

    /// Returns whether the site is served over HTTPS.
    pub(crate) fn is_https(&self) -> bool {
        self.origin.starts_with("https://")
    }
}

/// Splits an absolute `http(s)` URL into its normalized origin and its path.
///
/// The path always ends with `/`, so that relative file names can be appended.
///
/// # Errors
///
/// Returns [`BaseUrlError`] if the URL isn't an absolute `http(s)` URL or if it
/// has credentials, a query, or a fragment.
fn parse_origin_and_path(url: &str) -> Result<(String, String), BaseUrlError> {
    let url = Url::parse(url)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(BaseUrlError::Scheme);
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(BaseUrlError::ExtraParts);
    }
    let mut path = url.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    Ok((url.origin().ascii_serialization(), path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Host names are case-insensitive, a trailing slash is optional, and
    /// default ports aren't part of the origin.
    #[test]
    fn parse_normalizes_origin() {
        for url in [
            "https://Praxis.Test",
            "https://praxis.test/",
            "https://praxis.test:443",
        ] {
            let base = BaseUrl::parse(url).expect("URL should be valid");

            assert_eq!(base.origin(), "https://praxis.test");
        }
    }

    /// Ports are kept, as they are part of the origin.
    #[test]
    fn parse_keeps_port() {
        let base = BaseUrl::parse("http://localhost:8080").expect("URL should be valid");

        assert_eq!(base.origin(), "http://localhost:8080");
        assert!(!base.is_https());
    }

    /// `join` creates absolute URLs for canonical links and the sitemap.
    #[test]
    fn join_appends_path() {
        let base = BaseUrl::parse("https://praxis.test").expect("URL should be valid");

        assert_eq!(base.join("/kontakt/"), "https://praxis.test/kontakt/");
        assert!(base.is_https());
    }

    /// Everything but a plain origin is rejected.
    #[test]
    fn parse_rejects_non_origins() {
        assert_eq!(
            BaseUrl::parse("praxis.test"),
            Err(BaseUrlError::Invalid(ParseError::RelativeUrlWithoutBase))
        );
        assert_eq!(
            BaseUrl::parse("ftp://praxis.test"),
            Err(BaseUrlError::Scheme)
        );
        assert_eq!(
            BaseUrl::parse("https://praxis.test/sub/"),
            Err(BaseUrlError::Path)
        );
        assert_eq!(
            BaseUrl::parse("https://u:p@praxis.test"),
            Err(BaseUrlError::ExtraParts)
        );
        assert_eq!(
            BaseUrl::parse("https://praxis.test/?a=b"),
            Err(BaseUrlError::ExtraParts)
        );
        assert_eq!(
            BaseUrl::parse("https://praxis.test/#a"),
            Err(BaseUrlError::ExtraParts)
        );
        assert_eq!(
            BaseUrl::parse(""),
            Err(BaseUrlError::Invalid(ParseError::RelativeUrlWithoutBase))
        );
    }

    /// Paths are returned with a trailing slash.
    #[test]
    fn parse_origin_and_path_normalizes_path() {
        let (origin, path) =
            parse_origin_and_path("https://stats.test/matomo").expect("URL should be valid");

        assert_eq!(origin, "https://stats.test");
        assert_eq!(path, "/matomo/");
    }
}
