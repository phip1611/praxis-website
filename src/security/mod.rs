// SPDX-License-Identifier: MIT OR Apache-2.0

//! All security-relevant building blocks of the service.
//!
//! The rest of the crate only composes these parts and must not implement its
//! own checks.
//!
//! # Threat model
//!
//! - Visitors and bots are untrusted. Public pages are precomputed, so they
//!   have no request-dependent code paths beyond content negotiation.
//! - The reverse proxy in front of the service is trusted to terminate TLS.
//!
//! # Overview
//!
//! - [`token`]: Random secrets and constant-time comparison.
//! - [`password`]: Argon2id hashing and verification.
//! - [`credentials`]: The accounts file and account lookup.
//! - [`session`]: Server-side sessions with idle and absolute timeouts.
//! - [`csrf`]: Per-session tokens for state-changing requests.
//! - [`origin`]: Rejects cross-origin state-changing requests.
//! - [`headers`]: Security headers such as the Content Security Policy.
//! - [`sanitize`]: The allowlist for the HTML of the news.

#[expect(dead_code, reason = "used by code that a later commit adds")]
pub(crate) mod credentials;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
mod csrf;
pub(crate) mod headers;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod origin;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod password;
pub(crate) mod sanitize;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod session;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by code that a later commit adds")
)]
pub(crate) mod token;
