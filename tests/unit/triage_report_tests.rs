// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The report shows spam-folder proposals apart from INBOX ones: a bucket
//! there means where a message coming out of spam would go.

use rustymail::triage::report::format_report;
use rustymail::triage::store::StoredProposal;

fn proposal(folder: &str, uid: u32, bucket: &str, subject: &str) -> StoredProposal {
    StoredProposal {
        id: uid as i64,
        account_id: "me@example.test".into(),
        folder: folder.into(),
        uidvalidity: 1,
        uid,
        message_id: None,
        from_address: "sender@example.test".into(),
        subject: subject.into(),
        bucket: bucket.into(),
        confidence: Some(0.95),
        source: "model".into(),
        model_id: Some("m".into()),
        reason: "stub".into(),
        not_before: None,
        list_name: None,
        correction: None,
        apply_error: None,
        created_at: "2026-11-24".into(),
    }
}

#[test]
fn spam_folder_proposals_have_their_own_section_after_the_inbox() {
    let proposals = vec![
        proposal("Junk Mail", 7, "Noise", "Weekly digest"),
        proposal("INBOX", 3, "Accounting", "Invoice"),
        proposal("Junk Mail", 8, "Spam", "You won"),
    ];
    let report = format_report(&[], &proposals, "http://127.0.0.1:9437");
    let inbox = report
        .find("== me@example.test (1 proposals)")
        .expect(&report);
    let junk = report
        .find("== me@example.test Junk Mail: spam-folder review (2 proposals; Spam stays, anything else is a rescue)")
        .expect(&report);
    assert!(inbox < junk, "INBOX first:\n{report}");
    let invoice = report.find("Invoice").unwrap();
    let digest = report.find("Weekly digest").unwrap();
    assert!(
        invoice < junk && junk < digest,
        "rows under their own folder:\n{report}"
    );
}
