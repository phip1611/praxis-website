// SPDX-License-Identifier: MIT OR Apache-2.0

//! Security headers for all responses.

use axum::http::{HeaderMap, HeaderName, HeaderValue, header};

/// Headers that are the same for every response of every deployment.
const STATIC_HEADERS: [(HeaderName, &str); 6] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::X_FRAME_OPTIONS, "DENY"),
    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    (
        HeaderName::from_static("permissions-policy"),
        "camera=(), geolocation=(), microphone=(), payment=()",
    ),
    (
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    ),
    // Isolates the site in its own browser process where possible.
    (HeaderName::from_static("origin-agent-cluster"), "?1"),
];

/// Which sites may embed a response.
const CROSS_ORIGIN_RESOURCE_POLICY: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");

/// The HSTS value. Subdomains aren't included, as they aren't part of this
/// service.
const HSTS: &str = "max-age=31536000";

/// The precomputed security headers of a deployment.
#[derive(Clone, Debug)]
pub(crate) struct SecurityHeaders {
    /// The value of `Content-Security-Policy`.
    content_security_policy: HeaderValue,
    /// Whether to send `Strict-Transport-Security`.
    strict_transport_security: bool,
}

impl SecurityHeaders {
    /// Creates the headers.
    pub(crate) fn new(https: bool) -> Self {
        let policy = content_security_policy(https);
        Self {
            content_security_policy: HeaderValue::from_str(&policy)
                .expect("the policy should be a valid header"),
            strict_transport_security: https,
        }
    }

    /// Adds the security headers to a response.
    ///
    /// The Content Security Policy only goes on HTML documents. Browsers show
    /// other content, such as an image opened directly, in viewers with inline
    /// styles, which the policy would break.
    ///
    /// Other sites may embed the public files, such as the business card or
    /// the logo, as the WordPress site allowed. HTML documents stay
    /// limited to this origin.
    pub(crate) fn apply(&self, headers: &mut HeaderMap) {
        for (name, value) in STATIC_HEADERS {
            headers.insert(name, HeaderValue::from_static(value));
        }
        let is_html = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/html"));
        headers.insert(
            CROSS_ORIGIN_RESOURCE_POLICY,
            HeaderValue::from_static(if is_html {
                "same-origin"
            } else {
                "cross-origin"
            }),
        );
        if is_html {
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                self.content_security_policy.clone(),
            );
        }
        if self.strict_transport_security {
            headers.insert(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static(HSTS),
            );
        }
    }
}

/// Builds the Content Security Policy.
///
/// Scripts and styles only load as files from this origin, without any inline
/// code. So even if HTML injection slipped through the sanitizer, it couldn't
/// execute scripts. Over HTTPS, browsers also upgrade any `http://` resource.
fn content_security_policy(https: bool) -> String {
    let upgrade = if https {
        "; upgrade-insecure-requests"
    } else {
        ""
    };
    format!(
        "default-src 'none'; \
         script-src 'self'; \
         connect-src 'self'; \
         img-src 'self'; \
         style-src 'self'; \
         base-uri 'none'; \
         form-action 'self'; \
         frame-ancestors 'none'{upgrade}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies the headers to the headers of an HTML response.
    fn applied(headers: &SecurityHeaders) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        headers.apply(&mut map);
        map
    }

    /// Returns the CSP of the header map as text.
    fn csp(map: &HeaderMap) -> &str {
        map[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .expect("CSP should be ASCII")
    }

    /// The default policy allows nothing inline and nothing external.
    #[test]
    fn default_policy_is_strict() {
        let map = applied(&SecurityHeaders::new(true));

        assert!(csp(&map).contains("default-src 'none'"));
        assert!(csp(&map).contains("script-src 'self';"));
        assert!(csp(&map).contains("frame-ancestors 'none'"));
        assert!(!csp(&map).contains("unsafe-inline"));
        assert!(!csp(&map).contains("unsafe-eval"));
    }

    /// HSTS and upgrading requests over plain HTTP would break local dev on
    /// the same host name, so they're only sent over HTTPS.
    #[test]
    fn https_only_directives() {
        let https = applied(&SecurityHeaders::new(true));
        let http = applied(&SecurityHeaders::new(false));

        assert!(https.contains_key(header::STRICT_TRANSPORT_SECURITY));
        assert!(csp(&https).ends_with("; upgrade-insecure-requests"));
        assert!(!http.contains_key(header::STRICT_TRANSPORT_SECURITY));
        assert!(!csp(&http).contains("upgrade-insecure-requests"));
    }

    /// Images and other non-HTML responses get no CSP, so that the
    /// browser's own viewers work when they are opened directly.
    #[test]
    fn csp_only_for_html() {
        let mut map = HeaderMap::new();
        map.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));

        SecurityHeaders::new(true).apply(&mut map);

        assert!(!map.contains_key(header::CONTENT_SECURITY_POLICY));
        assert_eq!(map[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }

    /// Other sites may embed public files like images, but not HTML pages.
    #[test]
    fn only_non_html_may_be_embedded_cross_origin() {
        let headers = SecurityHeaders::new(true);
        let mut image = HeaderMap::new();
        image.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));

        headers.apply(&mut image);

        assert_eq!(image[CROSS_ORIGIN_RESOURCE_POLICY], "cross-origin");
        assert_eq!(
            applied(&headers)[CROSS_ORIGIN_RESOURCE_POLICY],
            "same-origin"
        );
    }

    /// All hardening headers are present.
    #[test]
    fn applies_static_headers() {
        let map = applied(&SecurityHeaders::new(false));

        assert_eq!(map[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(map[header::X_FRAME_OPTIONS], "DENY");
        assert!(map.contains_key(header::REFERRER_POLICY));
        assert!(map.contains_key("permissions-policy"));
        assert!(map.contains_key("cross-origin-opener-policy"));
        assert!(map.contains_key("cross-origin-resource-policy"));
        assert_eq!(map["origin-agent-cluster"], "?1");
    }
}
