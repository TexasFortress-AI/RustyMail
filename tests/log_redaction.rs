// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! API key values, and the sender, subject and model reasoning of triaged
//! mail, must never reach the log.
//!
//! Its own test binary, because it installs a process-wide logger that
//! captures every record.

#[path = "integration/fake_imap_server.rs"]
#[allow(dead_code)]
mod fake_imap_server;

use log::{LevelFilter, Log, Metadata, Record};
use rustymail::api::auth::{ApiKeyStore, ApiScope, ImapCredentials};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use rustymail::triage::model::{EscalatingClassifier, ModelBackend};
use rustymail::triage::rules::SenderRules;
use rustymail::triage::runner::{RunOptions, TriageRunner};
use rustymail::triage::store::TriageStore;
use std::sync::{Mutex, Once};

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct CapturingLogger;

/// Everything from RustyMail itself; other crates down to DEBUG, the level
/// the backend runs at. (async-imap dumps raw protocol bytes at TRACE, which
/// never reaches the real log.)
const THIRD_PARTY_LEVEL: log::Level = log::Level::Debug;

impl Log for CapturingLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().starts_with("rustymail") || metadata.level() <= THIRD_PARTY_LEVEL
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            CAPTURED.lock().unwrap().push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}

static LOGGER: CapturingLogger = CapturingLogger;
static INSTALL: Once = Once::new();

fn capture_logs() {
    INSTALL.call_once(|| {
        log::set_logger(&LOGGER).expect("only this binary installs a logger");
        log::set_max_level(LevelFilter::Trace);
    });
}

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
    capture_logs();

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

const TRIAGE_SENDER: &str = "private.sender@example.test";
const TRIAGE_SUBJECT: &str = "Confidential dinner plans";
const MODEL_REASON: &str = "mentions the surprise party";

struct ReasoningModel;

impl ModelBackend for ReasoningModel {
    async fn complete(&self, _model: &str, _system: &str, _user: &str) -> Result<String, String> {
        Ok(format!(
            r#"{{"bucket": "Needs Reply", "confidence": 0.9, "reason": "{}"}}"#,
            MODEL_REASON
        ))
    }
}

#[tokio::test]
async fn triage_never_logs_senders_subjects_or_model_reasons() {
    capture_logs();
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let (port, mailbox) = fake_imap_server::spawn_fake_imap(vec![(1, "<p@x.test>")]).await;
    mailbox.lock().unwrap().raw_bodies.insert(
        "<p@x.test>".to_string(),
        format!(
            "From: {}\r\nSubject: {}\r\nMessage-ID: <p@x.test>\r\n\r\nSee you then.\r\n",
            TRIAGE_SENDER, TRIAGE_SUBJECT
        ),
    );
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let runner = TriageRunner {
        store: TriageStore::new(pool),
        classifier: EscalatingClassifier::new(ReasoningModel, "m".into(), None, 0.7),
        senders: SenderRules::default(),
        code_window: chrono::Duration::hours(24),
        folder: "INBOX".into(),
        fetch_batch: 10,
        excerpt_chars: 500,
    };
    let summary = runner
        .triage_folder(&client, "chris@example.test", RunOptions::default())
        .await
        .unwrap();
    assert_eq!(summary.by_model, 1);

    assert_not_logged(TRIAGE_SENDER);
    assert_not_logged(TRIAGE_SUBJECT);
    assert_not_logged(MODEL_REASON);
}
