// SPDX-License-Identifier: MIT OR Apache-2.0

//! All security-relevant building blocks of the service.
//!
//! The rest of the crate only composes these parts and must not implement its
//! own checks. The form extractors in `web::admin` apply the request forgery
//! checks to every state-changing request.
//!
//! # Threat model
//!
//! - Visitors and bots are untrusted. Public pages are precomputed, so they
//!   have no request-dependent code paths beyond content negotiation.
//! - Accounts in the login area are trusted to edit content, but what they
//!   submit is still sanitized, as a stolen session must not lead to stored
//!   XSS against patients.
//! - The reverse proxy in front of the service is trusted to terminate TLS.
//!   Its client IP header is only used if configured, and only to throttle
//!   failed logins per client, see [`client_ip`]. IP addresses are never
//!   logged or stored, the throttle only keeps pseudonyms in memory.
//!
//! # Overview
//!
//! - [`token`]: Random secrets and constant-time comparison.
//! - [`password`]: Argon2id hashing and verification.
//! - [`credentials`]: The accounts file and account lookup.
//! - [`password_overrides`]: Passwords changed in the login area.
//! - [`client_ip`]: The client address behind the reverse proxy.
//! - [`throttle`]: Limits failed logins per client and per account.
//! - [`rate_limit`]: Limits password checks of all clients together.
//! - [`session`]: Server-side sessions with idle and absolute timeouts.
//! - [`authenticator`]: The login flow, combining the parts above.
//! - [`csrf`]: Per-session tokens for state-changing requests.
//! - [`origin`]: Rejects cross-origin state-changing requests.
//! - [`cookie`]: The attributes and parsing of the session cookie.
//! - [`headers`]: Security headers such as the Content Security Policy.
//! - [`sanitize`]: The allowlist for user-provided HTML.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod authenticator;
pub(crate) mod client_ip;
pub(crate) mod cookie;
pub(crate) mod credentials;
mod csrf;
pub(crate) mod headers;
pub(crate) mod origin;
pub(crate) mod password;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod password_overrides;
mod rate_limit;
pub(crate) mod sanitize;
pub(crate) mod session;
pub(crate) mod throttle;
pub(crate) mod token;
