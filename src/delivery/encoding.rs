// SPDX-License-Identifier: MIT OR Apache-2.0

//! Content negotiation for `Accept-Encoding`.

/// A content coding that the service can send.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContentEncoding {
    /// Brotli, the smallest for text.
    Brotli,
    /// Gzip, supported by every client.
    Gzip,
    /// Uncompressed.
    Identity,
}

impl ContentEncoding {
    /// Returns the token for the `Content-Encoding` header.
    pub(super) const fn token(self) -> &'static str {
        match self {
            Self::Brotli => "br",
            Self::Gzip => "gzip",
            Self::Identity => "identity",
        }
    }
}

/// Picks the best available encoding that the client accepts.
///
/// Follows RFC 9110, section 12.5.3: the highest quality value wins, `q=0`
/// means "not acceptable", and `*` covers codings that aren't listed. On a tie,
/// Brotli wins, as it's smaller.
pub(super) fn negotiate(
    accept_encoding: Option<&str>,
    brotli: bool,
    gzip: bool,
) -> ContentEncoding {
    let Some(header) = accept_encoding else {
        return ContentEncoding::Identity;
    };
    let candidates = [
        (ContentEncoding::Brotli, brotli),
        (ContentEncoding::Gzip, gzip),
    ];
    let mut best = (ContentEncoding::Identity, 0.0);
    for (encoding, available) in candidates {
        let quality = quality(header, encoding.token());
        if available && quality > best.1 {
            best = (encoding, quality);
        }
    }
    best.0
}

/// Returns the quality value of `coding` in the header, `0.0` if not accepted.
fn quality(header: &str, coding: &str) -> f32 {
    let mut wildcard = None;
    for entry in header.split(',') {
        let mut parts = entry.split(';');
        let name = parts.next().unwrap_or_default().trim();
        let q = parts
            .filter_map(|param| param.trim().strip_prefix("q="))
            .find_map(|value| value.trim().parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        if name.eq_ignore_ascii_case(coding) {
            return q;
        }
        if name == "*" {
            wildcard = Some(q);
        }
    }
    wildcard.unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shorthand for negotiating with both compressed variants available.
    fn pick(header: &str) -> ContentEncoding {
        negotiate(Some(header), true, true)
    }

    /// Modern browsers get Brotli.
    #[test]
    fn prefers_brotli_from_browsers() {
        assert_eq!(pick("gzip, deflate, br, zstd"), ContentEncoding::Brotli);
    }

    /// Clients without Brotli get gzip, others the identity.
    #[test]
    fn falls_back_to_gzip_and_identity() {
        assert_eq!(pick("gzip, deflate"), ContentEncoding::Gzip);
        assert_eq!(pick("deflate"), ContentEncoding::Identity);
        assert_eq!(negotiate(None, true, true), ContentEncoding::Identity);
    }

    /// Quality values decide, and `q=0` excludes a coding.
    #[test]
    fn respects_quality_values() {
        assert_eq!(pick("br;q=0.5, gzip;q=0.8"), ContentEncoding::Gzip);
        assert_eq!(pick("br;q=0, gzip"), ContentEncoding::Gzip);
        assert_eq!(pick("br;q=0, gzip;q=0"), ContentEncoding::Identity);
    }

    /// The wildcard covers unlisted codings, but not excluded ones.
    #[test]
    fn handles_wildcard() {
        assert_eq!(pick("*"), ContentEncoding::Brotli);
        assert_eq!(pick("br;q=0, *"), ContentEncoding::Gzip);
        assert_eq!(pick("*;q=0"), ContentEncoding::Identity);
    }

    /// Variants that aren't available can't be picked.
    #[test]
    fn only_picks_available_variants() {
        assert_eq!(
            negotiate(Some("br, gzip"), false, true),
            ContentEncoding::Gzip
        );
        assert_eq!(
            negotiate(Some("br, gzip"), false, false),
            ContentEncoding::Identity
        );
    }

    /// Tokens are case-insensitive, and odd spacing is fine.
    #[test]
    fn tolerates_case_and_whitespace() {
        assert_eq!(pick(" GZIP ; q=1 "), ContentEncoding::Gzip);
        assert_eq!(pick("Br"), ContentEncoding::Brotli);
    }

    /// Broken quality values count as the default, not as an error.
    #[test]
    fn tolerates_invalid_quality() {
        assert_eq!(pick("br;q=abc"), ContentEncoding::Brotli);
        assert_eq!(pick("br;q=7"), ContentEncoding::Brotli);
    }
}
