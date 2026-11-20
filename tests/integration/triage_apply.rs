// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage apply against the fake IMAP server: every move is verified,
//! including against a server that answers OK without moving anything.

use super::fake_imap_server::{MoveBehavior, SharedMailbox};
use super::triage::{folder_uids, ids_in, inbox, raw, runner, store, ACCOUNT};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use rustymail::triage::answered::release_answered;
use rustymail::triage::apply::{apply_account, ApplyOptions};
use rustymail::triage::folders::FolderMap;
use rustymail::triage::people::KnownPeople;
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
async fn a_lost_connection_stops_the_run_and_leaves_the_rest_pending() {
    let (client, mailbox, store) = proposed_inbox().await;
    mailbox.lock().unwrap().move_behavior = MoveBehavior::DropConnection;
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
    assert_eq!(summary.failures.len(), 1, "only the move in flight failed");
    assert!(summary.connection_lost.is_some());
    let pending = store.proposals_to_apply(ACCOUNT, "INBOX").await.unwrap();
    assert_eq!(pending.len(), 4);
    assert_eq!(
        pending.iter().filter(|p| p.apply_error.is_some()).count(),
        1,
        "moves never attempted carry no error"
    );
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

#[tokio::test]
#[serial]
async fn list_and_repo_mail_moves_to_a_folder_per_list() {
    let (client, mailbox) = inbox().await;
    {
        let mut state = mailbox.lock().unwrap();
        state.raw_bodies.insert(
            "<news@x.test>".to_string(),
            raw(
                "member@example.test",
                "Re: socket options",
                "List-Id: ZeroMQ development list <zeromq-dev.lists.zeromq.org>\r\n\
                 List-Post: <mailto:zeromq-dev@lists.zeromq.org>\r\n",
                "<news@x.test>",
                "A reply",
            ),
        );
        state.raw_bodies.insert(
            "<scam@x.test>".to_string(),
            raw(
                "notifications@github.com",
                "[TexasFortress-AI/RustyMail] A pull request",
                "List-Id: TexasFortress-AI/RustyMail <RustyMail.TexasFortress-AI.github.com>\r\n\
                 List-Post: noreply@github.com\r\n",
                "<scam@x.test>",
                "Merged",
            ),
        );
    }
    let runner = runner(store().await, Arc::new(AtomicBool::new(false)));
    runner
        .triage_folder(&client, ACCOUNT, RunOptions::default())
        .await
        .unwrap();
    let mut options = default_options();
    options.buckets.extend([Bucket::Lists, Bucket::Repos]);
    let summary = apply_account(
        &runner.store,
        &client,
        ACCOUNT,
        "INBOX",
        &FolderMap::default(),
        &options,
    )
    .await
    .unwrap();

    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert_eq!(ids_in(&mailbox, "Lists.zeromq-dev"), ["<news@x.test>"]);
    assert_eq!(ids_in(&mailbox, "Repos.RustyMail"), ["<scam@x.test>"]);
    let created = &summary.created_folders;
    let position = |name: &str| created.iter().position(|f| f == name).unwrap();
    for (parent, child) in [("Lists", "Lists.zeromq-dev"), ("Repos", "Repos.RustyMail")] {
        assert!(
            position(parent) < position(child),
            "parent first: {:?}",
            created
        );
    }
}

#[tokio::test]
#[serial]
async fn answered_mail_goes_back_from_needs_reply_to_the_inbox() {
    let (client, mailbox) = inbox().await;
    {
        // Three messages already in Needs Reply: one the server marks
        // \Answered, one sent mail replies to, one still unanswered.
        let mut state = mailbox.lock().unwrap();
        let inbox_messages = state.folders.remove("INBOX").unwrap();
        let (needs_reply, rest): (Vec<_>, Vec<_>) = inbox_messages.into_iter().partition(|m| {
            ["<friend@x.test>", "<news@x.test>", "<scam@x.test>"].contains(&m.message_id.as_str())
        });
        state.folders.insert("INBOX".to_string(), rest);
        state.folders.insert("Needs Reply".to_string(), needs_reply);
        state.answered.insert("<friend@x.test>".to_string());
    }
    let known = KnownPeople::default().with_replies(["<news@x.test>"]);
    let moved = release_answered(&client, ACCOUNT, &FolderMap::default(), &known, "INBOX")
        .await
        .unwrap();

    assert_eq!(moved, 2);
    let mut back = ids_in(&mailbox, "INBOX");
    back.sort();
    assert!(back.contains(&"<friend@x.test>".to_string()), "{:?}", back);
    assert!(back.contains(&"<news@x.test>".to_string()), "{:?}", back);
    assert_eq!(ids_in(&mailbox, "Needs Reply"), ["<scam@x.test>"]);
}

#[tokio::test]
#[serial]
async fn an_account_without_a_needs_reply_folder_has_nothing_to_release() {
    let (client, mailbox) = inbox().await;
    let moved = release_answered(
        &client,
        ACCOUNT,
        &FolderMap::default(),
        &KnownPeople::default(),
        "INBOX",
    )
    .await
    .unwrap();
    assert_eq!(moved, 0);
    assert!(!mailbox.lock().unwrap().folders.contains_key("Needs Reply"));
}
