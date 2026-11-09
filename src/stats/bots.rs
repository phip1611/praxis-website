// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tells crawlers and other automated clients apart from people.
//!
//! A simple heuristic on the user agent, which is good enough for statistics.
//! The user agent is neither stored nor logged.

/// Parts of user agents of automated clients, in lowercase. Covers search
/// engines, link previews, monitoring, feed readers, and HTTP libraries.
const BOT_MARKERS: [&str; 15] = [
    "bot",
    "crawl",
    "spider",
    "slurp",
    "facebookexternalhit",
    "preview",
    "monitor",
    "lighthouse",
    "headless",
    "feed",
    "curl",
    "wget",
    "python",
    "go-http-client",
    "java/",
];

/// Returns whether a request with this user agent comes from a machine.
///
/// Requests without user agent count as automated, as browsers always send
/// one.
pub(super) fn is_bot(user_agent: Option<&str>) -> bool {
    let Some(user_agent) = user_agent.filter(|ua| !ua.trim().is_empty()) else {
        return true;
    };
    let user_agent = user_agent.to_ascii_lowercase();
    BOT_MARKERS.iter().any(|marker| user_agent.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Common browsers count as people.
    #[test]
    fn browsers_are_people() {
        for user_agent in [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
            "Mozilla/5.0 (X11; Linux x86_64; rv:145.0) Gecko/20100101 Firefox/145.0",
        ] {
            assert!(!is_bot(Some(user_agent)), "{user_agent}");
        }
    }

    /// Search engines, previews, and tools count as machines.
    #[test]
    fn crawlers_and_tools_are_bots() {
        for user_agent in [
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
            "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
            "curl/8.11.0",
            "python-requests/2.32.3",
            "Feedly/1.0 (+http://www.feedly.com/fetcher.html)",
        ] {
            assert!(is_bot(Some(user_agent)), "{user_agent}");
        }
    }

    /// Browsers always send a user agent, so its absence means a machine.
    #[test]
    fn missing_user_agent_is_bot() {
        assert!(is_bot(None));
        assert!(is_bot(Some("  ")));
    }
}
