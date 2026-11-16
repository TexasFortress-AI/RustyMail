// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Sender rules that also match the subject, for senders whose mail splits
//! between buckets (a card's transaction notices vs. its declines).

use rustymail::triage::message::TriageMessage;
use rustymail::triage::rules::{apply_rules, SenderRules};
use rustymail::triage::Bucket;

const RULES: &str = r#"{
    "domains": {"card.example": "Alerts"},
    "subjects": [
        {"domain": "card.example", "subject": "^Transaction notification$", "bucket": "Accounting"},
        {"domain": "bank.example", "subject": "^You (sent|received) money with Zelle", "bucket": "Accounting"}
    ]
}"#;

fn message(from: &str, subject: &str) -> TriageMessage {
    let raw = format!(
        "From: {from}\r\nTo: me@example.test\r\nSubject: {subject}\r\n\
         Content-Type: text/plain\r\n\r\nDetails inside.\r\n"
    );
    TriageMessage::from_raw(7, raw.as_bytes(), 100).unwrap()
}

fn decided(from: &str, subject: &str) -> Option<Bucket> {
    let rules = SenderRules::from_json(RULES).unwrap();
    apply_rules(&message(from, subject), &rules, chrono::Duration::hours(24)).map(|p| p.bucket)
}

#[test]
fn a_matching_subject_decides_before_the_domain_rule() {
    assert_eq!(
        decided("notices@card.example", "Transaction notification"),
        Some(Bucket::Accounting)
    );
    assert_eq!(
        decided("notices@card.example", "Transaction decline notification"),
        Some(Bucket::Alerts),
        "other subjects fall through to the domain rule"
    );
}

#[test]
fn subjects_match_case_insensitively_and_on_subdomains() {
    assert_eq!(
        decided("alerts@notify.bank.example", "you sent money with Zelle"),
        Some(Bucket::Accounting)
    );
    assert_eq!(
        decided("alerts@notify.bank.example", "Your card was declined"),
        None,
        "no domain rule for the bank: the model decides"
    );
    assert_eq!(
        decided("alerts@notbank.example", "You sent money with Zelle"),
        None,
        "a look-alike domain is not a subdomain"
    );
}

#[test]
fn bad_subject_rules_are_rejected() {
    let bad_pattern =
        r#"{"subjects": [{"domain": "a.example", "subject": "(", "bucket": "Accounting"}]}"#;
    assert!(SenderRules::from_json(bad_pattern).is_err());
    let bad_bucket =
        r#"{"subjects": [{"domain": "a.example", "subject": "x", "bucket": "Invoices"}]}"#;
    assert!(SenderRules::from_json(bad_bucket).is_err());
}
