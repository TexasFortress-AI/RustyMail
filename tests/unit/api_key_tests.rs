// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The shared API key helpers used by `/mcp` and `/api/dashboard`.

use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};
use rustymail::api::api_key::{keys_match, presented_api_key};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    map
}

#[test]
fn keys_match_only_when_identical() {
    assert!(keys_match("abc123", "abc123"));
    assert!(!keys_match("abc124", "abc123"));
    assert!(!keys_match("abc12", "abc123"));
    assert!(!keys_match("", "abc123"));
}

#[test]
fn key_is_read_from_x_api_key_in_any_case() {
    assert_eq!(
        presented_api_key(&headers(&[("x-api-key", "k1")])),
        Some("k1")
    );
    assert_eq!(
        presented_api_key(&headers(&[("X-API-Key", "k2")])),
        Some("k2")
    );
}

#[test]
fn key_is_read_from_a_bearer_token() {
    let map = headers(&[(AUTHORIZATION.as_str(), "Bearer k3")]);
    assert_eq!(presented_api_key(&map), Some("k3"));
    let basic = headers(&[(AUTHORIZATION.as_str(), "Basic k3")]);
    assert_eq!(presented_api_key(&basic), None);
    assert_eq!(presented_api_key(&HeaderMap::new()), None);
}
