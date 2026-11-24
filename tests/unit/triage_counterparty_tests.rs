// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A trusted document waits for the owner when nobody behind it has dealt
//! with him before, as a payables clerk verifies a new vendor.

use chrono::{Duration, Utc};
use rustymail::triage::counterparty::{counterparty, history_key, registrable_domain};
use rustymail::triage::guards::ModelGuards;
use rustymail::triage::message::TriageMessage;
use rustymail::triage::people::KnownPeople;
use rustymail::triage::{Bucket, Proposal, ProposalSource};

fn platforms() -> Vec<String> {
    vec!["google.com".into(), "zoom.us".into()]
}

fn msg(from: &str, reply_to: Option<&str>) -> TriageMessage {
    let reply = reply_to
        .map(|r| format!("Reply-To: {r}\r\n"))
        .unwrap_or_default();
    let raw = format!("From: {from}\r\n{reply}Subject: Capital call notice\r\n\r\nView file");
    TriageMessage::from_raw(1, raw.as_bytes(), 200).unwrap()
}

#[test]
fn history_is_kept_per_organisation_or_per_address_at_mail_providers() {
    assert_eq!(registrable_domain("email.mg.mercury.com"), "mercury.com");
    assert_eq!(
        registrable_domain("bluegbuilding.co.uk"),
        "bluegbuilding.co.uk"
    );
    assert_eq!(history_key("hello@Mercury.com"), Some("mercury.com".into()));
    // One Gmail or nifty.com user says nothing about another.
    assert_eq!(
        history_key("robert@gmail.com"),
        Some("robert@gmail.com".into())
    );
    assert_eq!(history_key("ueno@nifty.com"), Some("ueno@nifty.com".into()));
}

#[test]
fn a_platforms_message_belongs_to_whoever_is_in_reply_to() {
    let share = msg(
        "drive-shares-dm-noreply@google.com",
        Some("Robert <robert@gmail.com>"),
    );
    assert_eq!(counterparty(&share, &platforms()), Some("robert@gmail.com"));
    // A Zoom Events "capital call" names nobody behind it.
    let event = msg("noreply-zoomevents@zoom.us", None);
    assert_eq!(counterparty(&event, &platforms()), None);
    let direct = msg("stefan@bluegbuilding.co.uk", None);
    assert_eq!(
        counterparty(&direct, &platforms()),
        Some("stefan@bluegbuilding.co.uk")
    );
}

#[test]
fn the_owner_deals_with_senders_he_wrote_to_or_approved() {
    let known = KnownPeople::new(["me@owner.test", "mehow@matterfi.test"])
        .with_approved(["hello@mercury.com", "noreply@uber.com"]);
    assert!(
        known.deals_with("notices@email.mercury.com"),
        "same organisation"
    );
    assert_eq!(known.approved_count("hello@mercury.com"), 1);
    assert!(
        known.deals_with("billy@matterfi.test"),
        "an organisation he writes to"
    );
    assert!(!known.deals_with("capital@revellecapital.test"));
    assert!(
        !known.deals_with("someone@gmail.com"),
        "not every Gmail user"
    );
}

fn model(bucket: Bucket) -> Proposal {
    Proposal {
        bucket,
        confidence: Some(0.95),
        source: ProposalSource::Model,
        model_id: Some("m".into()),
        reason: "capital call notice".into(),
        not_before: None,
        list_name: None,
        document: None,
    }
}

#[test]
fn a_trusted_document_from_a_new_counterparty_waits_for_the_owner() {
    let guards = ModelGuards {
        needs_reply_max_age: Duration::days(30),
    };
    let known = KnownPeople::new(["me@owner.test"]);
    let mut lure = msg("noreply-zoomevents@zoom.us", None);
    lure.new_counterparty = true;
    for bucket in [
        Bucket::Accounting,
        Bucket::Legal,
        Bucket::Credit,
        Bucket::Alerts,
    ] {
        let p = guards.check(model(bucket), &lure, &known, Utc::now());
        assert_eq!(p.bucket, Bucket::Keep, "{bucket}");
        assert!(p.reason.starts_with("new sender: verify"), "{}", p.reason);
    }
    // Other buckets are not held: Spam and Noise lose nothing.
    assert_eq!(
        guards
            .check(model(Bucket::Spam), &lure, &known, Utc::now())
            .bucket,
        Bucket::Spam
    );
    // Someone the owner deals with is filed as usual.
    let mut payment = msg("hello@mercury.com", None);
    payment.new_counterparty = false;
    let p = guards.check(model(Bucket::Accounting), &payment, &known, Utc::now());
    assert_eq!(p.bucket, Bucket::Accounting);
    // Rules are not held either.
    let rule = Proposal::from_rule(Bucket::Credit, "debt collector disclosure");
    assert_eq!(
        guards.check(rule, &lure, &known, Utc::now()).bucket,
        Bucket::Credit
    );
}

#[test]
fn each_apple_hide_my_email_sender_is_its_own_counterparty() {
    // Approving one relayed receipt must not vouch for every sender that
    // uses Apple's relay: each gets its own address there.
    let receipt = "receipts_at_shop_test_abc123@privaterelay.appleid.com";
    let stranger = "billing_at_evil_test_xyz789@privaterelay.appleid.com";
    assert_eq!(history_key(receipt).as_deref(), Some(receipt));
    let known = KnownPeople::default().with_approved([receipt]);
    assert!(known.deals_with(receipt));
    assert!(
        !known.deals_with(stranger),
        "another relay sender is a stranger"
    );
}
