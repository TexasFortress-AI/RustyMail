// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! TriageMessage parsing and the cheap header rules.

use rustymail::triage::message::TriageMessage;
use rustymail::triage::rules::{apply_rules, bulk_mail_default, SenderRules};
use rustymail::triage::{Bucket, Proposal, ProposalSource};

/// How long a code or reset stays useful when it states no expiry.
fn code_window() -> chrono::Duration {
    chrono::Duration::hours(24)
}

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
    apply_rules(message, &SenderRules::default(), code_window()).map(|p| p.bucket)
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

fn model(bucket: Bucket) -> Proposal {
    Proposal {
        bucket,
        confidence: Some(0.9),
        source: ProposalSource::Model,
        model_id: Some("m".into()),
        reason: "model".into(),
        not_before: None,
        list_name: None,
        document: None,
    }
}

const UNSUBSCRIBE: &str = "List-Unsubscribe: <mailto:u@example.test>\r\n";

#[test]
fn bulk_headers_alone_are_left_to_the_model() {
    assert_eq!(bucket(&parse(UNSUBSCRIBE, "news@example.test")), None);
    assert_eq!(
        bucket(&parse("Precedence: bulk\r\n", "a@example.test")),
        None
    );
}

#[test]
fn bulk_mail_is_noise_unless_the_model_says_receipt_or_alert() {
    let news = parse(UNSUBSCRIBE, "news@example.test");
    for bucket in [
        Bucket::Keep,
        Bucket::NeedsReply,
        Bucket::Spam,
        Bucket::Noise,
    ] {
        let p = bulk_mail_default(&news, model(bucket));
        assert_eq!(p.bucket, Bucket::Noise, "{bucket}");
        assert_eq!(p.source, ProposalSource::Rule, "{bucket}");
        assert_eq!(p.reason, "List-Unsubscribe header");
        assert_eq!(p.confidence, None);
    }
    for bucket in [
        Bucket::Accounting,
        Bucket::Legal,
        Bucket::Credit,
        Bucket::Alerts,
    ] {
        let p = bulk_mail_default(&news, model(bucket));
        assert_eq!((p.bucket, p.source), (bucket, ProposalSource::Model));
    }
    let bulk = parse("Precedence: bulk\r\n", "a@example.test");
    assert_eq!(
        bulk_mail_default(&bulk, model(Bucket::Keep)).bucket,
        Bucket::Noise
    );
}

#[test]
fn without_bulk_headers_the_model_decides() {
    for extra in ["", "Precedence: first-class\r\n"] {
        let p = bulk_mail_default(&parse(extra, "a@example.test"), model(Bucket::Keep));
        assert_eq!((p.bucket, p.source), (Bucket::Keep, ProposalSource::Model));
    }
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
fn dmarc_fail_counts_only_under_an_enforced_policy() {
    for policy in [
        "policy.dmarc=quarantine",
        "policy.dmarc=reject",
        "(p=REJECT sp=NONE dis=REJECT)",
    ] {
        let headers = format!(
            "Authentication-Results: mx.example.test; spf=pass; dmarc=fail {policy} header.from=a.test\r\n"
        );
        assert_eq!(
            bucket(&parse(&headers, "a@a.test")),
            Some(Bucket::Spam),
            "{policy}"
        );
    }
    for policy in ["policy.dmarc=none", "(p=NONE sp=NONE dis=NONE)"] {
        let headers = format!(
            "Authentication-Results: mx.example.test; spf=pass; dmarc=fail (policy not aligned) {policy} header.from=a.test\r\n"
        );
        assert_eq!(bucket(&parse(&headers, "a@a.test")), None, "{policy}");
    }
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
        r#"{"addresses": {"Orders@Shop.example": "Accounting"}, "domains": {"github.com": "alerts"}}"#,
    )
    .unwrap();
    let receipt = parse(
        "List-Unsubscribe: <mailto:u@shop.example>\r\n",
        "orders@shop.example",
    );
    let proposal = apply_rules(&receipt, &rules, code_window()).unwrap();
    assert_eq!(proposal.bucket, Bucket::Accounting);
    assert_eq!(proposal.reason, "known sender orders@shop.example");
    let subdomain = parse("", "noreply@notifications.github.com");
    assert_eq!(
        apply_rules(&subdomain, &rules, code_window())
            .unwrap()
            .bucket,
        Bucket::Alerts
    );
    assert!(apply_rules(&parse("", "someone@example.test"), &rules, code_window()).is_none());
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

#[test]
fn a_debt_collectors_notice_is_credit_even_when_the_disclosure_is_at_the_end() {
    // Collectors put the required disclosure far below the excerpt the
    // models read; it is found in the whole body.
    let body = format!(
        "Take action to restore your services. {}This is an attempt to collect a debt and any \
         information obtained will be used for that purpose. This communication is from a\r\n\
         DEBT COLLECTOR.",
        "Your account balance is past due. ".repeat(40)
    );
    let notice =
        TriageMessage::from_raw(1, &raw("", "notices@collector.example", &body), 100).unwrap();
    assert!(notice.debt_collector_notice);
    assert!(!notice
        .body_excerpt
        .to_lowercase()
        .contains("debt collector"));
    let proposal = apply_rules(&notice, &SenderRules::default(), code_window()).unwrap();
    assert_eq!(proposal.bucket, Bucket::Credit);
    assert_eq!(proposal.source, ProposalSource::Rule);
}

#[test]
fn a_mortgage_statements_collection_disclosure_is_not_a_collector() {
    // Servicers' statements say "an attempt to collect a debt"; they are
    // Accounting, decided by the model, not Credit.
    let statement = TriageMessage::from_raw(
        1,
        &raw(
            "",
            "statements@servicer.example",
            "Your mortgage statement is ready. This is an attempt to collect a debt.",
        ),
        100,
    )
    .unwrap();
    assert!(!statement.debt_collector_notice);
    assert_eq!(bucket(&statement), None);
}
