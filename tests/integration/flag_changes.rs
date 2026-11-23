// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Flags are refreshed FLAGS-only: everything on the first pass, then with
//! CONDSTORE only what changed, and nothing at all when the folder's
//! HIGHESTMODSEQ has not moved. FLAGS that do not parse are reported, not guessed.

use super::fake_imap_server::{spawn_fake_imap, SharedMailbox};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use serial_test::serial;

type Client = ImapClient<AsyncImapSessionWrapper>;

async fn server(count: u32, condstore: bool) -> (Client, SharedMailbox) {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let ids: Vec<String> = (1..=count)
        .map(|uid| format!("<m{uid}@example.test>"))
        .collect();
    let messages = (1..=count).zip(ids.iter().map(String::as_str)).collect();
    let (port, mailbox) = spawn_fake_imap(messages).await;
    mailbox.lock().unwrap().flags.condstore = condstore;
    let client = Client::connect("127.0.0.1", port, "user", "password")
        .await
        .expect("connect to fake IMAP");
    (client, mailbox)
}

fn seen(flags: &[(u32, Vec<String>)]) -> Vec<u32> {
    let mut uids: Vec<u32> = flags
        .iter()
        .filter(|(_, f)| f.iter().any(|name| name == "Seen"))
        .map(|(uid, _)| *uid)
        .collect();
    uids.sort_unstable();
    uids
}

fn flag_fetches(mailbox: &SharedMailbox) -> usize {
    let commands = &mailbox.lock().unwrap().commands;
    commands
        .iter()
        .filter(|c| c.contains("(UID FLAGS)"))
        .count()
}

#[tokio::test]
#[serial]
async fn condstore_fetches_everything_once_then_only_changes() {
    let (client, mailbox) = server(3, true).await;
    mailbox.lock().unwrap().flags.set_seen("INBOX", 2, true);

    let first = client.fetch_flag_changes("INBOX", None).await.unwrap();
    assert_eq!(first.flags.len(), 3, "first pass: every message");
    assert_eq!(
        seen(&first.flags),
        vec![2],
        "flags named as stored: \"Seen\""
    );
    let modseq = first
        .highest_modseq
        .expect("CONDSTORE reports HIGHESTMODSEQ");

    let fetches = flag_fetches(&mailbox);
    let quiet = client
        .fetch_flag_changes("INBOX", Some(modseq))
        .await
        .unwrap();
    assert!(quiet.flags.is_empty());
    assert_eq!(
        flag_fetches(&mailbox),
        fetches,
        "unchanged HIGHESTMODSEQ: no FETCH"
    );

    mailbox.lock().unwrap().flags.set_seen("INBOX", 3, true); // read elsewhere
    let changed = client
        .fetch_flag_changes("INBOX", Some(modseq))
        .await
        .unwrap();
    assert_eq!(changed.flags.len(), 1, "only the changed message");
    assert_eq!(seen(&changed.flags), vec![3]);
    assert!(changed.highest_modseq > Some(modseq));
}

#[tokio::test]
#[serial]
async fn without_condstore_every_flag_is_fetched_each_time() {
    let (client, mailbox) = server(3, false).await;
    mailbox.lock().unwrap().flags.set_seen("INBOX", 1, true);
    for since in [None, Some(5)] {
        let changes = client.fetch_flag_changes("INBOX", since).await.unwrap();
        assert_eq!(changes.highest_modseq, None);
        assert_eq!(changes.flags.len(), 3);
        assert_eq!(seen(&changes.flags), vec![1]);
    }
}

#[tokio::test]
#[serial]
async fn flags_that_do_not_parse_are_an_error_not_a_panic() {
    // Some servers send FLAGS the client cannot parse: the refresh fails
    // cleanly, so the caller keeps its stored HIGHESTMODSEQ and moves on.
    let (client, mailbox) = server(3, true).await;
    mailbox
        .lock()
        .unwrap()
        .flags
        .garbled
        .insert(("INBOX".to_string(), 2));
    assert!(client.fetch_flag_changes("INBOX", None).await.is_err());
}

#[tokio::test]
#[serial]
async fn an_empty_folder_fetches_nothing() {
    let (client, mailbox) = server(1, true).await;
    let changes = client.fetch_flag_changes("Archive", None).await.unwrap();
    assert!(changes.flags.is_empty());
    assert_eq!(flag_fetches(&mailbox), 0);
}
