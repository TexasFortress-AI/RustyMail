// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Sending must not deadlock on the account-service lock. send_email read the
//! account while holding that lock and kept holding it while saving to the
//! Outbox, whose helper takes the same lock: every send hung until the 40 s
//! APPEND timeout. With the lock released, a send to an unreachable server
//! fails at once with a connection error.

use rustymail::dashboard::services::account::{Account, AccountService};
use rustymail::dashboard::services::smtp::{SendEmailRequest, SmtpService};
use rustymail::imap::CloneableImapSessionFactory;
use serial_test::serial;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

const ACCOUNT: &str = "sender@example.test";

/// An existing account whose IMAP and SMTP servers refuse connections.
fn unreachable_account(port: u16) -> Account {
    Account {
        email_address: ACCOUNT.to_string(),
        id: ACCOUNT.to_string(),
        display_name: "Sender".to_string(),
        provider_type: None,
        imap_host: "127.0.0.1".to_string(),
        imap_port: port as i64,
        imap_user: ACCOUNT.to_string(),
        imap_pass: "pw".to_string(),
        imap_use_tls: true,
        imap_use_starttls: false,
        smtp_host: Some("127.0.0.1".to_string()),
        smtp_port: Some(port as i64),
        smtp_user: Some(ACCOUNT.to_string()),
        smtp_pass: Some("pw".to_string()),
        smtp_use_tls: Some(true),
        smtp_use_starttls: Some(false),
        is_active: true,
        is_default: true,
        connection_status: None,
        oauth_provider: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
        oauth_token_expiry: None,
    }
}

#[tokio::test]
#[serial]
async fn sending_fails_fast_instead_of_deadlocking() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("accounts.db");
    std::fs::File::create(&db_path).unwrap();
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    // A port nobody listens on: bind, note it, release it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut accounts = AccountService::new(dir.path().join("accounts.json").to_str().unwrap());
    accounts.initialize(pool).await.unwrap();
    accounts
        .create_account(unreachable_account(port))
        .await
        .unwrap();
    let accounts = Arc::new(TokioMutex::new(accounts));
    let factory = CloneableImapSessionFactory::new(Box::new(|| {
        Box::pin(async {
            Err(rustymail::imap::error::ImapError::Connection(
                "unused default session".into(),
            ))
        })
    }));
    let smtp = SmtpService::new(accounts, factory);

    let started = Instant::now();
    let request = SendEmailRequest {
        to: vec!["someone@example.test".to_string()],
        cc: None,
        bcc: None,
        subject: "lock check".to_string(),
        body: "body".to_string(),
        body_html: None,
    };
    let result = tokio::time::timeout(Duration::from_secs(15), smtp.send_email(ACCOUNT, request))
        .await
        .expect("send_email hung: the account-service lock is still held");
    let error = result.expect_err("nothing listens on the port").to_string();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
    assert!(
        !error.contains("timed out"),
        "deadlocked until the APPEND timeout: {error}"
    );
}
