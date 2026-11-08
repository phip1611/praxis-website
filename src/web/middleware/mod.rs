// SPDX-License-Identifier: MIT OR Apache-2.0

//! Middleware that applies to all requests.

pub(super) mod noindex;
pub(super) mod request_log;
pub(super) mod security_headers;
pub(super) mod timeout;
