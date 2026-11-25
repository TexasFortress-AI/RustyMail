// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A rescued message's subject loses the spam filter's tag, and nothing else.

use rustymail::triage::spam_tag::{untag_message, untagged_subject};

#[test]
fn the_usual_spam_tags_are_removed_from_a_subject() {
    let cases = [
        (
            "***SPAM***  One login to connect Glassdoor",
            "One login to connect Glassdoor",
        ),
        ("*****SPAM***** Invoice", "Invoice"),
        ("[SPAM] Weekly digest", "Weekly digest"),
        ("[spam]Weekly digest", "Weekly digest"),
    ];
    for (tagged, clean) in cases {
        assert_eq!(untagged_subject(tagged).as_deref(), Some(clean), "{tagged}");
    }
    for untouched in [
        "Spam filter settings",
        "Re: ***SPAM*** later",
        "[SPAMMY] x",
        "SPAM",
        "***",
    ] {
        assert_eq!(untagged_subject(untouched), None, "{untouched}");
    }
}

#[test]
fn only_the_subject_header_changes() {
    let raw = b"From: a@b.test\r\nSubject: ***SPAM*** Hello\r\nMessage-ID: <m@b.test>\r\n\r\nSubject: ***SPAM*** in the body\r\n";
    let clean = untag_message(raw).expect("tag removed");
    assert_eq!(
        String::from_utf8(clean).unwrap(),
        "From: a@b.test\r\nSubject: Hello\r\nMessage-ID: <m@b.test>\r\n\r\nSubject: ***SPAM*** in the body\r\n"
    );
    assert_eq!(untag_message(b"Subject: Hello\r\n\r\nbody"), None);
    assert_eq!(
        untag_message(b"From: a@b.test\r\n\r\nSubject: ***SPAM*** x"),
        None,
        "body only"
    );
}
