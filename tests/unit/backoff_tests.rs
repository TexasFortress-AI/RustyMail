// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustymail::backoff::{BackoffPolicy, FailureBackoff};
use rustymail::dashboard::services::sync_backoff::SyncBackoffStore;
use sqlx::sqlite::SqlitePoolOptions;
use std::time::{Duration, Instant};

const ACCOUNT: &str = "user@example.test";
const NOW: i64 = 1_000_000;

async fn store() -> SyncBackoffStore {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    SyncBackoffStore::new(pool, BackoffPolicy::new(300, 3600))
}

#[test]
fn delay_doubles_up_to_the_maximum() {
    let policy = BackoffPolicy::new(300, 3600);
    let delays: Vec<u64> = (1..=6).map(|n| policy.delay_seconds(n)).collect();
    assert_eq!(delays, vec![300, 600, 1200, 2400, 3600, 3600]);
    assert_eq!(policy.delay_seconds(u32::MAX), 3600, "must not overflow");
}

#[test]
fn in_memory_backoff_waits_then_resets_on_success() {
    let now = Instant::now();
    let mut backoff = FailureBackoff::new(BackoffPolicy::new(300, 3600));
    assert!(backoff.is_due(now));

    backoff.record_failure(now);
    assert!(!backoff.is_due(now + Duration::from_secs(299)));
    assert!(backoff.is_due(now + Duration::from_secs(300)));

    backoff.record_failure(now);
    assert!(!backoff.is_due(now + Duration::from_secs(599)));

    backoff.record_success();
    assert!(backoff.is_due(now));
}

#[tokio::test]
async fn failed_account_is_skipped_until_its_delay_passes() {
    let store = store().await;
    assert!(store.is_due(ACCOUNT, NOW).await.unwrap());

    assert_eq!(store.record_failure(ACCOUNT, NOW).await.unwrap(), 300);
    assert!(!store.is_due(ACCOUNT, NOW + 299).await.unwrap());
    assert!(store.is_due(ACCOUNT, NOW + 300).await.unwrap());
    assert!(store.is_due("other@example.test", NOW).await.unwrap());
}

#[tokio::test]
async fn consecutive_failures_lengthen_the_wait_until_success_clears_it() {
    let store = store().await;
    store.record_failure(ACCOUNT, NOW).await.unwrap();
    assert_eq!(store.record_failure(ACCOUNT, NOW).await.unwrap(), 600);

    store.record_success(ACCOUNT).await.unwrap();
    assert!(store.is_due(ACCOUNT, NOW).await.unwrap());
    assert_eq!(store.record_failure(ACCOUNT, NOW).await.unwrap(), 300);
}
