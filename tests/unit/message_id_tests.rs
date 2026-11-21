// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Sent mail carries a unique Message-ID on the sender's domain, never the
//! sending computer's hostname.

use rustymail::dashboard::services::message_id::new_message_id;

#[test]
fn a_message_id_is_unique_and_on_the_senders_domain() {
    let first = new_message_id("chris@example.com");
    let second = new_message_id("chris@example.com");
    assert_ne!(first, second);
    for id in [&first, &second] {
        assert!(id.starts_with('<') && id.ends_with("@example.com>"), "{id}");
        let local = &id[1..id.len() - "@example.com>".len()];
        assert!(
            local.len() >= 32 && local.chars().all(|c| c.is_ascii_hexdigit()),
            "{id}"
        );
    }
}

#[test]
fn lettre_writes_the_id_into_the_message() {
    let id = new_message_id("chris@example.com");
    let message = lettre::Message::builder()
        .from("chris@example.com".parse().unwrap())
        .to("someone@example.org".parse().unwrap())
        .message_id(Some(id.clone()))
        .subject("hi")
        .body(String::from("body"))
        .unwrap();
    let formatted = String::from_utf8(message.formatted()).unwrap();
    assert!(
        formatted.contains(&format!("Message-ID: {id}")),
        "{formatted}"
    );
}
