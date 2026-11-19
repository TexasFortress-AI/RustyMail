// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Known correspondents and the checks that overrule a model's bucket.

use chrono::{DateTime, Duration, Utc};
use rustymail::triage::guards::ModelGuards;
use rustymail::triage::message::TriageMessage;
use rustymail::triage::people::KnownPeople;
use rustymail::triage::{Bucket, Proposal, ProposalSource};
use sqlx::sqlite::SqlitePoolOptions;

const ACCOUNT: &str = "me@example.test";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-14T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn guards() -> ModelGuards {
    ModelGuards {
        needs_reply_max_age: Duration::days(30),
    }
}

fn message(from: &str, received_days_ago: Option<i64>) -> TriageMessage {
    let raw = format!(
        "From: {from}\r\nTo: {ACCOUNT}\r\nSubject: Hi\r\nContent-Type: text/plain\r\n\r\nHello\r\n"
    );
    let mut message = TriageMessage::from_raw(1, raw.as_bytes(), 100).unwrap();
    message.received_at = received_days_ago.map(|days| now() - Duration::days(days));
    message
}

fn model(bucket: Bucket) -> Proposal {
    Proposal {
        bucket,
        confidence: Some(0.95),
        source: ProposalSource::Model,
        model_id: Some("m".into()),
        reason: "looks suspicious".into(),
        not_before: None,
        list_name: None,
    }
}

fn checked(proposal: Proposal, from: &str, days_ago: Option<i64>, known: &KnownPeople) -> Proposal {
    guards().check(proposal, &message(from, days_ago), known, now())
}

#[test]
fn a_known_sender_is_never_spam_or_noise() {
    let known = KnownPeople::new(["Colleague@Example.test"]);
    for bucket in [Bucket::Spam, Bucket::Noise] {
        let p = checked(model(bucket), "colleague@example.test", Some(1), &known);
        assert_eq!(p.bucket, Bucket::Keep, "{bucket}");
        assert!(p.reason.contains("correspond"), "{}", p.reason);
        assert!(
            p.reason.contains("looks suspicious"),
            "keeps the model's reason"
        );
    }
}

#[test]
fn other_buckets_from_a_known_sender_are_kept() {
    let known = KnownPeople::new(["colleague@example.test"]);
    for bucket in [
        Bucket::Accounting,
        Bucket::Legal,
        Bucket::Credit,
        Bucket::Alerts,
        Bucket::NeedsReply,
    ] {
        let p = checked(model(bucket), "colleague@example.test", Some(1), &known);
        assert_eq!(p.bucket, bucket);
    }
}

#[test]
fn an_unknown_sender_can_still_be_spam() {
    let known = KnownPeople::new(["colleague@example.test"]);
    let p = checked(
        model(Bucket::Spam),
        "stranger@example.test",
        Some(1),
        &known,
    );
    assert_eq!(p.bucket, Bucket::Spam);
}

#[test]
fn needs_reply_only_for_recent_mail() {
    let known = KnownPeople::new(["pat@example.test"]);
    let recent = checked(
        model(Bucket::NeedsReply),
        "pat@example.test",
        Some(29),
        &known,
    );
    assert_eq!(recent.bucket, Bucket::NeedsReply);

    let old = checked(
        model(Bucket::NeedsReply),
        "pat@example.test",
        Some(31),
        &known,
    );
    assert_eq!(old.bucket, Bucket::Keep);
    assert!(old.reason.contains("older than 30 days"), "{}", old.reason);

    let undated = checked(model(Bucket::NeedsReply), "pat@example.test", None, &known);
    assert_eq!(undated.bucket, Bucket::Keep);
}

#[test]
fn rule_proposals_are_never_overruled() {
    let known = KnownPeople::new(["colleague@example.test"]);
    let rule = Proposal::from_rule(Bucket::Spam, "DMARC fail in Authentication-Results");
    let p = checked(rule, "colleague@example.test", Some(1), &known);
    assert_eq!(p.bucket, Bucket::Spam, "a spoofed known sender stays Spam");
}

#[tokio::test]
async fn known_people_are_the_account_and_its_sent_recipients() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    for account in [ACCOUNT, "other@example.test"] {
        sqlx::query(
            "INSERT INTO accounts (email_address, display_name, imap_host, imap_user, imap_pass)
             VALUES (?, ?, 'imap.example.test', ?, 'x')",
        )
        .bind(account)
        .bind(account)
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    }
    let folders = [
        (1, ACCOUNT, "Sent Items"),
        (2, ACCOUNT, "INBOX.Sent"),
        (3, ACCOUNT, "INBOX"),
        (4, "other@example.test", "Sent"),
    ];
    for (id, account, name) in folders {
        sqlx::query("INSERT INTO folders (id, account_id, name) VALUES (?, ?, ?)")
            .bind(id)
            .bind(account)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    let emails = [
        (
            1,
            r#"["Ann@Example.test"]"#,
            Some(r#"["bob@example.test"]"#),
        ),
        (2, r#"["carol@example.test"]"#, None),
        (3, r#"["inbox-only@example.test"]"#, None),
        (4, r#"["other-account@example.test"]"#, None),
    ];
    for (folder, to, cc) in emails {
        sqlx::query(
            "INSERT INTO emails (folder_id, uid, to_addresses, cc_addresses) VALUES (?, ?, ?, ?)",
        )
        .bind(folder)
        .bind(folder)
        .bind(to)
        .bind(cc)
        .execute(&pool)
        .await
        .unwrap();
    }

    let known = KnownPeople::load(&pool, ACCOUNT).await.unwrap();
    for address in [
        ACCOUNT,
        "ann@example.test",
        "bob@example.test",
        "carol@example.test",
    ] {
        assert!(known.contains(address), "{address}");
    }
    assert!(
        !known.contains("inbox-only@example.test"),
        "only Sent folders count"
    );
    assert!(
        !known.contains("other-account@example.test"),
        "only this account"
    );
    assert_eq!(known.len(), 4);
}

#[test]
fn needs_reply_only_from_someone_you_correspond_with() {
    // Cold outreach reads like a person asking something; what sets a real
    // correspondent apart is that the owner has written to them.
    let known = KnownPeople::new(["pat@example.test"]);
    let stranger = checked(
        model(Bucket::NeedsReply),
        "sales@outreach.example",
        Some(1),
        &known,
    );
    assert_eq!(stranger.bucket, Bucket::Keep);
    assert!(stranger.reason.contains("not someone you correspond with"));
    let correspondent = checked(
        model(Bucket::NeedsReply),
        "pat@example.test",
        Some(1),
        &known,
    );
    assert_eq!(correspondent.bucket, Bucket::NeedsReply);
}
