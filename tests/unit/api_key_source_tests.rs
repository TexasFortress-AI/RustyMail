// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The API key comes from the environment, else the Keychain, else nowhere.

use rustymail::api::api_key_source::{resolve_api_key, ApiKeySource};

#[test]
fn environment_wins_and_the_keychain_is_not_read() {
    let (value, source) = resolve_api_key(Some("from-env".into()), || {
        panic!("Keychain must not be read when the environment has the key")
    });
    assert_eq!(value.as_deref(), Some("from-env"));
    assert_eq!(source, ApiKeySource::Environment);
}

#[test]
fn keychain_is_used_when_the_environment_is_unset_or_blank() {
    for environment in [None, Some(String::new()), Some("  ".to_string())] {
        let (value, source) = resolve_api_key(environment, || Some("from-keychain".into()));
        assert_eq!(value.as_deref(), Some("from-keychain"));
        assert_eq!(source, ApiKeySource::Keychain);
    }
}

#[test]
fn missing_everywhere_is_reported_as_missing() {
    assert_eq!(
        resolve_api_key(None, || None),
        (None, ApiKeySource::Missing)
    );
    assert_eq!(
        resolve_api_key(None, || Some(String::new())),
        (None, ApiKeySource::Missing)
    );
}
