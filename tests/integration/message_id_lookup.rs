// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A message is found by Message-ID even on a server whose SEARCH HEADER
//! finds nothing (the fake, like the mail.* hosts, returns no hits for it).
//! The Outbox copy of a sent message is removed this way.

use super::fake_imap_server::spawn_fake_imap;
use rustymail::imap::client::ImapClient;
use rustymail::imap::message_id_lookup::uids_with_message_id;
use rustymail::imap::session::AsyncImapSessionWrapper;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn finds_a_message_by_id_where_search_header_finds_nothing() {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let (port, _mailbox) = spawn_fake_imap(vec![
        (4, "<other@example.test>"),
        (5, "<sent-copy@example.test>"),
    ])
    .await;
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .expect("connect to fake IMAP");
    client.select_folder("INBOX").await.unwrap();

    let by_search = client
        .search_emails("HEADER Message-ID <sent-copy@example.test>")
        .await
        .unwrap_or_default();
    assert!(
        by_search.is_empty(),
        "the fake mimics servers whose SEARCH HEADER misses"
    );

    for id in ["<sent-copy@example.test>", "sent-copy@example.test"] {
        assert_eq!(
            uids_with_message_id(&client, id).await.unwrap(),
            vec![5],
            "{id}"
        );
    }
    assert!(uids_with_message_id(&client, "<absent@example.test>")
        .await
        .unwrap()
        .is_empty());
}
