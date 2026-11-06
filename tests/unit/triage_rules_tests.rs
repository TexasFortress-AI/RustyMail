// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! TriageMessage parsing and the cheap header rules.

use rustymail::triage::message::TriageMessage;
use rustymail::triage::rules::{apply_rules, SenderRules};
use rustymail::triage::{Bucket, ProposalSource};

fn raw(extra_headers: &str, from: &str, body: &str) -> Vec<u8> {
    format!(
        "{extra_headers}From: {from}\r\nTo: chris@example.test\r\nSubject: Hello there\r\n\
         Message-ID: <id-1@example.test>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}"
    )
    .into_bytes()
}

fn parse(extra_headers: &str, from: &str) -> TriageMessage {
    TriageMessage::from_raw(42, &raw(extra_headers, from, "Body text"), 100).unwrap()
}

fn bucket(message: &TriageMessage) -> Option<Bucket> {
    apply_rules(message, &SenderRules::default()).map(|p| p.bucket)
}

#[test]
fn parses_sender_subject_id_and_excerpt() {
    let body = "Line one\r\n\r\n   line   two and a long tail";
    let raw = raw("", "\"Jane Doe\" <Jane@Example.TEST>", body);
    let message = TriageMessage::from_raw(7, &raw, 18).unwrap();
    assert_eq!(message.uid, 7);
    assert_eq!(message.from_address, "jane@example.test");
    assert_eq!(message.from_name.as_deref(), Some("Jane Doe"));
    assert_eq!(message.from_domain(), Some("example.test"));
    assert_eq!(message.subject, "Hello there");
    assert_eq!(message.message_id.as_deref(), Some("id-1@example.test"));
    assert_eq!(message.body_excerpt, "Line one line two ");
}

#[test]
fn folded_header_is_unfolded_and_lookup_is_case_insensitive() {
    let message = parse(
        "List-Unsubscribe: <mailto:u@example.test>,\r\n <https://example.test/u>\r\n",
        "news@example.test",
    );
    assert_eq!(
        message.header("LIST-UNSUBSCRIBE"),
        Some("<mailto:u@example.test>, <https://example.test/u>")
    );
}

#[test]
fn list_unsubscribe_and_bulk_precedence_are_noise() {
    let noise = apply_rules(
        &parse(
            "List-Unsubscribe: <mailto:u@example.test>\r\n",
            "news@example.test",
        ),
        &SenderRules::default(),
    )
    .unwrap();
    assert_eq!(noise.bucket, Bucket::Noise);
    assert_eq!(noise.source, ProposalSource::Rule);
    assert_eq!(noise.confidence, None);
    assert_eq!(
        bucket(&parse("Precedence: bulk\r\n", "a@example.test")),
        Some(Bucket::Noise)
    );
    assert_eq!(
        bucket(&parse("Precedence: first-class\r\n", "a@example.test")),
        None
    );
}

#[test]
fn list_id_alone_is_left_to_the_model() {
    assert_eq!(
        bucket(&parse(
            "List-Id: <dev.lists.example.test>\r\n",
            "a@example.test"
        )),
        None
    );
}

#[test]
fn authentication_failures_are_spam() {
    let dmarc = "Authentication-Results: mx.example.test; spf=pass; dkim=pass; dmarc=fail\r\n";
    assert_eq!(bucket(&parse(dmarc, "a@example.test")), Some(Bucket::Spam));
    let spf = "Authentication-Results: mx.example.test; spf=fail smtp.mailfrom=x; dkim=none\r\n";
    assert_eq!(bucket(&parse(spf, "a@example.test")), Some(Bucket::Spam));
    let dkim = "Authentication-Results: mx.example.test; spf=none; dkim=fail\r\n";
    assert_eq!(bucket(&parse(dkim, "a@example.test")), Some(Bucket::Spam));
}

#[test]
fn passing_or_soft_authentication_is_not_spam() {
    let forwarded = "Authentication-Results: mx.example.test; spf=fail; dkim=pass\r\n";
    assert_eq!(bucket(&parse(forwarded, "a@example.test")), None);
    let softfail = "Authentication-Results: mx.example.test; spf=softfail; dkim=none\r\n";
    assert_eq!(bucket(&parse(softfail, "a@example.test")), None);
}

#[test]
fn only_the_topmost_authentication_results_counts() {
    // A sender can add a fake lower header; our server's header is on top.
    let headers = "Authentication-Results: mx.example.test; spf=pass; dkim=pass; dmarc=pass\r\n\
                   Authentication-Results: forged; dmarc=fail\r\n";
    assert_eq!(bucket(&parse(headers, "a@example.test")), None);
}

#[test]
fn known_senders_win_over_header_rules() {
    let rules = SenderRules::from_json(
        r#"{"addresses": {"Orders@Shop.example": "Receipts"}, "domains": {"github.com": "alerts"}}"#,
    )
    .unwrap();
    let receipt = parse(
        "List-Unsubscribe: <mailto:u@shop.example>\r\n",
        "orders@shop.example",
    );
    let proposal = apply_rules(&receipt, &rules).unwrap();
    assert_eq!(proposal.bucket, Bucket::Receipts);
    assert_eq!(proposal.reason, "known sender orders@shop.example");
    let subdomain = parse("", "noreply@notifications.github.com");
    assert_eq!(
        apply_rules(&subdomain, &rules).unwrap().bucket,
        Bucket::Alerts
    );
    assert!(apply_rules(&parse("", "someone@example.test"), &rules).is_none());
}

#[test]
fn sender_rules_reject_unknown_buckets() {
    let err = SenderRules::from_json(r#"{"domains": {"x.example": "Newsletters"}}"#).unwrap_err();
    assert!(err.contains("Newsletters"), "{}", err);
}

#[test]
fn plain_personal_mail_has_no_rule() {
    assert_eq!(bucket(&parse("", "friend@example.test")), None);
}
