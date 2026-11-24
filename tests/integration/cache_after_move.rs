// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A message moved or deleted through the email service leaves the source
//! folder's cache at once: the dashboard must not keep showing it there
//! until the next full sync.

use super::write_tools_account_routing::{harness, ACCOUNT_B};
use rustymail::dashboard::services::cache::{CacheConfig, CacheService};
use rustymail::imap::types::{Email, Envelope};
use serial_test::serial;
use std::sync::Arc;

fn cached(uid: u32) -> Email {
    Email {
        uid,
        flags: vec![],
        internal_date: None,
        envelope: Some(Envelope {
            subject: Some(format!("message {uid}")),
            from: vec![],
            to: vec![],
            cc: vec![],
            bcc: vec![],
            reply_to: vec![],
            date: None,
            in_reply_to: None,
            message_id: Some(format!("<m{uid}@example.test>")),
        }),
        body: None,
        mime_parts: vec![],
        text_body: None,
        html_body: None,
        attachments: vec![],
    }
}

async fn cache_with(dir: &std::path::Path, uids: &[u32]) -> Arc<CacheService> {
    // The harness's database: its accounts table holds the test accounts.
    let db = dir.join("accounts.db");
    let mut cache = CacheService::new(CacheConfig {
        database_url: format!("sqlite:{}", db.display()),
        max_memory_items: 100,
        max_folder_items: 50,
        max_cache_size_mb: 100,
        max_email_age_days: 30,
        sync_interval_seconds: 300,
    });
    cache.initialize().await.unwrap();
    for &uid in uids {
        cache
            .cache_email("INBOX", &cached(uid), ACCOUNT_B)
            .await
            .unwrap();
    }
    Arc::new(cache)
}

#[tokio::test]
#[serial]
async fn moved_and_deleted_messages_leave_the_source_folder_cache() {
    let h = harness(vec![
        (1, "<m1@example.test>"),
        (2, "<m2@example.test>"),
        (3, "<m3@example.test>"),
        (4, "<m4@example.test>"),
    ])
    .await;
    h.box_b.lock().unwrap().uidplus = true;
    let cache = cache_with(h._config_dir.path(), &[1, 2, 3, 4]).await;
    let service = h.service.with_cache(cache.clone());

    service
        .atomic_move_message_for_account(1, "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();
    service
        .atomic_batch_move_for_account(&[2], "INBOX", "Archive", ACCOUNT_B)
        .await
        .unwrap();
    service
        .delete_messages_for_account("INBOX", &[3], ACCOUNT_B)
        .await
        .unwrap();

    for uid in [1, 2, 3] {
        let row = cache
            .get_cached_email("INBOX", uid, ACCOUNT_B)
            .await
            .unwrap();
        assert!(row.is_none(), "uid {uid} still cached in INBOX");
    }
    let kept = cache.get_cached_email("INBOX", 4, ACCOUNT_B).await.unwrap();
    assert!(kept.is_some(), "an untouched message stays cached");
}
