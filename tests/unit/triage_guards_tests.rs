// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Known correspondents and the checks that overrule a model's bucket.

use chrono::{DateTime, Duration, Utc};
use rustymail::triage::guards::ModelGuards;
use rustymail::triage::message::TriageMessage;
use rustymail::triage::people::{owner_accounts, KnownPeople};
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
        document: None,
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

    let known = KnownPeople::load(&pool, &[ACCOUNT.to_string()])
        .await
        .unwrap();
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
        "only the accounts given"
    );
    // Someone written to from another of the owner's accounts is a
    // correspondent here too (a father written to from the work account).
    let owner = owner_accounts(&["other@example.test".to_string()], ACCOUNT);
    let pooled = KnownPeople::load(&pool, &owner).await.unwrap();
    assert!(pooled.contains("other-account@example.test"));
    assert!(pooled.contains("ann@example.test"));
    assert!(
        pooled.contains("other@example.test"),
        "the owner's own address"
    );
    assert_eq!(known.len(), 4);
}

#[test]
fn a_stranger_asking_something_can_need_a_reply() {
    // A recruiter's job offer comes from someone the owner has never written
    // to; the models tell cold sales pitches (Spam) from personal requests,
    // so the guard no longer turns a stranger's request into Keep.
    let known = KnownPeople::new(["pat@example.test"]);
    let recruiter = checked(
        model(Bucket::NeedsReply),
        "jonathan@onepay.example",
        Some(1),
        &known,
    );
    assert_eq!(recruiter.bucket, Bucket::NeedsReply);
    // Spam from someone the owner writes to still stays in the INBOX.
    let colleague = checked(model(Bucket::Spam), "pat@example.test", Some(1), &known);
    assert_eq!(colleague.bucket, Bucket::Keep);
}

#[test]
fn a_message_you_already_answered_is_not_needs_reply() {
    let known = KnownPeople::new(["pat@example.test"]);
    let mut m = message("pat@example.test", Some(1));
    m.answered = true;
    let p = guards().check(model(Bucket::NeedsReply), &m, &known, now());
    assert_eq!(p.bucket, Bucket::Keep);
    assert!(p.reason.contains("already replied"), "{}", p.reason);
}

#[test]
fn replies_are_matched_by_message_id_with_or_without_brackets() {
    let known = KnownPeople::default().with_replies(["<a1@x.test>", " b2@x.test "]);
    assert!(known.replied_to(Some("a1@x.test")));
    assert!(known.replied_to(Some("<b2@x.test>")));
    assert!(!known.replied_to(Some("c3@x.test")));
    assert!(!known.replied_to(None));
}

#[test]
fn the_answered_flag_name_matches_what_the_imap_library_reports() {
    // fetch_flags records format!("{:?}", flag); a library change here would
    // silently stop answered mail leaving Needs Reply.
    assert_eq!(
        format!("{:?}", async_imap::types::Flag::Answered),
        rustymail::triage::answered::ANSWERED_FLAG
    );
}

fn in_thread(from: &str, arrived_days_ago: i64) -> TriageMessage {
    let raw = format!(
        "From: {from}\r\nTo: {ACCOUNT}\r\nSubject: Re: Account\r\nMessage-ID: <later@x.test>\r\n\
         References: <root@x.test> <middle@x.test>\r\nContent-Type: text/plain\r\n\r\nHello\r\n"
    );
    let mut message = TriageMessage::from_raw(1, raw.as_bytes(), 100).unwrap();
    message.received_at = Some(now() - Duration::days(arrived_days_ago));
    message
}

#[test]
fn writing_later_in_the_conversation_answers_earlier_messages() {
    let known =
        KnownPeople::default().with_sent_in_thread("root@x.test", now() - Duration::days(1));
    // Arrived before the owner's latest message in the thread: answered.
    assert!(known.answered(&in_thread("pat@example.test", 3)));
    // Arrived after it: still waiting on the owner.
    let newer =
        KnownPeople::default().with_sent_in_thread("root@x.test", now() - Duration::days(5));
    assert!(!newer.answered(&in_thread("pat@example.test", 3)));
    // Another conversation does not count.
    let other = KnownPeople::default().with_sent_in_thread("elsewhere@x.test", now());
    assert!(!other.answered(&in_thread("pat@example.test", 3)));
}

#[test]
fn the_conversation_root_is_the_first_reference() {
    let m = in_thread("pat@example.test", 1);
    assert_eq!(m.thread_root().as_deref(), Some("root@x.test"));
    assert_eq!(
        rustymail::triage::message::thread_root(None, Some("<parent@x>"), Some("own@x")).as_deref(),
        Some("parent@x")
    );
    assert_eq!(
        rustymail::triage::message::thread_root(None, None, Some("own@x")).as_deref(),
        Some("own@x")
    );
}

#[test]
fn a_question_not_left_for_a_reply_is_filed_as_its_document() {
    // "Signed contract", too old for Needs Reply: Legal, not the INBOX.
    let known = KnownPeople::new(["drreed@example.test"]);
    let mut contract = model(Bucket::NeedsReply);
    contract.document = Some((Bucket::Legal, 0.81));
    let p = checked(contract, "drreed@example.test", Some(200), &known);
    assert_eq!(p.bucket, Bucket::Legal);
    assert_eq!(p.confidence, Some(0.81));
    assert!(p.reason.contains("filed as Legal"), "{}", p.reason);
    // Without a document type it stays in the INBOX, as before.
    let p = checked(
        model(Bucket::NeedsReply),
        "drreed@example.test",
        Some(200),
        &known,
    );
    assert_eq!(p.bucket, Bucket::Keep);
}

#[test]
fn the_owners_accounts_include_the_one_being_triaged() {
    let configured = vec!["a@x.test".to_string(), "b@x.test".to_string()];
    assert_eq!(owner_accounts(&configured, "B@x.test"), configured);
    assert_eq!(
        owner_accounts(&configured, "c@x.test"),
        vec!["a@x.test", "b@x.test", "c@x.test"]
    );
    assert_eq!(owner_accounts(&[], "c@x.test"), vec!["c@x.test"]);
}
