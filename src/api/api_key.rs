// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The single check of a request's key against `RUSTYMAIL_API_KEY`.
//! Shared by the MCP HTTP transport (`/mcp`) and the dashboard API
//! (`/api/dashboard`).

use actix_web::http::header::{HeaderMap, AUTHORIZATION};
use subtle::ConstantTimeEq;

/// Environment variable holding the server's API key.
pub const API_KEY_ENV: &str = "RUSTYMAIL_API_KEY";
/// Header carrying the key (header names are case-insensitive).
pub const API_KEY_HEADER: &str = "X-Api-Key";
/// The `.env.example` placeholder, never accepted as a real key.
const PLACEHOLDER_KEY: &str = "your-secure-api-key-here";
const BEARER_PREFIX: &str = "Bearer ";

/// Outcome of checking a request's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyCheck {
    Valid,
    Missing,
    Invalid,
    /// The server has no usable key configured, so every request is refused.
    NotConfigured,
}

/// The configured key, or `None` when it is unset, empty or the placeholder.
pub fn configured_api_key() -> Option<String> {
    std::env::var(API_KEY_ENV)
        .ok()
        .filter(|key| !key.is_empty() && key != PLACEHOLDER_KEY)
}

/// The key a request presents, from `X-Api-Key` or `Authorization: Bearer`.
pub fn presented_api_key(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix(BEARER_PREFIX))
        })
}

/// Compares keys in constant time, so response timing reveals nothing
/// about how many leading characters of a guess were right.
pub fn keys_match(presented: &str, configured: &str) -> bool {
    presented.as_bytes().ct_eq(configured.as_bytes()).into()
}

/// Checks the request's key against the configured key.
pub fn check_api_key(headers: &HeaderMap) -> ApiKeyCheck {
    let Some(configured) = configured_api_key() else {
        return ApiKeyCheck::NotConfigured;
    };
    match presented_api_key(headers) {
        None => ApiKeyCheck::Missing,
        Some(presented) if keys_match(presented, &configured) => ApiKeyCheck::Valid,
        Some(_) => ApiKeyCheck::Invalid,
    }
}
