// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Pure planning for triage: what to scan, where each bucket goes, which
//! proposals apply moves, and the TRIAGE_* settings.

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use rustymail::imap::special_folders::SpecialUse;
use rustymail::triage::apply::{decide, ApplyOptions, Skip};
use rustymail::triage::config::TriageConfig;
use rustymail::triage::folders::{bucket_namespace, special_use, Destination, FolderMap};
use rustymail::triage::scan::{advance, plan_scan, select_uids};
use rustymail::triage::store::{StoredProposal, Watermark};
use rustymail::triage::Bucket;
use std::collections::{HashMap, HashSet};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn first_run_scans_the_whole_folder_from_uid_one() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap();
    let plan = plan_scan(None, 5, None, today);
    assert_eq!(plan.criteria, "UID 1:*");
    assert_eq!(plan.above, 0);
}

#[test]
fn later_runs_start_above_the_watermark_unless_uidvalidity_changed() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap();
    let stored = Some(Watermark {
        uidvalidity: 5,
        last_uid: 6021,
    });
    let plan = plan_scan(stored, 5, None, today);
    assert_eq!((plan.criteria.as_str(), plan.above), ("UID 6022:*", 6021));
    let reset = plan_scan(stored, 6, None, today);
    assert_eq!((reset.criteria.as_str(), reset.above), ("UID 1:*", 0));
    let recent = plan_scan(stored, 5, Some(3), today);
    assert_eq!(
        (recent.criteria.as_str(), recent.above),
        ("SINCE 11-Nov-2026", 0)
    );
}

#[test]
fn select_skips_proposed_and_stale_uids_and_honours_the_limit() {
    let proposed: HashSet<u32> = [12].into_iter().collect();
    // `UID 11:*` also returns the newest message (10) when nothing is above.
    assert_eq!(
        select_uids(&[13, 10, 12, 11], 10, &proposed, None),
        vec![11, 13]
    );
    assert_eq!(
        select_uids(&[13, 11, 14], 10, &proposed, Some(2)),
        vec![11, 13]
    );
    assert_eq!(advance(Some(20), Some(15)), Some(20));
    assert_eq!(advance(None, Some(15)), Some(15));
}

#[test]
fn bucket_folders_follow_the_account_namespace() {
    let map = FolderMap::default();
    let nested = names(&["INBOX", "INBOX.Sent", "INBOX.Trash", "INBOX.Noise"]);
    assert_eq!(
        map.resolve("a@x.test", Bucket::Noise, &nested, None, "INBOX."),
        Destination::Existing("INBOX.Noise".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::NeedsReply, &nested, None, "INBOX."),
        Destination::Create("INBOX.Needs Reply".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Trash, &nested, None, "INBOX."),
        Destination::Existing("INBOX.Trash".into())
    );
    let flat = names(&["INBOX", "Sent", "Deleted Items"]);
    assert_eq!(
        map.resolve("a@x.test", Bucket::Spam, &flat, None, ""),
        Destination::Create("Spam".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Trash, &flat, None, ""),
        Destination::Existing("Deleted Items".into())
    );
}

#[test]
fn trash_is_never_created() {
    let map = FolderMap::default();
    assert!(matches!(
        map.resolve("a@x.test", Bucket::Trash, &names(&["INBOX"]), None, ""),
        Destination::Unavailable(_)
    ));
    let pinned = FolderMap::from_json(r#"{"a@x.test": {"Trash": "Bin"}}"#).unwrap();
    assert!(matches!(
        pinned.resolve("a@x.test", Bucket::Trash, &names(&["INBOX"]), None, ""),
        Destination::Unavailable(_)
    ));
}

#[test]
fn per_account_overrides_win() {
    let map =
        FolderMap::from_json(r#"{"A@X.test": {"Spam": "Junk Mail", "Trash": "Deleted Items"}}"#)
            .unwrap();
    let existing = names(&["INBOX", "INBOX.Trash", "Deleted Items", "Junk Mail"]);
    assert_eq!(
        map.resolve("a@x.test", Bucket::Spam, &existing, None, "INBOX."),
        Destination::Existing("Junk Mail".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Trash, &existing, None, "INBOX."),
        Destination::Existing("Deleted Items".into())
    );
    assert!(FolderMap::from_json(r#"{"a@x.test": {"Keep": "INBOX"}}"#).is_err());
}

fn proposal(bucket: &str, source: &str, confidence: Option<f64>) -> StoredProposal {
    StoredProposal {
        id: 1,
        account_id: "a@x.test".into(),
        folder: "INBOX".into(),
        uidvalidity: 5,
        uid: 10,
        message_id: Some("<m@x.test>".into()),
        from_address: "n@x.test".into(),
        subject: "s".into(),
        bucket: bucket.into(),
        confidence,
        source: source.into(),
        model_id: None,
        reason: "r".into(),
        not_before: None,
        list_name: None,
        apply_error: None,
        created_at: "2026-11-14 00:00:00".into(),
    }
}

fn options() -> ApplyOptions {
    ApplyOptions {
        min_confidence: 0.8,
        buckets: vec![Bucket::Spam, Bucket::Noise, Bucket::Trash],
        limit: None,
    }
}

#[test]
fn decide_moves_confident_due_proposals_only() {
    let now = Utc.with_ymd_and_hms(2026, 11, 14, 12, 0, 0).unwrap();
    let o = options();
    assert_eq!(
        decide(&proposal("Noise", "rule", None), 5, now, &o),
        Ok(Bucket::Noise)
    );
    assert_eq!(
        decide(&proposal("Spam", "model", Some(0.9)), 5, now, &o),
        Ok(Bucket::Spam)
    );
    assert_eq!(
        decide(&proposal("Spam", "model", Some(0.5)), 5, now, &o),
        Err(Skip::LowConfidence)
    );
    assert_eq!(
        decide(&proposal("Keep", "model", Some(0.99)), 5, now, &o),
        Err(Skip::StaysInInbox)
    );
    assert_eq!(
        decide(&proposal("Needs Reply", "model", Some(0.99)), 5, now, &o),
        Err(Skip::BucketNotApplied)
    );
    assert_eq!(
        decide(&proposal("Noise", "rule", None), 6, now, &o),
        Err(Skip::Stale)
    );
    let mut code = proposal("Trash", "rule", None);
    code.not_before = Some(now + Duration::minutes(5));
    assert_eq!(decide(&code, 5, now, &o), Err(Skip::NotYetDue));
    code.not_before = Some(now - Duration::minutes(5));
    assert_eq!(decide(&code, 5, now, &o), Ok(Bucket::Trash));
}

fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name| map.get(name).cloned()
}

#[test]
fn config_defaults_keep_real_people_in_the_inbox() {
    let config =
        TriageConfig::from_lookup(lookup(&[("REST_HOST", "127.0.0.1"), ("REST_PORT", "9437")]))
            .unwrap();
    assert_eq!(config.dashboard_url, "http://127.0.0.1:9437");
    assert!(!config.apply_buckets.contains(&Bucket::NeedsReply));
    assert!(config.apply_buckets.contains(&Bucket::Trash));
    assert!(config.apply_buckets.contains(&Bucket::Lists));
    assert!(config.apply_buckets.contains(&Bucket::Repos));
    assert_eq!(config.code_window, Duration::hours(24));
    assert_eq!(config.min_confidence, 0.8);
    assert_eq!(config.needs_reply_max_age, Duration::days(30));
    assert!(
        config.decision_settings().is_none(),
        "chat model unless set"
    );
}

#[test]
fn config_reads_triage_settings() {
    let config = TriageConfig::from_lookup(lookup(&[
        ("TRIAGE_DASHBOARD_URL", "http://127.0.0.1:9437/"),
        ("TRIAGE_ACCOUNTS", "A@X.test, b@x.test"),
        ("TRIAGE_APPLY_BUCKETS", "Spam, needs reply"),
        ("TRIAGE_CODE_WINDOW_HOURS", "6"),
        ("TRIAGE_MIN_CONFIDENCE", "0.9"),
        ("TRIAGE_NEEDS_REPLY_DAYS", "7"),
        ("TRIAGE_DECISION_URL", "http://127.0.0.1:11434"),
        ("TRIAGE_DECISION_MODEL", "autotrust-jev:9b"),
    ]))
    .unwrap();
    assert_eq!(config.dashboard_url, "http://127.0.0.1:9437");
    assert_eq!(config.accounts, vec!["a@x.test", "b@x.test"]);
    assert_eq!(config.apply_buckets, vec![Bucket::Spam, Bucket::NeedsReply]);
    assert_eq!(config.code_window, Duration::hours(6));
    assert_eq!(config.min_confidence, 0.9);
    assert_eq!(config.needs_reply_max_age, Duration::days(7));
    assert_eq!(
        config.decision_settings(),
        Some(("http://127.0.0.1:11434", "autotrust-jev:9b"))
    );
    let keep = TriageConfig::from_lookup(lookup(&[
        ("TRIAGE_DASHBOARD_URL", "http://h"),
        ("TRIAGE_APPLY_BUCKETS", "Keep"),
    ]));
    assert!(keep.is_err());
}

#[test]
fn the_servers_marked_trash_and_junk_win_over_old_folder_names() {
    // An account that kept an old server's INBOX.Trash next to the current
    // server's Deleted Items (\Trash) and Junk Mail (\Junk).
    let map = FolderMap::default();
    let existing = names(&[
        "INBOX",
        "INBOX.Trash",
        "INBOX.Noise",
        "Deleted Items",
        "Junk Mail",
    ]);
    assert_eq!(
        map.resolve(
            "a@x.test",
            Bucket::Trash,
            &existing,
            Some("Deleted Items"),
            "INBOX."
        ),
        Destination::Existing("Deleted Items".into())
    );
    assert_eq!(
        map.resolve(
            "a@x.test",
            Bucket::Spam,
            &existing,
            Some("Junk Mail"),
            "INBOX."
        ),
        Destination::Existing("Junk Mail".into())
    );
    assert_eq!(special_use(Bucket::Trash), Some(SpecialUse::Trash));
    assert_eq!(special_use(Bucket::Spam), Some(SpecialUse::Junk));
    assert_eq!(special_use(Bucket::Noise), None);
}

#[test]
fn overrides_win_over_the_servers_marked_folder() {
    let map = FolderMap::from_json(r#"{"a@x.test": {"Spam": "INBOX.Review"}}"#).unwrap();
    let existing = names(&["INBOX", "INBOX.Review", "Junk Mail"]);
    assert_eq!(
        map.resolve(
            "a@x.test",
            Bucket::Spam,
            &existing,
            Some("Junk Mail"),
            "INBOX."
        ),
        Destination::Existing("INBOX.Review".into())
    );
}

#[test]
fn receipts_is_the_legacy_name_for_accounting() {
    assert_eq!(Bucket::parse("Receipts"), Some(Bucket::Accounting));
    assert_eq!(Bucket::parse("accounting"), Some(Bucket::Accounting));
    assert_eq!(Bucket::parse("Legal"), Some(Bucket::Legal));
    assert_eq!(Bucket::parse("credit"), Some(Bucket::Credit));
    assert_eq!(Bucket::Accounting.as_str(), "Accounting");
    assert_eq!(Bucket::parse("Invoices"), None);

    // A proposal stored before the rename applies as Accounting.
    let o = ApplyOptions {
        buckets: vec![Bucket::Accounting],
        ..options()
    };
    let now = Utc.with_ymd_and_hms(2026, 11, 14, 12, 0, 0).unwrap();
    assert_eq!(
        decide(&proposal("Receipts", "model", Some(0.9)), 5, now, &o),
        Ok(Bucket::Accounting)
    );
}

#[test]
fn accounting_and_legal_folders_follow_the_account_namespace() {
    let map = FolderMap::default();
    let nested = names(&["INBOX", "INBOX.Accounting"]);
    assert_eq!(
        map.resolve("a@x.test", Bucket::Accounting, &nested, None, "INBOX."),
        Destination::Existing("INBOX.Accounting".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Legal, &nested, None, "INBOX."),
        Destination::Create("INBOX.Legal".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Credit, &nested, None, "INBOX."),
        Destination::Create("INBOX.Credit".into())
    );
}

#[test]
fn bucket_folders_use_the_namespace_they_are_given() {
    // Gmail: a stray "INBOX.Outbox" label must not put new folders under INBOX.
    let map = FolderMap::default();
    let gmail = names(&["INBOX", "INBOX.Outbox", "[Gmail]/Sent Mail"]);
    assert_eq!(
        map.resolve("a@x.test", Bucket::Noise, &gmail, None, ""),
        Destination::Create("Noise".into())
    );
    assert_eq!(
        map.resolve("a@x.test", Bucket::Noise, &gmail, None, "INBOX/"),
        Destination::Create("INBOX/Noise".into())
    );
}

#[test]
fn bucket_folders_keep_the_accounts_existing_prefix() {
    // A real INBOX namespace (opentransactions.org) wins.
    assert_eq!(
        bucket_namespace(&names(&["INBOX", "INBOX.Sent"]), "INBOX."),
        "INBOX."
    );
    // A "/" server whose triage folders are top-level "INBOX.Noise" etc.
    let stalwart = names(&[
        "INBOX",
        "INBOX.Alerts",
        "INBOX.Noise",
        "INBOX.Outbox",
        "Sent Items",
    ]);
    assert_eq!(bucket_namespace(&stalwart, ""), "INBOX.");
    assert_eq!(
        bucket_namespace(&names(&["INBOX", "INBOX.Receipts"]), ""),
        "INBOX."
    );
    // Gmail: only a stray Outbox label and a leftover trash; no bucket folders.
    let gmail = names(&["INBOX", "INBOX.Outbox", "INBOX.Trash", "[Gmail]/Sent Mail"]);
    assert_eq!(bucket_namespace(&gmail, ""), "");
}
