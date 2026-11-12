// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Moves and deletes must never purge other messages already marked \Deleted.
//!
//! A message the user flagged \Deleted (but did not purge) sits in INBOX
//! next to the message being moved or deleted. Before the fix, the COPY
//! fallback and delete_messages sent a folder-wide EXPUNGE and destroyed it.

use super::fake_imap_server::{spawn_fake_imap, MoveBehavior, SharedMailbox};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::{AsyncImapOps, AsyncImapSessionWrapper};
use serial_test::serial;

const TARGET_UID: u32 = 1;
const FLAGGED_BY_USER_UID: u32 = 2;
const UNTOUCHED_UID: u32 = 3;

/// INBOX holds a target, a message the user already flagged \Deleted, and
/// one with no flags. The server has no MOVE, so moves use COPY + EXPUNGE.
async fn mailbox(
    uidplus: bool,
    user_flagged: bool,
) -> (ImapClient<AsyncImapSessionWrapper>, SharedMailbox) {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let (port, mailbox) = spawn_fake_imap(vec![
        (TARGET_UID, "<target@example.test>"),
        (FLAGGED_BY_USER_UID, "<flagged@example.test>"),
        (UNTOUCHED_UID, "<untouched@example.test>"),
    ])
    .await;
    {
        let mut state = mailbox.lock().unwrap();
        state.move_behavior = MoveBehavior::NoMoveCommand;
        state.uidplus = uidplus;
        if user_flagged {
            state
                .deleted
                .insert(("INBOX".to_string(), FLAGGED_BY_USER_UID));
        }
    }
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .expect("connect to fake IMAP");
    (client, mailbox)
}

fn inbox_uids(mailbox: &SharedMailbox) -> Vec<u32> {
    let state = mailbox.lock().unwrap();
    let mut uids: Vec<u32> = state.folders["INBOX"].iter().map(|m| m.uid).collect();
    uids.sort();
    uids
}

fn archive_count(mailbox: &SharedMailbox) -> usize {
    mailbox.lock().unwrap().folders["Archive"].len()
}

fn sent_plain_expunge(mailbox: &SharedMailbox) -> bool {
    mailbox
        .lock()
        .unwrap()
        .commands
        .iter()
        .any(|c| c.trim().eq_ignore_ascii_case("EXPUNGE"))
}

#[tokio::test]
#[serial]
async fn copy_fallback_move_keeps_other_deleted_messages_with_uidplus() {
    let (client, mailbox) = mailbox(true, true).await;
    client
        .move_email(TARGET_UID, "INBOX", "Archive")
        .await
        .unwrap();
    assert_eq!(
        inbox_uids(&mailbox),
        vec![FLAGGED_BY_USER_UID, UNTOUCHED_UID]
    );
    assert_eq!(archive_count(&mailbox), 1);
    assert!(!sent_plain_expunge(&mailbox));
}

#[tokio::test]
#[serial]
async fn batch_move_keeps_other_deleted_messages_with_uidplus() {
    let (client, mailbox) = mailbox(true, true).await;
    client
        .session()
        .move_messages(&[TARGET_UID], "INBOX", "Archive")
        .await
        .unwrap();
    assert_eq!(
        inbox_uids(&mailbox),
        vec![FLAGGED_BY_USER_UID, UNTOUCHED_UID]
    );
    assert_eq!(archive_count(&mailbox), 1);
}

#[tokio::test]
#[serial]
async fn delete_keeps_other_deleted_messages_with_uidplus() {
    let (client, mailbox) = mailbox(true, true).await;
    client.select_folder("INBOX").await.unwrap();
    client.delete_messages(&[TARGET_UID]).await.unwrap();
    assert_eq!(
        inbox_uids(&mailbox),
        vec![FLAGGED_BY_USER_UID, UNTOUCHED_UID]
    );
}

/// Without UIDPLUS a plain EXPUNGE would also purge the user's message, so
/// the target is copied and flagged but left in place.
#[tokio::test]
#[serial]
async fn without_uidplus_nothing_is_expunged_when_others_are_flagged() {
    let (client, mailbox) = mailbox(false, true).await;
    client
        .move_email(TARGET_UID, "INBOX", "Archive")
        .await
        .unwrap();
    assert_eq!(
        inbox_uids(&mailbox),
        vec![TARGET_UID, FLAGGED_BY_USER_UID, UNTOUCHED_UID]
    );
    assert_eq!(archive_count(&mailbox), 1);
    assert!(!sent_plain_expunge(&mailbox));
}

/// Without UIDPLUS, a plain EXPUNGE is still used when only the targets
/// are flagged, so the move completes.
#[tokio::test]
#[serial]
async fn without_uidplus_plain_expunge_runs_when_only_targets_are_flagged() {
    let (client, mailbox) = mailbox(false, false).await;
    client
        .move_email(TARGET_UID, "INBOX", "Archive")
        .await
        .unwrap();
    assert_eq!(
        inbox_uids(&mailbox),
        vec![FLAGGED_BY_USER_UID, UNTOUCHED_UID]
    );
    assert_eq!(archive_count(&mailbox), 1);
    assert!(sent_plain_expunge(&mailbox));
}
