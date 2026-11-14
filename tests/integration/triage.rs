// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage end to end against the fake IMAP server, with a stub model:
//! proposing resumes from the watermark, and apply verifies every move,
//! including against a server that answers OK without moving anything.

use super::fake_imap_server::{spawn_fake_imap, MoveBehavior, SharedMailbox};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use rustymail::triage::apply::{apply_account, ApplyOptions};
use rustymail::triage::folders::FolderMap;
use rustymail::triage::model::{EscalatingClassifier, ModelBackend};
use rustymail::triage::rules::SenderRules;
use rustymail::triage::runner::{RunOptions, TriageRunner};
use rustymail::triage::store::{MessageRef, TriageStore};
use rustymail::triage::{Bucket, Proposal};
use serial_test::serial;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const ACCOUNT: &str = "chris@example.test";
const SMALL_MODEL: &str = "small-local";
const FETCH_BATCH: usize = 2;

/// Replies by sender address found in the prompt; fails while `down` is set.
#[derive(Clone)]
struct StubModel {
    down: Arc<AtomicBool>,
}

impl ModelBackend for StubModel {
    async fn complete(&self, _model: &str, _system: &str, user: &str) -> Result<String, String> {
        if self.down.load(Ordering::SeqCst) {
            return Err("model server unavailable".to_string());
        }
        let bucket = if user.contains("friend@example.test") {
            "Needs Reply"
        } else {
            "Spam"
        };
        Ok(format!(
            r#"{{"bucket": "{}", "confidence": 0.95, "reason": "stub"}}"#,
            bucket
        ))
    }
}

fn raw(from: &str, subject: &str, extra: &str, id: &str, body: &str) -> String {
    format!(
        "{extra}From: {from}\r\nTo: {ACCOUNT}\r\nSubject: {subject}\r\nMessage-ID: {id}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n"
    )
}

/// INBOX: a newsletter, a friend, an old sign-in code and a scam.
async fn inbox() -> (ImapClient<AsyncImapSessionWrapper>, SharedMailbox) {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let messages = vec![
        (1, "<news@x.test>"),
        (2, "<friend@x.test>"),
        (3, "<code@x.test>"),
        (4, "<scam@x.test>"),
    ];
    let (port, mailbox) = spawn_fake_imap(messages).await;
    {
        let mut state = mailbox.lock().unwrap();
        for (id, body) in [
            (
                "<news@x.test>",
                raw(
                    "news@shop.test",
                    "Deals",
                    "List-Unsubscribe: <mailto:u@shop.test>\r\n",
                    "<news@x.test>",
                    "Sale",
                ),
            ),
            (
                "<friend@x.test>",
                raw(
                    "friend@example.test",
                    "Dinner?",
                    "",
                    "<friend@x.test>",
                    "Free Friday?",
                ),
            ),
            (
                "<code@x.test>",
                raw(
                    "no-reply@bank.test",
                    "Your sign-in code",
                    "",
                    "<code@x.test>",
                    "Code 551203",
                ),
            ),
            (
                "<scam@x.test>",
                raw(
                    "prize@lottery.test",
                    "You won",
                    "",
                    "<scam@x.test>",
                    "Claim now",
                ),
            ),
        ] {
            state.raw_bodies.insert(id.to_string(), body);
        }
        state.internal_dates.insert(
            "<code@x.test>".to_string(),
            "01-Nov-2026 09:00:00 +0000".to_string(),
        );
        state.folders.insert("Trash".to_string(), Vec::new());
    }
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .expect("connect to fake IMAP");
    (client, mailbox)
}

async fn store() -> TriageStore {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    TriageStore::new(pool)
}

fn runner(store: TriageStore, down: Arc<AtomicBool>) -> TriageRunner<StubModel> {
    TriageRunner {
        store,
        classifier: EscalatingClassifier::new(StubModel { down }, SMALL_MODEL.into(), None, 0.7),
        senders: SenderRules::default(),
        code_window: chrono::Duration::hours(24),
        folder: "INBOX".into(),
        fetch_batch: FETCH_BATCH,
        excerpt_chars: 500,
    }
}

fn folder_uids(mailbox: &SharedMailbox, folder: &str) -> Vec<u32> {
    let state = mailbox.lock().unwrap();
    let mut uids: Vec<u32> = state.folders[folder].iter().map(|m| m.uid).collect();
    uids.sort();
    uids
}

fn ids_in(mailbox: &SharedMailbox, folder: &str) -> Vec<String> {
    let state = mailbox.lock().unwrap();
    state.folders.get(folder).map_or_else(Vec::new, |messages| {
        messages.iter().map(|m| m.message_id.clone()).collect()
    })
}

#[tokio::test]
#[serial]
async fn propose_reads_the_backlog_in_batches_and_resumes_from_the_watermark() {
    let (client, mailbox) = inbox().await;
    let down = Arc::new(AtomicBool::new(false));
    let runner = runner(store().await, down.clone());

    // First run: UIDs 1-2 only. Second run: UID 3 is decided by a rule, then
    // the model is down at UID 4. Third run: only UID 4 is left.
    let first = runner
        .triage_folder(
            &client,
            ACCOUNT,
            RunOptions {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!((first.examined, first.watermark), (2, Some(2)));
    down.store(true, Ordering::SeqCst);
    let second = runner
        .triage_folder(&client, ACCOUNT, RunOptions::default())
        .await
        .unwrap();
    assert!(second.stopped_early.is_some());
    assert_eq!(second.watermark, Some(3), "UID 3 was decided by a rule");

    down.store(false, Ordering::SeqCst);
    let third = runner
        .triage_folder(&client, ACCOUNT, RunOptions::default())
        .await
        .unwrap();
    assert_eq!(
        (third.examined, third.watermark),
        (1, Some(4)),
        "only UID 4 is new"
    );

    let proposals = runner
        .store
        .proposals_to_apply(ACCOUNT, "INBOX")
        .await
        .unwrap();
    let buckets: Vec<(u32, &str)> = proposals
        .iter()
        .map(|p| (p.uid, p.bucket.as_str()))
        .collect();
    assert_eq!(
        buckets,
        [(1, "Noise"), (2, "Needs Reply"), (3, "Trash"), (4, "Spam")]
    );
    // The code expires a day after its INTERNALDATE (01-Nov-2026 09:00 UTC).
    let arrived = chrono::DateTime::parse_from_rfc3339("2026-11-01T09:00:00Z").unwrap();
    assert_eq!(
        proposals[2].not_before,
        Some(arrived.with_timezone(&chrono::Utc) + chrono::Duration::hours(24))
    );
    // Proposing never changes the mailbox.
    assert_eq!(folder_uids(&mailbox, "INBOX"), [1, 2, 3, 4]);
}

fn default_options() -> ApplyOptions {
    ApplyOptions {
        min_confidence: 0.8,
        buckets: vec![
            Bucket::Spam,
            Bucket::Noise,
            Bucket::Receipts,
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

#[tokio::test]
#[serial]
async fn apply_moves_verified_and_leaves_real_people_in_the_inbox() {
    let (client, mailbox, store) = proposed_inbox().await;
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

    let left = store.proposals_to_apply(ACCOUNT, "INBOX").await.unwrap();
    assert_eq!(left.iter().map(|p| p.uid).collect::<Vec<_>>(), [2]);
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
