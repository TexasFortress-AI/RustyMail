// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Applying the spam-folder review: Spam stays, everything else is rescued to
//! its folder (Keep to the INBOX) with its junk marks cleared, and a rescued
//! message is never proposed as Spam again by the INBOX pass.

use super::triage::{ids_in, ACCOUNT};
use super::triage_junk::{junk_setup, JUNK};
use rustymail::triage::apply::{apply_folder, ApplyIn, ApplyOptions, Skip};
use rustymail::triage::folders::FolderMap;
use rustymail::triage::runner::RunOptions;
use rustymail::triage::Bucket;
use serial_test::serial;

fn options() -> ApplyOptions {
    ApplyOptions {
        min_confidence: 0.8,
        buckets: vec![Bucket::Spam, Bucket::Noise, Bucket::Accounting],
        limit: None,
    }
}

#[tokio::test]
#[serial]
async fn applying_the_spam_folder_rescues_reviewed_mail_and_keeps_it_out() {
    let (client, mailbox, runner) = junk_setup().await;
    runner
        .triage_junk(&client, ACCOUNT, JUNK, RunOptions::default())
        .await
        .unwrap();
    // The owner reviewed the folder: "You won" is real (the model says Spam).
    sqlx::query("UPDATE triage_proposals SET correction = 'Keep' WHERE subject = 'You won'")
        .execute(runner.store.pool())
        .await
        .unwrap();

    let role = ApplyIn::SpamFolder { inbox: "INBOX" };
    let summary = apply_folder(
        &runner.store,
        &client,
        ACCOUNT,
        JUNK,
        role,
        &FolderMap::default(),
        &options(),
    )
    .await
    .unwrap();

    assert_eq!(
        summary.moved, 2,
        "{:?} {:?}",
        summary.failures, summary.skipped
    );
    assert_eq!(summary.skipped.get(&Skip::StaysInSpam), Some(&2));
    assert_eq!(
        ids_in(&mailbox, "INBOX"),
        ["<scam@x.test>"],
        "Keep: the INBOX"
    );
    assert_eq!(ids_in(&mailbox, "Noise"), ["<news@x.test>"]);
    let mut stayed = ids_in(&mailbox, JUNK);
    stayed.sort();
    assert_eq!(stayed, ["<filed@x.test>", "<invoice@x.test>"]);
    let commands = mailbox.lock().unwrap().commands.join("\n").to_uppercase();
    assert!(commands.contains("-FLAGS ($JUNK JUNK)"), "{commands}");
    assert!(commands.contains("+FLAGS ($NOTJUNK NONJUNK)"), "{commands}");

    // Today's model still calls it Spam; the INBOX pass must not send it back.
    runner
        .triage_folder(&client, ACCOUNT, RunOptions::default())
        .await
        .unwrap();
    let pending = runner
        .store
        .pending_proposals(Some(ACCOUNT), 50)
        .await
        .unwrap();
    let again = pending
        .iter()
        .find(|p| p.folder == "INBOX" && p.subject == "You won")
        .expect("the INBOX pass saw the rescued message");
    // The owner's Keep makes the sender kept (#158), which already overrules
    // the Spam verdict; the rescue pin (tests/unit/triage_pinned_tests.rs)
    // covers rescues without such a correction.
    assert_eq!(again.bucket, "Keep", "{}", again.reason);
    assert!(again.reason.contains("model said Spam"), "{}", again.reason);
}

#[tokio::test]
#[serial]
async fn a_rescued_subject_loses_the_spam_tag_and_keeps_flags_and_date() {
    let (client, mailbox, runner) = junk_setup().await;
    {
        let mut state = mailbox.lock().unwrap();
        state.raw_bodies.insert(
            "<news@x.test>".into(),
            super::triage::raw(
                "news@shop.test",
                "***SPAM***  Deals",
                "List-Unsubscribe: <mailto:u@shop.test>\r\n",
                "<news@x.test>",
                "Sale",
            ),
        );
        state
            .internal_dates
            .insert("<news@x.test>".into(), "02-Dec-2025 20:21:47 +0000".into());
        state.answered.insert("<news@x.test>".into());
    }
    runner
        .triage_junk(&client, ACCOUNT, JUNK, RunOptions::default())
        .await
        .unwrap();

    let role = ApplyIn::SpamFolder { inbox: "INBOX" };
    let summary = apply_folder(
        &runner.store,
        &client,
        ACCOUNT,
        JUNK,
        role,
        &FolderMap::default(),
        &options(),
    )
    .await
    .unwrap();

    assert_eq!(summary.moved, 1, "{:?}", summary.failures);
    assert_eq!(summary.untagged, 1);
    let state = mailbox.lock().unwrap();
    let copy = &state.appended[0];
    assert_eq!(copy.folder, "Noise");
    assert_eq!(
        copy.flags.as_deref(),
        Some("(\\Answered)"),
        "flags carried over"
    );
    assert_eq!(
        copy.internal_date.as_deref(),
        Some("\"02-Dec-2025 20:21:47 +0000\"")
    );
    let body = &state.raw_bodies["<news@x.test>"];
    assert!(body.contains("\r\nSubject: Deals\r\n"), "{body}");
    assert!(!body.contains("SPAM"), "{body}");
    assert!(
        state.folders[JUNK]
            .iter()
            .all(|m| m.message_id != "<news@x.test>"),
        "the original left the spam folder"
    );
    assert_eq!(
        state.folders["Noise"]
            .iter()
            .filter(|m| m.message_id == "<news@x.test>")
            .count(),
        1,
        "one copy in Noise"
    );
}
