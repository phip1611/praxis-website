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
//! - Accounts in the login area are trusted to see the access statistics.
//! - The reverse proxy in front of the service is trusted to terminate TLS.
//!
//! # Overview
//!
//! - [`token`]: Random secrets and constant-time comparison.
//! - [`password`]: Argon2id hashing and verification.
//! - [`credentials`]: The accounts file and account lookup.
//! - [`client_ip`]: The client address behind the reverse proxy.
//! - [`throttle`]: Limits failed logins per client and per account.
//! - [`session`]: Server-side sessions with idle and absolute timeouts.
//! - [`authenticator`]: The login flow, combining the parts above.
//! - [`csrf`]: Per-session tokens for state-changing requests.
//! - [`origin`]: Rejects cross-origin state-changing requests.
//! - [`cookie`]: The attributes and parsing of the session cookie.
//! - [`headers`]: Security headers such as the Content Security Policy.
//! - [`sanitize`]: The allowlist for user-provided HTML.

pub(crate) mod authenticator;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod client_ip;
pub(crate) mod cookie;
pub(crate) mod credentials;
mod csrf;
pub(crate) mod headers;
pub(crate) mod origin;
pub(crate) mod password;
pub(crate) mod sanitize;
pub(crate) mod session;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod throttle;
pub(crate) mod token;
