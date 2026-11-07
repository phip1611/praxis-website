// SPDX-License-Identifier: MIT OR Apache-2.0

//! The allowlist for user-provided HTML.
//!
//! [`SanitizedHtml`] is the only way to get user-provided HTML into a page, so
//! the type system ensures that it went through the allowlist. Formatting uses
//! CSS classes instead of `style` attributes, so that the Content Security
//! Policy can forbid inline styles.

use ammonia::{Builder, UrlRelative};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::sync::LazyLock;

/// The allowed elements: what the visual editor produces, plus `b`, `i`,
/// headings, and ordered lists for HTML written by hand in the fallback text
/// field. All other elements are removed, while their text stays.
///
/// A new editor feature has to fit through this allowlist, see
/// docs/developer.md.
const ALLOWED_TAGS: [&str; 14] = [
    "a", "b", "br", "em", "h2", "h3", "hr", "i", "li", "ol", "p", "span", "strong", "ul",
];

/// Elements that are removed including their content, as it isn't text for
/// the reader.
const REMOVED_WITH_CONTENT: [&str; 2] = ["script", "style"];

/// The attributes allowed on every element. Only `title`, as the old pages
/// have link titles; everything else could carry scripts or styles.
const GENERIC_ATTRIBUTES: [&str; 1] = ["title"];

/// The URL schemes of links.
const ALLOWED_URL_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tel"];

/// The CSS class for centered text.
const CLASS_CENTER: &str = "text-center";

/// The CSS class for red text.
const CLASS_RED: &str = "text-red";

/// The configured sanitizer.
static SANITIZER: LazyLock<Builder<'static>> = LazyLock::new(|| {
    let mut builder = Builder::empty();
    builder
        .tags(HashSet::from(ALLOWED_TAGS))
        .generic_attributes(HashSet::from(GENERIC_ATTRIBUTES))
        .clean_content_tags(HashSet::from(REMOVED_WITH_CONTENT))
        .tag_attributes(HashMap::from([("a", HashSet::from(["href"]))]))
        .allowed_classes(HashMap::from([
            ("p", HashSet::from([CLASS_CENTER])),
            ("h2", HashSet::from([CLASS_CENTER])),
            ("h3", HashSet::from([CLASS_CENTER])),
            ("span", HashSet::from([CLASS_RED])),
        ]))
        .url_schemes(HashSet::from(ALLOWED_URL_SCHEMES))
        .url_relative(UrlRelative::PassThrough)
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true);
    builder
});

/// HTML that went through the allowlist.
///
/// Deserialization sanitizes as well, so manual edits of the state file can't
/// bypass the allowlist.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(from = "String", into = "String")]
pub(crate) struct SanitizedHtml(String);

impl SanitizedHtml {
    /// Removes everything from `html` that isn't on the allowlist.
    pub(crate) fn sanitize(html: &str) -> Self {
        Self(SANITIZER.clean(html).to_string().trim().to_string())
    }

    /// Returns the HTML, safe for output without escaping.
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the length of the HTML in bytes.
    pub(crate) const fn len(&self) -> usize {
        self.0.len()
    }
}

impl From<String> for SanitizedHtml {
    /// Sanitizes `html`.
    fn from(html: String) -> Self {
        Self::sanitize(&html)
    }
}

impl From<SanitizedHtml> for String {
    /// Returns the sanitized HTML.
    fn from(html: SanitizedHtml) -> Self {
        html.0
    }
}

impl Display for SanitizedHtml {
    /// Writes the HTML verbatim.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the sanitized form of `html` as text.
    fn clean(html: &str) -> String {
        SanitizedHtml::sanitize(html).as_str().to_string()
    }

    /// All formatting that the editor offers survives without changes.
    #[test]
    fn keeps_editor_formatting() {
        let html = "<p class=\"text-center\"><strong>a</strong> <em>b</em> \
                    <span class=\"text-red\">c</span></p><hr><ul><li>d</li></ul>";

        assert_eq!(clean(html), html);
    }

    /// Script elements disappear with their content.
    #[test]
    fn removes_script_with_content() {
        assert_eq!(clean("<p>a</p><script>alert(1)</script>"), "<p>a</p>");
        assert_eq!(
            clean("<style>* { color: red; }</style><p>a</p>"),
            "<p>a</p>"
        );
    }

    /// Well-known XSS payloads don't leave anything executable behind.
    #[test]
    fn neutralizes_xss_payloads() {
        for payload in [
            "<img src=x onerror=alert(1)>",
            "<svg onload=alert(1)>",
            "<a href=\"javascript:alert(1)\">x</a>",
            "<a href=\" jAvAsCrIpT:alert(1)\">x</a>",
            "<a href=\"data:text/html,<script>alert(1)</script>\">x</a>",
            "<p onclick=\"alert(1)\">x</p>",
            "<iframe src=\"https://evil.test\"></iframe>",
            "<form action=\"https://evil.test\"><input name=x></form>",
            "<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>",
        ] {
            let cleaned = clean(payload);

            assert!(!cleaned.contains("onerror"));
            assert!(!cleaned.contains("onload"));
            assert!(!cleaned.contains("onclick"));
            assert!(!cleaned.to_lowercase().contains("javascript:"));
            assert!(!cleaned.contains("data:"));
            assert!(!cleaned.contains("<iframe"));
            assert!(!cleaned.contains("<form"));
            assert!(!cleaned.contains("<img"));
        }
    }

    /// `style` attributes would violate the Content Security Policy, and only
    /// the known classes are kept.
    #[test]
    fn removes_styles_and_unknown_classes() {
        assert_eq!(clean("<p style=\"color:red\">a</p>"), "<p>a</p>");
        assert!(!clean("<p class=\"evil\">a</p>").contains("evil"));
        assert!(!clean("<span class=\"text-center\">a</span>").contains("text-center"));
    }

    /// Attributes other than `title` are removed from every element.
    #[test]
    fn removes_generic_attributes_except_title() {
        assert_eq!(
            clean("<p lang=\"en\" id=\"x\" dir=\"rtl\" title=\"t\">a</p>"),
            "<p title=\"t\">a</p>"
        );
    }

    /// Unknown elements are removed, but the patient must still see the text.
    #[test]
    fn keeps_text_of_unknown_elements() {
        assert_eq!(
            clean("<div><font color=\"red\">wichtig</font></div>"),
            "wichtig"
        );
    }

    /// Links keep their target and title, lose `target`, and get protective
    /// `rel` values.
    #[test]
    fn sanitizes_links() {
        assert_eq!(
            clean("<a href=\"https://kvs.test/\" target=\"_blank\" title=\"x\">k</a>"),
            "<a href=\"https://kvs.test/\" title=\"x\" rel=\"noopener noreferrer\">k</a>"
        );
        assert!(clean("<a href=\"/kontakt/\">k</a>").contains("href=\"/kontakt/\""));
        assert!(clean("<a href=\"tel:+49372139040\">k</a>").contains("tel:"));
        assert!(clean("<a href=\"mailto:a@b.test\">k</a>").contains("mailto:"));
    }

    /// Text that looks like markup stays escaped text.
    #[test]
    fn escapes_angle_brackets_in_text() {
        assert_eq!(clean("1 &lt; 2 &amp; 3"), "1 &lt; 2 &amp; 3");
    }

    /// Sanitizing is idempotent, so loading stored content doesn't change it.
    #[test]
    fn sanitizing_twice_changes_nothing() {
        let once = clean("<p class=\"text-center\">a &amp; <strong>b</strong></p><hr>");

        assert_eq!(clean(&once), once);
    }

    /// The bundled start page content must survive the allowlist without any
    /// loss, or its formatting would disappear on first start.
    #[test]
    fn keeps_bundled_content_unchanged() {
        let seed = include_str!("../../content/aktuelle-informationen.html").trim();

        assert_eq!(clean(seed), seed);
    }

    /// A manipulated state file can't inject markup.
    #[test]
    fn deserializing_sanitizes() {
        let html: SanitizedHtml = serde_json::from_str("\"<script>alert(1)</script><p>a</p>\"")
            .expect("JSON string should parse");

        assert_eq!(html.as_str(), "<p>a</p>");
    }

    /// Serializing produces a plain JSON string.
    #[test]
    fn serializes_as_string() {
        let html = SanitizedHtml::sanitize("<p>a</p>");

        assert_eq!(
            serde_json::to_string(&html).expect("should serialize"),
            "\"<p>a</p>\""
        );
    }
}
