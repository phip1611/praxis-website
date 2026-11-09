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
//! - [`headers`]: Security headers such as the Content Security Policy.
//! - [`sanitize`]: The allowlist for the HTML of the news.

pub(crate) mod headers;
pub(crate) mod sanitize;
