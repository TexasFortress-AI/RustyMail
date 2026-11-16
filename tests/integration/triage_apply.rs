// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage apply against the fake IMAP server: every move is verified,
//! including against a server that answers OK without moving anything.

use super::fake_imap_server::{MoveBehavior, SharedMailbox};
use super::triage::{folder_uids, ids_in, inbox, runner, store, ACCOUNT};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use rustymail::triage::apply::{apply_account, ApplyOptions};
use rustymail::triage::folders::FolderMap;
use rustymail::triage::runner::RunOptions;
use rustymail::triage::store::{MessageRef, TriageStore};
use rustymail::triage::{Bucket, Proposal};
use serial_test::serial;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn default_options() -> ApplyOptions {
    ApplyOptions {
        min_confidence: 0.8,
        buckets: vec![
            Bucket::Spam,
            Bucket::Noise,
            Bucket::Accounting,
            Bucket::Legal,
            Bucket::Credit,
            Bucket::Alerts,
            Bucket::Trash,
        ],
        limit: None,
    }
}

async fn proposed_inbox() -> (
    ImapClient<AsyncImapSessionWrapper>,
    SharedMailbox,
    TriageStore,
) {
    let (client, mailbox) = inbox().await;
    let runner = runner(store().await, Arc::new(AtomicBool::new(false)));
    runner
        .triage_folder(&client, ACCOUNT, RunOptions::default())
        .await
        .unwrap();
    (client, mailbox, runner.store)
}

/// Cache rows for INBOX UIDs, as a sync would have written them.
async fn cache_inbox(store: &TriageStore, uids: &[u32]) {
    let pool = store.pool();
    sqlx::query(
        "INSERT INTO accounts (email_address, display_name, imap_host, imap_user, imap_pass)
         VALUES (?1, ?1, 'imap.example.test', ?1, 'pw')",
    )
    .bind(ACCOUNT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO folders (account_id, name) VALUES (?, 'INBOX')")
        .bind(ACCOUNT)
        .execute(pool)
        .await
        .unwrap();
    for uid in uids {
        sqlx::query("INSERT INTO emails (folder_id, uid) SELECT id, ? FROM folders")
            .bind(i64::from(*uid))
            .execute(pool)
            .await
            .unwrap();
    }
}

async fn cached_inbox(store: &TriageStore) -> Vec<i64> {
    sqlx::query_scalar("SELECT uid FROM emails ORDER BY uid")
        .fetch_all(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn apply_moves_verified_and_leaves_real_people_in_the_inbox() {
    let (client, mailbox, store) = proposed_inbox().await;
    cache_inbox(&store, &[1, 2, 3, 4]).await;
    let summary = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &default_options(),
    )
    .await
    .unwrap();

    assert_eq!(
        summary.moved, 3,
        "{:?} {:?}",
        summary.failures, summary.skipped
    );
    assert!(summary.failures.is_empty());
    assert_eq!(summary.created_folders, ["Noise", "Spam"]);
    assert_eq!(ids_in(&mailbox, "INBOX"), ["<friend@x.test>"]);
    assert_eq!(ids_in(&mailbox, "Noise"), ["<news@x.test>"]);
    assert_eq!(ids_in(&mailbox, "Spam"), ["<scam@x.test>"]);
    assert_eq!(ids_in(&mailbox, "Trash"), ["<code@x.test>"]);
    let commands = mailbox.lock().unwrap().commands.join("\n").to_uppercase();
    assert!(!commands.contains("\nEXPUNGE"), "no folder-wide expunge");
    for folder in ["NOISE", "SPAM"] {
        assert!(
            commands.contains(&format!("SUBSCRIBE \"{}\"", folder)),
            "created folders are subscribed so mail clients show them"
        );
    }

    let left = store.proposals_to_apply(ACCOUNT, "INBOX").await.unwrap();
    assert_eq!(left.iter().map(|p| p.uid).collect::<Vec<_>>(), [2]);
    assert_eq!(
        cached_inbox(&store).await,
        [2],
        "moved mail leaves the cache"
    );
    // A second apply has nothing left to move.
    let again = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &default_options(),
    )
    .await
    .unwrap();
    assert_eq!(again.moved, 0);
}

#[tokio::test]
#[serial]
async fn a_move_the_server_only_claims_is_reported_and_stays_pending() {
    let (client, mailbox, store) = proposed_inbox().await;
    mailbox.lock().unwrap().move_behavior = MoveBehavior::IgnoreMove;
    let summary = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &default_options(),
    )
    .await
    .unwrap();

    assert_eq!(summary.moved, 0);
    assert_eq!(summary.failures.len(), 3);
    assert!(summary
        .failures
        .iter()
        .all(|f| f.error.contains("still in INBOX")));
    assert_eq!(folder_uids(&mailbox, "INBOX"), [1, 2, 3, 4]);
    let pending = store.proposals_to_apply(ACCOUNT, "INBOX").await.unwrap();
    assert_eq!(pending.len(), 4);
    assert!(pending.iter().filter(|p| p.apply_error.is_some()).count() == 3);
}

#[tokio::test]
#[serial]
async fn a_message_lost_in_transit_is_reported() {
    let (client, mailbox, store) = proposed_inbox().await;
    mailbox.lock().unwrap().move_behavior = MoveBehavior::LoseMessage;
    let summary = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &default_options(),
    )
    .await
    .unwrap();
    assert_eq!(summary.moved, 0);
    assert_eq!(summary.failures.len(), 3);
    assert!(summary
        .failures
        .iter()
        .all(|f| f.error.contains("not found")));
}

#[tokio::test]
#[serial]
async fn without_a_trash_folder_codes_stay_put_and_low_confidence_is_skipped() {
    let (client, mailbox, store) = proposed_inbox().await;
    mailbox.lock().unwrap().folders.remove("Trash");
    let options = ApplyOptions {
        min_confidence: 0.99,
        ..default_options()
    };
    let summary = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &options,
    )
    .await
    .unwrap();
    assert_eq!(summary.moved, 1, "only the rule-based Noise moves");
    assert_eq!(summary.failures.len(), 1);
    assert!(summary.failures[0].error.contains("trash"));
    assert!(
        !mailbox.lock().unwrap().folders.contains_key("Trash"),
        "trash is never created"
    );
    assert!(ids_in(&mailbox, "INBOX").contains(&"<code@x.test>".to_string()));
    assert!(ids_in(&mailbox, "INBOX").contains(&"<scam@x.test>".to_string()));
}

#[tokio::test]
#[serial]
async fn unexpired_codes_wait() {
    let (client, mailbox) = inbox().await;
    let store = store().await;
    let mut fresh = Proposal::from_rule(Bucket::Trash, "one-time code");
    fresh.not_before = Some(chrono::Utc::now() + chrono::Duration::hours(1));
    let reference = MessageRef {
        account_id: ACCOUNT,
        folder: "INBOX",
        uidvalidity: 1,
        uid: 3,
        message_id: Some("<code@x.test>"),
        from_address: "no-reply@bank.test",
        subject: "Your sign-in code",
    };
    store.insert_proposal(&reference, &fresh).await.unwrap();
    let summary = apply_account(
        &store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &default_options(),
    )
    .await
    .unwrap();
    assert_eq!(summary.moved, 0);
    assert_eq!(summary.skipped.values().sum::<usize>(), 1);
    assert_eq!(folder_uids(&mailbox, "INBOX"), [1, 2, 3, 4]);
}
