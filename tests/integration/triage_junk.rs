// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The spam-folder review: mail the server misfiled is proposed for rescue,
//! a trusted-looking document from a new sender stays in spam (the phishing
//! the owner marked was all of this kind), and mail triage itself filed as
//! Spam stays unless the owner corrects it, even when today's verdict differs.

use super::fake_imap_server::{spawn_fake_imap, FakeMessage, SharedMailbox};
use super::triage::{raw, store, ACCOUNT};
use rustymail::imap::client::ImapClient;
use rustymail::imap::session::AsyncImapSessionWrapper;
use rustymail::triage::guards::ModelGuards;
use rustymail::triage::model::{EscalatingClassifier, ModelBackend};
use rustymail::triage::rules::SenderRules;
use rustymail::triage::runner::{RunOptions, TriageRunner};
use serial_test::serial;
use std::collections::HashMap;

pub(super) const JUNK: &str = "Junk";

/// Calls a vendor's "invoice" Accounting and everything else Spam.
pub(super) struct VendorModel;

impl ModelBackend for VendorModel {
    async fn complete(&self, _model: &str, _system: &str, user: &str) -> Result<String, String> {
        let bucket = if user.contains("billing@new-vendor.test") {
            "Accounting"
        } else if user.contains("billing@lender.test") {
            "Noise"
        } else {
            "Spam"
        };
        Ok(format!(
            r#"{{"bucket": "{bucket}", "confidence": 0.95, "reason": "stub"}}"#
        ))
    }
}

fn runner(
    store: rustymail::triage::store::TriageStore,
) -> TriageRunner<EscalatingClassifier<VendorModel>> {
    TriageRunner {
        store,
        classifier: EscalatingClassifier::new(VendorModel, "small-local".into(), None, 0.7),
        guards: ModelGuards {
            needs_reply_max_age: chrono::Duration::MAX,
        },
        senders: SenderRules::default(),
        lists: rustymail::triage::lists::ListRules::new(vec!["github.com".to_string()]),
        code_window: chrono::Duration::hours(24),
        folder: "INBOX".into(),
        fetch_batch: 10,
        excerpt_chars: 500,
        fuller_excerpt_chars: 500,
        owner_accounts: Vec::new(),
        platform_domains: Vec::new(),
        junk_review: None,
    }
}

/// A spam folder holding a newsletter, a new vendor's invoice, a message
/// triage filed as Spam earlier, and a scam; nothing proposed yet.
pub(super) async fn junk_setup() -> (
    ImapClient<AsyncImapSessionWrapper>,
    SharedMailbox,
    TriageRunner<EscalatingClassifier<VendorModel>>,
) {
    std::env::set_var("RUSTYMAIL_ALLOW_INVALID_MAIL_CERTS", "true");
    let (port, mailbox) = spawn_fake_imap(Vec::new()).await;
    let junk: Vec<(u32, &str, String)> = vec![
        (
            1,
            "<news@x.test>",
            raw(
                "news@shop.test",
                "Deals",
                "List-Unsubscribe: <mailto:u@shop.test>\r\n",
                "<news@x.test>",
                "Sale",
            ),
        ),
        (
            2,
            "<invoice@x.test>",
            raw(
                "billing@new-vendor.test",
                "Invoice 4471",
                "",
                "<invoice@x.test>",
                "Pay now",
            ),
        ),
        (
            3,
            "<filed@x.test>",
            raw(
                "billing@lender.test",
                "Statement",
                "",
                "<filed@x.test>",
                "Due",
            ),
        ),
        (
            4,
            "<scam@x.test>",
            raw(
                "prize@lottery.test",
                "You won",
                "",
                "<scam@x.test>",
                "Claim",
            ),
        ),
    ];
    {
        let mut state = mailbox.lock().unwrap();
        state.folders.insert(
            JUNK.to_string(),
            junk.iter()
                .map(|(uid, id, _)| FakeMessage {
                    uid: *uid,
                    message_id: id.to_string(),
                })
                .collect(),
        );
        for (_, id, body) in &junk {
            state.raw_bodies.insert(id.to_string(), body.clone());
        }
    }
    let store = store().await;
    // Triage itself moved <filed@x.test> to Spam earlier, by the owner's review.
    sqlx::query(
        "INSERT INTO triage_proposals (account_id, folder, uidvalidity, uid, message_id, from_address,
           subject, bucket, source, reason, applied_at, applied_folder)
         VALUES (?, 'INBOX', 1, 90, '<filed@x.test>', 'promo@spammer.test', 'Win', 'Spam', 'model',
           'stub', CURRENT_TIMESTAMP, 'Junk')",
    )
    .bind(ACCOUNT)
    .execute(store.pool())
    .await
    .unwrap();
    let client =
        ImapClient::<AsyncImapSessionWrapper>::connect("127.0.0.1", port, "user", "password")
            .await
            .unwrap();

    (client, mailbox, runner(store))
}

#[tokio::test]
#[serial]
async fn the_spam_folder_review_rescues_misfiled_mail_and_keeps_the_rest() {
    let (client, mailbox, runner) = junk_setup().await;
    let summary = runner
        .triage_junk(&client, ACCOUNT, JUNK, RunOptions::default())
        .await
        .unwrap();
    assert_eq!(summary.filed_by_triage, 1, "{summary:?}");

    let proposals = runner
        .store
        .pending_proposals(Some(ACCOUNT), 50)
        .await
        .unwrap();
    let by_subject: HashMap<String, (String, String, String)> = proposals
        .iter()
        .map(|p| {
            (
                p.subject.clone(),
                (p.folder.clone(), p.bucket.clone(), p.reason.clone()),
            )
        })
        .collect();
    assert_eq!(by_subject.len(), 4, "{by_subject:?}");
    let (folder, bucket, _) = &by_subject["Deals"];
    assert_eq!(
        (folder.as_str(), bucket.as_str()),
        (JUNK, "Noise"),
        "newsletter: rescue to Noise"
    );
    let (_, bucket, reason) = &by_subject["Invoice 4471"];
    assert_eq!(bucket, "Spam", "a new vendor's invoice stays in spam");
    assert!(
        reason.starts_with("stays in spam, new sender: verify"),
        "{reason}"
    );
    assert_eq!(by_subject["You won"].1, "Spam");
    // Triage filed this one earlier. Today's model calls it Noise, but only the
    // owner's correction may rescue it; the reason says what the model says now.
    let (_, bucket, reason) = &by_subject["Statement"];
    assert_eq!(bucket, "Spam");
    assert!(
        reason.contains("filed by triage earlier; model now says Noise"),
        "{reason}"
    );
    // Proposing never changes the mailbox.
    let state = mailbox.lock().unwrap();
    assert_eq!(state.folders[JUNK].len(), 4);
}
