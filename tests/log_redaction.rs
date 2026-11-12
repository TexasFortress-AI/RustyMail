// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! API key values must never reach the log.
//!
//! Its own test binary, because it installs a process-wide logger that
//! captures every record.

use log::{LevelFilter, Log, Metadata, Record};
use rustymail::api::auth::{ApiKeyStore, ApiScope, ImapCredentials};
use std::sync::Mutex;

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct CapturingLogger;

impl Log for CapturingLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        CAPTURED.lock().unwrap().push(record.args().to_string());
    }
    fn flush(&self) {}
}

static LOGGER: CapturingLogger = CapturingLogger;

fn assert_not_logged(secret: &str) {
    let lines = CAPTURED.lock().unwrap();
    assert!(
        !lines.is_empty(),
        "nothing was logged; the logger is not capturing"
    );
    for line in lines.iter() {
        assert!(
            !line.contains(secret),
            "secret written to the log: {:?}",
            line
        );
    }
}

#[tokio::test]
async fn api_key_store_never_logs_key_values() {
    log::set_logger(&LOGGER).expect("only this test installs a logger");
    log::set_max_level(LevelFilter::Trace);

    let store = ApiKeyStore::new();
    let created = store
        .create_api_key(
            "log-test".to_string(),
            "log-test@example.test".to_string(),
            ImapCredentials {
                username: "u".to_string(),
                password: "p".to_string(),
                server: "imap.example.test".to_string(),
                port: 993,
            },
            vec![ApiScope::ReadEmail],
        )
        .await;
    store.validate_key(&created).await.unwrap();
    store.revoke_key(&created).await.unwrap();
    assert!(store.validate_key(&created).await.is_err()); // inactive

    let unknown = "unknown-key-that-must-not-be-logged-0123456789";
    assert!(store.validate_key(unknown).await.is_err());

    assert_not_logged(&created);
    assert_not_logged(unknown);
}
