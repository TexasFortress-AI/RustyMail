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
