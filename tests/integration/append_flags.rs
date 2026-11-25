// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! APPEND sends the flags it is given: a sent copy is \Seen, a draft is
//! \Draft. They used to be dropped, so every saved Sent copy showed unread.

use super::fake_imap_server::spawn_fake_imap;
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn append_sends_its_flags_and_stores_the_message() {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let (port, mailbox) = spawn_fake_imap(Vec::new()).await;
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .unwrap();
    let raw = b"Message-ID: <sent@x.test>\r\nSubject: Hello\r\n\r\nBody\r\n";
    client
        .append(
            "INBOX",
            raw,
            &["\\Seen".to_string(), "$NotJunk".to_string()],
        )
        .await
        .unwrap();
    client
        .append("INBOX", b"Message-ID: <bare@x.test>\r\n\r\nx\r\n", &[])
        .await
        .unwrap();

    let state = mailbox.lock().unwrap();
    assert_eq!(state.appended.len(), 2, "{:?}", state.commands);
    assert_eq!(
        state.appended[0].flags.as_deref(),
        Some("(\\Seen $NotJunk)")
    );
    assert_eq!(state.appended[0].message_id, "<sent@x.test>");
    assert_eq!(state.appended[1].flags, None, "no flags: no list");
}
