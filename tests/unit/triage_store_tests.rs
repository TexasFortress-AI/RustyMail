// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Bucket parsing and TriageStore watermark/proposal persistence.

use rustymail::triage::store::{MessageRef, TriageStore, Watermark};
use rustymail::triage::{Bucket, Proposal, ProposalSource};
use sqlx::sqlite::SqlitePoolOptions;

async fn store() -> TriageStore {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    TriageStore::new(pool)
}

fn message(account_id: &str, uid: u32) -> MessageRef<'_> {
    MessageRef {
        account_id,
        folder: "INBOX",
        uidvalidity: 7,
        uid,
        message_id: Some("<m@example.test>"),
        from_address: "news@example.test",
        subject: "Weekly digest",
        counterparty: None,
    }
}

#[test]
fn bucket_parse_accepts_model_spellings() {
    assert_eq!(Bucket::parse("Needs Reply"), Some(Bucket::NeedsReply));
    assert_eq!(Bucket::parse("needs_reply"), Some(Bucket::NeedsReply));
    assert_eq!(Bucket::parse("NOISE"), Some(Bucket::Noise));
    assert_eq!(Bucket::parse("Newsletter"), None);
    for bucket in Bucket::ALL {
        assert_eq!(Bucket::parse(bucket.as_str()), Some(bucket));
    }
}

#[tokio::test]
async fn watermark_round_trips_and_updates() {
    let store = store().await;
    assert_eq!(
        store.watermark("a@example.test", "INBOX").await.unwrap(),
        None
    );
    let first = Watermark {
        uidvalidity: 7,
        last_uid: 100,
    };
    store
        .set_watermark("a@example.test", "INBOX", first)
        .await
        .unwrap();
    assert_eq!(
        store.watermark("a@example.test", "INBOX").await.unwrap(),
        Some(first)
    );
    let second = Watermark {
        uidvalidity: 8,
        last_uid: 5,
    };
    store
        .set_watermark("a@example.test", "INBOX", second)
        .await
        .unwrap();
    assert_eq!(
        store.watermark("a@example.test", "INBOX").await.unwrap(),
        Some(second)
    );
    assert_eq!(
        store.watermark("b@example.test", "INBOX").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn proposal_is_recorded_once_and_listed_per_account() {
    let store = store().await;
    let rule = Proposal::from_rule(Bucket::Noise, "List-Unsubscribe header");
    assert!(store
        .insert_proposal(&message("a@example.test", 1), &rule)
        .await
        .unwrap());
    assert!(!store
        .insert_proposal(&message("a@example.test", 1), &rule)
        .await
        .unwrap());

    let model = Proposal {
        bucket: Bucket::NeedsReply,
        confidence: Some(0.8),
        source: ProposalSource::Model,
        model_id: Some("local-model".to_string()),
        reason: "direct question".to_string(),
        not_before: None,
        list_name: None,
        document: None,
    };
    assert!(store
        .insert_proposal(&message("b@example.test", 2), &model)
        .await
        .unwrap());

    let all = store.pending_proposals(None, 10).await.unwrap();
    assert_eq!(all.len(), 2);
    let only_b = store
        .pending_proposals(Some("b@example.test"), 10)
        .await
        .unwrap();
    assert_eq!(only_b.len(), 1);
    assert_eq!(only_b[0].bucket, "Needs Reply");
    assert_eq!(only_b[0].confidence, Some(0.8));
    assert_eq!(only_b[0].source, "model");
    assert_eq!(only_b[0].model_id.as_deref(), Some("local-model"));
}

#[tokio::test]
async fn apply_bookkeeping_and_complete_counts() {
    let store = store().await;
    let due = chrono::DateTime::parse_from_rfc3339("2026-11-14T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let mut code = Proposal::from_rule(Bucket::Trash, "one-time code");
    code.not_before = Some(due);
    for uid in 1..=3 {
        let p = if uid == 1 {
            code.clone()
        } else {
            Proposal::from_rule(Bucket::Noise, "List-Unsubscribe header")
        };
        store
            .insert_proposal(&message("a@example.test", uid), &p)
            .await
            .unwrap();
    }

    let to_apply = store
        .proposals_to_apply("a@example.test", "INBOX")
        .await
        .unwrap();
    assert_eq!(
        to_apply.iter().map(|p| p.uid).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(to_apply[0].not_before, Some(due));
    assert_eq!(to_apply[0].uidvalidity, 7);

    store
        .mark_applied(to_apply[1].id, "INBOX.Noise")
        .await
        .unwrap();
    store
        .mark_failed(to_apply[2].id, "still in INBOX")
        .await
        .unwrap();
    let left = store
        .proposals_to_apply("a@example.test", "INBOX")
        .await
        .unwrap();
    assert_eq!(left.len(), 2);
    assert_eq!(left[1].apply_error.as_deref(), Some("still in INBOX"));

    // Counts come from SQL, so a one-row report limit does not shrink them.
    assert_eq!(store.pending_proposals(None, 1).await.unwrap().len(), 1);
    let counts = store.bucket_counts(Some("a@example.test")).await.unwrap();
    let noise = counts.iter().find(|c| c.bucket == "Noise").unwrap();
    assert_eq!((noise.pending, noise.applied), (1, 1));
    let trash = counts.iter().find(|c| c.bucket == "Trash").unwrap();
    assert_eq!((trash.pending, trash.applied), (1, 0));
}

#[tokio::test]
async fn the_list_name_is_stored_with_the_proposal() {
    let store = store().await;
    let mut post = Proposal::from_rule(Bucket::Lists, "mailing list post");
    post.list_name = Some("zeromq-dev".to_string());
    store
        .insert_proposal(&message("a@example.test", 1), &post)
        .await
        .unwrap();
    let stored = store.pending_proposals(None, 10).await.unwrap();
    assert_eq!(stored[0].bucket, "Lists");
    assert_eq!(stored[0].list_name.as_deref(), Some("zeromq-dev"));
}

#[tokio::test]
async fn approving_a_shared_document_trusts_its_sharer_not_the_platform() {
    use rustymail::triage::people::KnownPeople;
    let store = store().await;
    let owner = "me@owner.test";
    let share = MessageRef {
        from_address: "drive-shares-dm-noreply@google.com",
        subject: "Document shared with you",
        counterparty: Some("xenia@transformgroup.test"),
        ..message(owner, 1)
    };
    let proposal = Proposal {
        bucket: Bucket::Alerts,
        confidence: Some(0.9),
        source: ProposalSource::Model,
        model_id: Some("m".into()),
        reason: "a colleague's document".into(),
        not_before: None,
        list_name: None,
        document: None,
    };
    assert!(store.insert_proposal(&share, &proposal).await.unwrap());
    let before = KnownPeople::load(store.pool(), &[owner.to_string()])
        .await
        .unwrap();
    assert!(
        !before.deals_with("xenia@transformgroup.test"),
        "not approved yet"
    );
    let id: i64 = sqlx::query_scalar("SELECT id FROM triage_proposals WHERE uid = 1")
        .fetch_one(store.pool())
        .await
        .unwrap();
    store.mark_applied(id, "Alerts").await.unwrap();
    let after = KnownPeople::load(store.pool(), &[owner.to_string()])
        .await
        .unwrap();
    assert!(
        after.deals_with("xenia@transformgroup.test"),
        "the sharer is trusted"
    );
    assert!(
        after.deals_with("colleague@transformgroup.test"),
        "and her organisation"
    );
    assert_eq!(
        after.approved_count("drive-shares-dm-noreply@google.com"),
        0,
        "not Google"
    );
}
