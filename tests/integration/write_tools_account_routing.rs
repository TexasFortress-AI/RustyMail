// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Proves EmailService write operations reach the named account's IMAP server
//! (never the default session), and that moves are verified before reporting success.
//! Each account gets a real TCP fake IMAP server, so routing is exercised end to end.

use super::fake_imap_server::{spawn_fake_imap, FakeMessage, MoveBehavior, SharedMailbox};
use rustymail::connection_pool::{ConnectionPool, ImapConnectionFactory, PoolConfig};
use rustymail::dashboard::services::account::{Account, AccountService};
use rustymail::dashboard::services::email::EmailService;
use rustymail::imap::error::ImapError;
use rustymail::imap::CloneableImapSessionFactory;
use serial_test::serial;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

const ACCOUNT_A: &str = "default-box@example.test";
const ACCOUNT_B: &str = "named-box@example.test";

fn fake_account(email: &str, port: u16) -> Account {
    Account {
        email_address: email.to_string(),
        id: email.to_string(),
        display_name: email.to_string(),
        provider_type: None,
        imap_host: "127.0.0.1".to_string(),
        imap_port: port as i64,
        imap_user: email.to_string(),
        imap_pass: "pw".to_string(),
        imap_use_tls: true,
        imap_use_starttls: false,
        smtp_host: None,
        smtp_port: None,
        smtp_user: None,
        smtp_pass: None,
        smtp_use_tls: None,
        smtp_use_starttls: None,
        is_active: true,
        is_default: false,
        connection_status: None,
        oauth_provider: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
        oauth_token_expiry: None,
    }
}

/// Two accounts on separate fake servers, plus a default session that must never be used.
struct RoutingHarness {
    service: EmailService,
    box_a: SharedMailbox,
    box_b: SharedMailbox,
    default_session_uses: Arc<AtomicUsize>,
    _config_dir: TempDir,
}

async fn harness(messages_in_b: Vec<(u32, &str)>) -> RoutingHarness {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let config_dir = TempDir::new().unwrap();
    let config_path = config_dir.path().join("accounts.json");
    let db_path = config_dir.path().join("accounts.db");
    std::fs::File::create(&db_path).unwrap();
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let (port_a, box_a) = spawn_fake_imap(vec![(1, "<a-only@example.test>")]).await;
    let (port_b, box_b) = spawn_fake_imap(messages_in_b).await;

    let mut accounts = AccountService::new(config_path.to_str().unwrap());
    accounts.initialize(pool).await.unwrap();
    accounts
        .create_account(fake_account(ACCOUNT_A, port_a))
        .await
        .unwrap();
    accounts
        .create_account(fake_account(ACCOUNT_B, port_b))
        .await
        .unwrap();

    let default_session_uses = Arc::new(AtomicUsize::new(0));
    let uses = default_session_uses.clone();
    let imap_factory = CloneableImapSessionFactory::new(Box::new(move || {
        uses.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ImapError::Connection("default session used".to_string())) })
    }));
    let pool_factory = Arc::new(ImapConnectionFactory::new(
        "127.0.0.1".to_string(),
        1,
        "unused".to_string(),
        "unused".to_string(),
    ));
    let connection_pool = ConnectionPool::new(pool_factory, PoolConfig::default());
    let service = EmailService::new(imap_factory, connection_pool)
        .with_account_service(Arc::new(TokioMutex::new(accounts)));

    RoutingHarness {
        service,
        box_a,
        box_b,
        default_session_uses,
        _config_dir: config_dir,
    }
}

fn uids_in(mailbox: &SharedMailbox, folder: &str) -> Vec<u32> {
    mailbox.lock().unwrap().folders[folder]
        .iter()
        .map(|m| m.uid)
        .collect()
}

fn sent_move(mailbox: &SharedMailbox) -> bool {
    mailbox
        .lock()
        .unwrap()
        .commands
        .iter()
        .any(|c| c.starts_with("UID MOVE"))
}

fn assert_only_b_used(h: &RoutingHarness) {
    assert_eq!(h.default_session_uses.load(Ordering::SeqCst), 0);
    assert_eq!(h.box_a.lock().unwrap().connections, 0, "account A touched");
    assert!(h.box_b.lock().unwrap().connections > 0, "account B unused");
}

#[tokio::test]
#[serial]
async fn move_reaches_only_the_named_account_and_is_verified() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let destination_verified = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();

    assert!(destination_verified);
    assert_only_b_used(&h);
    assert!(uids_in(&h.box_b, "INBOX").is_empty());
    assert_eq!(uids_in(&h.box_b, "Archive"), vec![9000]);
    assert_eq!(
        uids_in(&h.box_a, "INBOX"),
        vec![1],
        "same UID in A untouched"
    );
}

#[tokio::test]
#[serial]
async fn unknown_account_is_rejected_without_connecting() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", "nobody@example.test")
        .await;

    assert!(result.is_err());
    assert_eq!(h.default_session_uses.load(Ordering::SeqCst), 0);
    assert_eq!(h.box_a.lock().unwrap().connections, 0);
    assert_eq!(h.box_b.lock().unwrap().connections, 0);
}

#[tokio::test]
#[serial]
async fn missing_source_uid_fails_without_sending_move() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let error = h
        .service
        .atomic_move_message_for_account(777, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("777"), "got: {}", error);
    assert!(!sent_move(&h.box_b));
    assert_eq!(uids_in(&h.box_b, "INBOX"), vec![1]);
}

#[tokio::test]
#[serial]
async fn missing_destination_folder_fails_without_sending_move() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "NoSuchFolder", ACCOUNT_B)
        .await;

    assert!(result.is_err());
    assert!(!sent_move(&h.box_b));
    assert_eq!(uids_in(&h.box_b, "INBOX"), vec![1]);
}

#[tokio::test]
#[serial]
async fn ok_response_without_a_move_is_reported_as_failure() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::IgnoreMove;

    let error = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("still in"), "got: {}", error);
}

#[tokio::test]
#[serial]
async fn sender_controlled_message_id_is_never_sent_to_the_server() {
    let hostile = r#"<x"y) OR ALL@example.test>"#;
    let h = harness(vec![(1, hostile)]).await;

    let destination_verified = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();

    assert!(destination_verified);
    assert_eq!(uids_in(&h.box_b, "Archive"), vec![9000]);
    let commands = h.box_b.lock().unwrap().commands.clone();
    assert!(
        !commands.iter().any(|c| c.contains("OR ALL")),
        "{:?}",
        commands
    );
}

#[tokio::test]
#[serial]
async fn newest_unrelated_destination_message_does_not_satisfy_verification() {
    // `UID n:*` returns this message even though it predates the move.
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::LoseMessage;
    h.box_b
        .lock()
        .unwrap()
        .folders
        .get_mut("Archive")
        .unwrap()
        .push(FakeMessage {
            uid: 50,
            message_id: "<m1@example.test>".to_string(),
        });

    let error = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("not found in"), "got: {}", error);
}

#[tokio::test]
#[serial]
async fn batch_move_reports_moved_and_failed_uids() {
    let h = harness(vec![(1, "<m1@example.test>"), (2, "<m2@example.test>")]).await;

    let error = h
        .service
        .atomic_batch_move_for_account(&[1, 777, 2], "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("[1, 2]"), "moved UIDs missing: {}", error);
    assert!(error.contains("777"), "failed UID missing: {}", error);
    assert_only_b_used(&h);
    assert!(uids_in(&h.box_b, "INBOX").is_empty());
    assert_eq!(uids_in(&h.box_b, "Archive").len(), 2);
}

#[tokio::test]
#[serial]
async fn flag_and_expunge_operations_reach_only_the_named_account() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    let s = &h.service;

    s.mark_as_read_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.mark_as_unread_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.mark_as_deleted_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.undelete_messages_for_account("INBOX", &[1], ACCOUNT_B)
        .await
        .unwrap();
    s.expunge_for_account("INBOX", ACCOUNT_B).await.unwrap();

    assert_only_b_used(&h);
    let commands = h.box_b.lock().unwrap().commands.clone();
    let stores = commands.iter().filter(|c| c.starts_with("UID STORE"));
    assert_eq!(stores.count(), 4);
    assert!(commands.iter().any(|c| c.starts_with("EXPUNGE")));
}

#[tokio::test]
#[serial]
async fn failed_copy_never_deletes_the_same_uid_in_the_destination() {
    let h = harness(vec![(1, "<m1@example.test>")]).await;
    h.box_b.lock().unwrap().move_behavior = MoveBehavior::RefuseCopyAndMove;
    // UIDs are per-folder: this unrelated message happens to share the source UID.
    h.box_b
        .lock()
        .unwrap()
        .folders
        .get_mut("Archive")
        .unwrap()
        .push(FakeMessage {
            uid: 1,
            message_id: "<unrelated@example.test>".to_string(),
        });

    let result = h
        .service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await;

    assert!(result.is_err());
    let commands = h.box_b.lock().unwrap().commands.clone();
    assert!(
        !commands
            .iter()
            .any(|c| c.starts_with("UID STORE") || c.starts_with("EXPUNGE")),
        "rollback touched the destination: {:?}",
        commands
    );
}
