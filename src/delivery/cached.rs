// SPDX-License-Identifier: MIT OR Apache-2.0

//! A response with precomputed compressed variants and validators.

use crate::delivery::compression;
use crate::delivery::encoding::{ContentEncoding, negotiate};
use crate::hex;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use sha2::{Digest, Sha256};

/// The number of hex digits of a content hash in ETags and asset URLs. 64 bits
/// make accidental collisions practically impossible for a few dozen files.
const HASH_HEX_DIGITS: usize = 16;

/// How clients and proxies may cache a response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CachePolicy {
    /// Cache, but check for changes on every use. For pages, as the news
    /// must show up immediately after an edit.
    Revalidate,
    /// Cache forever. Only for URLs that contain a hash of the content.
    Immutable,
}

impl CachePolicy {
    /// Returns the `Cache-Control` value.
    const fn header_value(self) -> &'static str {
        match self {
            Self::Revalidate => "public, no-cache",
            Self::Immutable => "public, max-age=31536000, immutable",
        }
    }
}

/// Whether a response gets compressed variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Compression {
    /// Precompute Brotli and gzip variants. For text.
    Precompress,
    /// Only serve the body as is. For formats that are compressed already,
    /// such as images.
    Never,
}

/// One encoding of the body.
#[derive(Debug)]
struct Variant {
    /// The encoded body.
    body: Bytes,
    /// The strong validator, unique per content and encoding.
    etag: HeaderValue,
}

/// A precomputed response.
#[derive(Debug)]
pub(crate) struct CachedResponse {
    /// The status code.
    status: StatusCode,
    /// The `Content-Type` value.
    content_type: HeaderValue,
    /// The `Cache-Control` value.
    cache_control: HeaderValue,
    /// The uncompressed body.
    identity: Variant,
    /// The Brotli body, if it's smaller.
    brotli: Option<Variant>,
    /// The gzip body, if it's smaller.
    gzip: Option<Variant>,
}

impl CachedResponse {
    /// Creates a response and, depending on `compression`, its compressed
    /// variants. They're only kept if they're smaller.
    pub(crate) fn new(
        status: StatusCode,
        content_type: &'static str,
        policy: CachePolicy,
        body: Vec<u8>,
        compression: Compression,
    ) -> Self {
        let hash = content_hash(&body);
        let variant = |encoded: Vec<u8>, suffix: &str| Variant {
            body: Bytes::from(encoded),
            etag: HeaderValue::from_str(&format!("\"{hash}{suffix}\""))
                .expect("a hex hash should be a valid header"),
        };
        let compressed = |encoded: Vec<u8>, suffix: &str| {
            (encoded.len() < body.len()).then(|| variant(encoded, suffix))
        };
        let (brotli, gzip) = if compression == Compression::Precompress {
            (
                compressed(compression::brotli(&body), "-br"),
                compressed(compression::gzip(&body), "-gz"),
            )
        } else {
            (None, None)
        };
        Self {
            status,
            content_type: HeaderValue::from_static(content_type),
            cache_control: HeaderValue::from_static(policy.header_value()),
            identity: variant(body, ""),
            brotli,
            gzip,
        }
    }

    /// Returns the uncompressed body.
    #[cfg(test)]
    pub(super) fn body(&self) -> &[u8] {
        &self.identity.body
    }

    /// Builds the response for a request with the given headers.
    ///
    /// Picks the encoding and answers `304 Not Modified` if the client's copy
    /// is current. For `HEAD` requests, hyper drops the body but keeps the
    /// `Content-Length`.
    pub(crate) fn respond(&self, request: &HeaderMap) -> Response {
        let accept_encoding = request
            .get(header::ACCEPT_ENCODING)
            .and_then(|value| value.to_str().ok());
        let encoding = negotiate(accept_encoding, self.brotli.is_some(), self.gzip.is_some());
        let variant = match encoding {
            ContentEncoding::Brotli => self.brotli.as_ref(),
            ContentEncoding::Gzip => self.gzip.as_ref(),
            ContentEncoding::Identity => None,
        }
        .unwrap_or(&self.identity);

        let not_modified = self.status == StatusCode::OK
            && request
                .get(header::IF_NONE_MATCH)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| etag_matches(value, &variant.etag));

        let mut response = if not_modified {
            Response::new(Body::empty())
        } else {
            Response::new(Body::from(variant.body.clone()))
        };
        *response.status_mut() = if not_modified {
            StatusCode::NOT_MODIFIED
        } else {
            self.status
        };
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, self.content_type.clone());
        headers.insert(header::CACHE_CONTROL, self.cache_control.clone());
        headers.insert(header::ETAG, variant.etag.clone());
        if self.brotli.is_some() || self.gzip.is_some() {
            headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
        }
        if encoding != ContentEncoding::Identity {
            headers.insert(
                header::CONTENT_ENCODING,
                HeaderValue::from_static(encoding.token()),
            );
        }
        response
    }
}

/// Returns the first [`HASH_HEX_DIGITS`] hex digits of the SHA-256 of `data`.
pub(super) fn content_hash(data: &[u8]) -> String {
    hex::encode(&Sha256::digest(data)[..HASH_HEX_DIGITS / 2])
}

/// Checks if an `If-None-Match` header matches `etag`.
///
/// Uses the weak comparison of RFC 9110, section 13.1.2, as reverse proxies
/// may turn strong ETags into weak ones when they modify the body.
fn etag_matches(if_none_match: &str, etag: &HeaderValue) -> bool {
    let etag = etag.as_bytes();
    if_none_match
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/").as_bytes() == etag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    /// A compressible page of realistic size.
    fn page() -> CachedResponse {
        let body = "<p>Aktuelle Informationen</p>\n".repeat(100).into_bytes();
        CachedResponse::new(
            StatusCode::OK,
            "text/html; charset=utf-8",
            CachePolicy::Revalidate,
            body,
            Compression::Precompress,
        )
    }

    /// Builds request headers from name-value pairs.
    fn request(headers: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.insert(*name, HeaderValue::from_static(value));
        }
        map
    }

    /// Returns the response body.
    async fn body(response: Response) -> Bytes {
        response
            .into_body()
            .collect()
            .await
            .expect("body should be readable")
            .to_bytes()
    }

    /// Browsers get Brotli with the matching headers.
    #[tokio::test]
    async fn serves_brotli_to_browsers() {
        let page = page();

        let response = page.respond(&request(&[("accept-encoding", "gzip, br")]));

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_ENCODING], "br");
        assert_eq!(response.headers()[header::VARY], "accept-encoding");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, no-cache"
        );
        assert!(body(response).await.len() < page.body().len());
    }

    /// Clients without compression support get the plain body.
    #[tokio::test]
    async fn serves_identity_without_accept_encoding() {
        let page = page();

        let response = page.respond(&HeaderMap::new());

        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert_eq!(body(response).await, page.body());
    }

    /// Each encoding has its own ETag, or caches could mix them up.
    #[test]
    fn etags_differ_per_encoding() {
        let page = page();

        let brotli = page.respond(&request(&[("accept-encoding", "br")]));
        let gzip = page.respond(&request(&[("accept-encoding", "gzip")]));
        let identity = page.respond(&HeaderMap::new());

        let etags = [brotli, gzip, identity].map(|r| r.headers()[header::ETAG].clone());
        assert_ne!(etags[0], etags[1]);
        assert_ne!(etags[1], etags[2]);
    }

    /// A current cached copy is confirmed with an empty `304`.
    #[tokio::test]
    async fn answers_not_modified_for_matching_etag() {
        let page = page();
        let etag = page.respond(&HeaderMap::new()).headers()[header::ETAG].clone();
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);

        let response = page.respond(&headers);

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert!(response.headers().contains_key(header::ETAG));
        assert!(body(response).await.is_empty());
    }

    /// Weak ETags from proxies, lists, and `*` all count as matches.
    #[test]
    fn etag_matching_follows_weak_comparison() {
        let etag = HeaderValue::from_static("\"abc\"");

        assert!(etag_matches("\"abc\"", &etag));
        assert!(etag_matches("W/\"abc\"", &etag));
        assert!(etag_matches("\"x\", \"abc\"", &etag));
        assert!(etag_matches("*", &etag));
        assert!(!etag_matches("\"abcd\"", &etag));
        assert!(!etag_matches("abc", &etag));
    }

    /// Error pages are never answered with `304`, which would hide the error.
    #[test]
    fn never_answers_not_modified_for_errors() {
        let page = CachedResponse::new(
            StatusCode::NOT_FOUND,
            "text/html; charset=utf-8",
            CachePolicy::Revalidate,
            b"<p>nicht gefunden</p>".to_vec(),
            Compression::Precompress,
        );

        let response = page.respond(&request(&[("if-none-match", "*")]));

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// Images don't get compressed variants or a `Vary` header.
    #[test]
    fn incompressible_bodies_have_only_identity() {
        let image = CachedResponse::new(
            StatusCode::OK,
            "image/png",
            CachePolicy::Immutable,
            vec![0; 1000],
            Compression::Never,
        );

        let response = image.respond(&request(&[("accept-encoding", "br")]));

        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert!(!response.headers().contains_key(header::VARY));
        assert!(
            response.headers()[header::CACHE_CONTROL]
                .to_str()
                .is_ok_and(|v| v.contains("immutable"))
        );
    }

    /// Tiny bodies would grow by compression, so the variant is dropped.
    #[test]
    fn drops_variants_that_are_not_smaller() {
        let tiny = CachedResponse::new(
            StatusCode::OK,
            "text/plain",
            CachePolicy::Revalidate,
            b"a".to_vec(),
            Compression::Precompress,
        );

        assert!(tiny.brotli.is_none());
        assert!(tiny.gzip.is_none());
    }

    /// The hash has the documented length and is stable.
    #[test]
    fn content_hash_is_stable_prefix_of_sha256() {
        assert_eq!(content_hash(b"abc"), "ba7816bf8f01cfea");
    }
}
