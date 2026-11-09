// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client IP address, as reported by the reverse proxy.
//!
//! It's only used to throttle failed logins per client, and only as a
//! pseudonym, see [`ClientPseudonyms`]. Addresses are never logged or stored.
//! The header is only trusted if configured, because clients can set any
//! header themselves. The proxy must overwrite it, as nginx does with
//! `proxy_set_header X-Real-IP $remote_addr`.

use crate::hex;
use crate::security::token::RandomError;
use axum::http::{HeaderMap, HeaderName};
use sha2::{Digest, Sha256};
use std::fmt::{self, Debug, Formatter};
use std::net::{IpAddr, Ipv6Addr};

/// The number of leading bits of an IPv6 address that identify a client.
/// Providers assign whole /64 networks, so one client can use any address in
/// it.
const IPV6_CLIENT_PREFIX_BITS: u32 = 64;

/// The length of the random pseudonym key in bytes.
const KEY_BYTES: usize = 32;

/// The length of a pseudonym in bytes. 128 bits make collisions between
/// clients practically impossible.
const PSEUDONYM_BYTES: usize = 16;

/// Turns client addresses into pseudonyms for the login throttle.
///
/// A pseudonym is the SHA-256 digest of a random key and the client network.
/// The key only exists in memory and changes on every start. So the throttle
/// table contains no addresses, and they can't be recovered by hashing all
/// IPv4 addresses without the key. HMAC isn't needed, as pseudonyms never
/// leave the process.
pub(super) struct ClientPseudonyms {
    /// The random key, prepended to each network before hashing.
    key: [u8; KEY_BYTES],
}

impl ClientPseudonyms {
    /// Creates pseudonyms with a new key from the operating system's random
    /// number generator.
    ///
    /// # Errors
    ///
    /// Returns [`RandomError`] if the random number generator fails.
    pub(super) fn generate() -> Result<Self, RandomError> {
        let mut key = [0; KEY_BYTES];
        getrandom::fill(&mut key).map_err(|_| RandomError)?;
        Ok(Self { key })
    }

    /// Returns the pseudonym of the client with `address`. All addresses of
    /// an IPv6 /64 network share one pseudonym, see [`network`].
    pub(super) fn of(&self, address: IpAddr) -> String {
        let digest = Sha256::new()
            .chain_update(self.key)
            .chain_update(network(address))
            .finalize();
        hex::encode(&digest[..PSEUDONYM_BYTES])
    }
}

impl Debug for ClientPseudonyms {
    /// Prints a placeholder instead of the key.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("ClientPseudonyms(<redacted>)")
    }
}

/// Returns the client address from the configured proxy `header`.
///
/// If the header has a list, such as `X-Forwarded-For`, the last entry counts,
/// as the proxy appends the address of the client it saw. Returns `None` if no
/// header is configured or its value isn't an IP address.
pub(crate) fn from_headers(headers: &HeaderMap, header: Option<&HeaderName>) -> Option<IpAddr> {
    let value = headers.get(header?)?.to_str().ok()?;
    value.rsplit(',').next()?.trim().parse().ok()
}

/// Returns the network that identifies a client: the address itself for
/// IPv4, the /64 network for IPv6.
fn network(address: IpAddr) -> String {
    match address.to_canonical() {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let mask = u128::MAX << (128 - IPV6_CLIENT_PREFIX_BITS);
            let network = Ipv6Addr::from_bits(v6.to_bits() & mask);
            format!("{network}/{IPV6_CLIENT_PREFIX_BITS}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// Returns request headers with `X-Real-IP: value`.
    fn real_ip(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static(value));
        headers
    }

    /// The configured header name.
    fn configured() -> HeaderName {
        HeaderName::from_static("x-real-ip")
    }

    /// The address is read from the configured header only.
    #[test]
    fn reads_configured_header_only() {
        let headers = real_ip("192.0.2.7");

        assert_eq!(
            from_headers(&headers, Some(&configured())),
            Some("192.0.2.7".parse().expect("valid"))
        );
        assert_eq!(from_headers(&headers, None), None);
    }

    /// In lists, the entry that the proxy appended counts, not the ones a
    /// client may have sent.
    #[test]
    fn takes_last_list_entry() {
        let headers = real_ip("10.0.0.1, 198.51.100.4");

        assert_eq!(
            from_headers(&headers, Some(&configured())),
            Some("198.51.100.4".parse().expect("valid"))
        );
    }

    /// Garbage doesn't produce an address.
    #[test]
    fn rejects_invalid_values() {
        assert_eq!(from_headers(&real_ip("unknown"), Some(&configured())), None);
        assert_eq!(from_headers(&HeaderMap::new(), Some(&configured())), None);
    }

    /// Returns `address` parsed, for tests with literal addresses.
    fn ip(address: &str) -> IpAddr {
        address.parse().expect("test addresses should be valid")
    }

    /// IPv6 clients are grouped by their /64 network, IPv4 clients aren't.
    #[test]
    fn network_groups_ipv6_networks() {
        assert_eq!(network(ip("2001:db8:1:2::1")), "2001:db8:1:2::/64");
        assert_eq!(network(ip("2001:db8:1:2:ffff::9")), "2001:db8:1:2::/64");
        assert_eq!(network(ip("2001:db8:1:3::1")), "2001:db8:1:3::/64");
        assert_eq!(network(ip("192.0.2.7")), "192.0.2.7");
    }

    /// IPv4 clients behind an IPv6 socket are treated as IPv4 clients.
    #[test]
    fn network_unwraps_mapped_ipv4() {
        assert_eq!(network(ip("::ffff:192.0.2.7")), "192.0.2.7");
    }

    /// One client keeps its pseudonym, so its failures add up, and other
    /// clients get other pseudonyms.
    #[test]
    fn pseudonym_is_stable_per_client() {
        let pseudonyms = ClientPseudonyms::generate().expect("OS RNG should work in tests");

        assert_eq!(
            pseudonyms.of(ip("192.0.2.7")),
            pseudonyms.of(ip("::ffff:192.0.2.7"))
        );
        assert_eq!(
            pseudonyms.of(ip("2001:db8::1")),
            pseudonyms.of(ip("2001:db8::2"))
        );
        assert_ne!(
            pseudonyms.of(ip("192.0.2.7")),
            pseudonyms.of(ip("192.0.2.8"))
        );
    }

    /// The pseudonym depends on the secret key, so it can't be computed from
    /// the address alone.
    #[test]
    fn pseudonym_depends_on_key() {
        let first = ClientPseudonyms {
            key: [1; KEY_BYTES],
        };
        let second = ClientPseudonyms {
            key: [2; KEY_BYTES],
        };

        let pseudonym = first.of(ip("192.0.2.7"));

        assert_ne!(pseudonym, second.of(ip("192.0.2.7")));
        assert_eq!(pseudonym.len(), PSEUDONYM_BYTES * 2);
    }

    /// The key must not end up in logs through `{:?}`.
    #[test]
    fn debug_output_hides_key() {
        let pseudonyms = ClientPseudonyms {
            key: [0xab; KEY_BYTES],
        };

        assert!(!format!("{pseudonyms:?}").contains("abab"));
    }
}
