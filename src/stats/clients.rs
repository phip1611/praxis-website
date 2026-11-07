// SPDX-License-Identifier: MIT OR Apache-2.0

//! The device class, browser, and operating system of a visitor.
//!
//! A heuristic on the user agent, which is neither stored nor logged. It only
//! tells families apart, as browsers no longer report details such as the
//! version of the operating system. The order of the checks matters, as user
//! agents name other browsers for compatibility: Edge, for example, also
//! contains `Chrome/` and `Safari/`. The tokens follow
//! <https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Browser_detection_using_the_user_agent>.
//!
//! iPads send the same user agent as Safari on macOS by default, so they
//! count as macOS computers.

use serde::{Deserialize, Serialize};

/// A property of visitors that the statistics count, such as the browser.
pub(crate) trait Category: Copy + Ord + 'static {
    /// All values, in the order the statistics page lists equal counts.
    const ALL: &'static [Self];

    /// Returns the German name for the statistics page.
    fn label(self) -> &'static str;
}

/// The kind of device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Device {
    /// A phone or another small mobile device.
    Smartphone,
    /// A tablet.
    Tablet,
    /// A desktop or laptop computer.
    Computer,
}

impl Category for Device {
    const ALL: &'static [Self] = &[Self::Smartphone, Self::Tablet, Self::Computer];

    fn label(self) -> &'static str {
        match self {
            Self::Smartphone => "Smartphone",
            Self::Tablet => "Tablet",
            Self::Computer => "Computer",
        }
    }
}

/// The browser family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Browser {
    /// Google Chrome and other Chromium-based browsers without own token.
    Chrome,
    /// Apple Safari, including other browsers on iOS without own token.
    Safari,
    /// Mozilla Firefox.
    Firefox,
    /// Microsoft Edge.
    Edge,
    /// Samsung Internet.
    SamsungInternet,
    /// Opera.
    Opera,
    /// Anything else.
    Other,
}

impl Category for Browser {
    const ALL: &'static [Self] = &[
        Self::Chrome,
        Self::Safari,
        Self::Firefox,
        Self::Edge,
        Self::SamsungInternet,
        Self::Opera,
        Self::Other,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Safari => "Safari",
            Self::Firefox => "Firefox",
            Self::Edge => "Edge",
            Self::SamsungInternet => "Samsung Internet",
            Self::Opera => "Opera",
            Self::Other => "Andere",
        }
    }
}

/// The operating system family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OperatingSystem {
    /// Microsoft Windows.
    Windows,
    /// Apple macOS, including most iPads, see the module docs.
    MacOs,
    /// Apple iOS and iPadOS.
    Ios,
    /// Google Android.
    Android,
    /// Linux on desktops.
    Linux,
    /// Anything else, including ChromeOS.
    Other,
}

impl Category for OperatingSystem {
    const ALL: &'static [Self] = &[
        Self::Windows,
        Self::MacOs,
        Self::Ios,
        Self::Android,
        Self::Linux,
        Self::Other,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOs => "macOS",
            Self::Ios => "iOS/iPadOS",
            Self::Android => "Android",
            Self::Linux => "Linux",
            Self::Other => "Andere",
        }
    }
}

/// What the statistics count of a visitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Client {
    /// The kind of device.
    pub(super) device: Device,
    /// The browser family.
    pub(super) browser: Browser,
    /// The operating system family.
    pub(super) system: OperatingSystem,
}

impl Client {
    /// Classifies the visitor with this user agent.
    pub(super) fn from_user_agent(user_agent: &str) -> Self {
        Self {
            device: device(user_agent),
            browser: browser(user_agent),
            system: operating_system(user_agent),
        }
    }
}

/// Returns whether `user_agent` contains any of `tokens`.
fn contains_any(user_agent: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|token| user_agent.contains(token))
}

/// Returns the kind of device.
///
/// Phones have `Mobi` in the user agent. iPads have it as well, and Android
/// tablets are the Android devices without it.
fn device(user_agent: &str) -> Device {
    let mobile = user_agent.contains("Mobi");
    if user_agent.contains("iPad") || (user_agent.contains("Android") && !mobile) {
        Device::Tablet
    } else if mobile {
        Device::Smartphone
    } else {
        Device::Computer
    }
}

/// Returns the browser family. Each check excludes the browsers whose tokens
/// the later checks look for, e.g., Edge contains `Chrome/`.
fn browser(user_agent: &str) -> Browser {
    if contains_any(user_agent, &["Edg/", "EdgA/", "EdgiOS/"]) {
        Browser::Edge
    } else if contains_any(user_agent, &["OPR/", "OPT/", "OPiOS/", "Opera"]) {
        Browser::Opera
    } else if user_agent.contains("SamsungBrowser/") {
        Browser::SamsungInternet
    } else if contains_any(user_agent, &["Firefox/", "FxiOS/"]) {
        Browser::Firefox
    } else if contains_any(user_agent, &["Chrome/", "CriOS/", "Chromium/"]) {
        Browser::Chrome
    } else if user_agent.contains("Safari/") {
        Browser::Safari
    } else {
        Browser::Other
    }
}

/// Returns the operating system family. iOS and Android user agents also
/// contain `Mac OS X` and `Linux`, and ChromeOS contains `X11`.
fn operating_system(user_agent: &str) -> OperatingSystem {
    if user_agent.contains("Windows") {
        OperatingSystem::Windows
    } else if contains_any(user_agent, &["iPhone", "iPad", "iPod"]) {
        OperatingSystem::Ios
    } else if user_agent.contains("Android") {
        OperatingSystem::Android
    } else if user_agent.contains("CrOS") {
        OperatingSystem::Other
    } else if user_agent.contains("Macintosh") {
        OperatingSystem::MacOs
    } else if contains_any(user_agent, &["Linux", "X11"]) {
        OperatingSystem::Linux
    } else {
        OperatingSystem::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Current user agents of common browsers with the expected result.
    const CASES: &[(&str, Device, Browser, OperatingSystem)] = &[
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
            Device::Computer,
            Browser::Chrome,
            OperatingSystem::Windows,
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0",
            Device::Computer,
            Browser::Edge,
            OperatingSystem::Windows,
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 OPR/124.0.0.0",
            Device::Computer,
            Browser::Opera,
            OperatingSystem::Windows,
        ),
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
            Device::Computer,
            Browser::Safari,
            OperatingSystem::MacOs,
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:145.0) Gecko/20100101 Firefox/145.0",
            Device::Computer,
            Browser::Firefox,
            OperatingSystem::Linux,
        ),
        (
            "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
            Device::Computer,
            Browser::Chrome,
            OperatingSystem::Other,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
            Device::Smartphone,
            Browser::Safari,
            OperatingSystem::Ios,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/142.0.7444.46 Mobile/15E148 Safari/604.1",
            Device::Smartphone,
            Browser::Chrome,
            OperatingSystem::Ios,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/145.0 Mobile/15E148 Safari/605.1.15",
            Device::Smartphone,
            Browser::Firefox,
            OperatingSystem::Ios,
        ),
        (
            "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
            Device::Tablet,
            Browser::Safari,
            OperatingSystem::Ios,
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36",
            Device::Smartphone,
            Browser::Chrome,
            OperatingSystem::Android,
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
            Device::Tablet,
            Browser::Chrome,
            OperatingSystem::Android,
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/29.0 Chrome/136.0.0.0 Mobile Safari/537.36",
            Device::Smartphone,
            Browser::SamsungInternet,
            OperatingSystem::Android,
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36 EdgA/142.0.0.0",
            Device::Smartphone,
            Browser::Edge,
            OperatingSystem::Android,
        ),
        (
            "Mozilla/5.0 (Android 15; Mobile; rv:145.0) Gecko/145.0 Firefox/145.0",
            Device::Smartphone,
            Browser::Firefox,
            OperatingSystem::Android,
        ),
        (
            "Mozilla/5.0 (Android 15; Tablet; rv:145.0) Gecko/145.0 Firefox/145.0",
            Device::Tablet,
            Browser::Firefox,
            OperatingSystem::Android,
        ),
    ];

    /// Common browsers are recognized despite the tokens of other browsers
    /// in their user agents.
    #[test]
    fn classifies_common_browsers() {
        for &(user_agent, device, browser, system) in CASES {
            let client = Client::from_user_agent(user_agent);

            assert_eq!(client.device, device, "{user_agent}");
            assert_eq!(client.browser, browser, "{user_agent}");
            assert_eq!(client.system, system, "{user_agent}");
        }
    }

    /// Unknown clients count as computers with other browser and system.
    #[test]
    fn unknown_user_agent_is_other() {
        let client = Client::from_user_agent("Lynx/2.9.2 libwww-FM/2.14");

        assert_eq!(client.device, Device::Computer);
        assert_eq!(client.browser, Browser::Other);
        assert_eq!(client.system, OperatingSystem::Other);
    }
}
