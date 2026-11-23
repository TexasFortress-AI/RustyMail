// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Refreshed flags land in the cache in the form RustyMail queries
//! (`flags LIKE '%"Seen"%'`), with the HIGHESTMODSEQ they were read at.

use rustymail::cache_flags::FlagCache;
use rustymail::imap::flag_changes::FlagChanges;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

const ACCOUNT: &str = "me@example.test";

async fn cache_with_two_messages() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO accounts (email_address, display_name, imap_host, imap_user, imap_pass)
         VALUES (?, ?, 'imap.example.test', ?, 'x')",
    )
    .bind(ACCOUNT)
    .bind(ACCOUNT)
    .bind(ACCOUNT)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO folders (id, account_id, name) VALUES (1, ?, 'INBOX')")
        .bind(ACCOUNT)
        .execute(&pool)
        .await
        .unwrap();
    for uid in [1, 2] {
        sqlx::query("INSERT INTO emails (folder_id, uid, flags) VALUES (1, ?, '[]')")
            .bind(uid)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool
}

async fn unread(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT uid FROM emails WHERE flags NOT LIKE '%\"Seen\"%' ORDER BY uid")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn saved_flags_answer_the_unread_query_and_keep_the_modseq() {
    let pool = cache_with_two_messages().await;
    let cache = FlagCache::new(&pool);
    assert_eq!(cache.highest_modseq(ACCOUNT, "INBOX").await.unwrap(), None);

    let changes = FlagChanges {
        highest_modseq: Some(42),
        flags: vec![(1, vec!["Seen".into(), "Answered".into()]), (2, vec![])],
    };
    assert_eq!(
        cache.save(ACCOUNT, "INBOX", &changes).await.unwrap(),
        1,
        "uid 2 unchanged"
    );
    assert_eq!(unread(&pool).await, vec![2]);
    assert_eq!(
        cache.highest_modseq(ACCOUNT, "INBOX").await.unwrap(),
        Some(42)
    );

    // Saving the same flags again changes no rows.
    assert_eq!(cache.save(ACCOUNT, "INBOX", &changes).await.unwrap(), 0);
    // A server without CONDSTORE stores no mod-sequence.
    let full = FlagChanges {
        highest_modseq: None,
        flags: vec![(2, vec!["Seen".into()])],
    };
    assert_eq!(cache.save(ACCOUNT, "INBOX", &full).await.unwrap(), 1);
    assert_eq!(unread(&pool).await, Vec::<i64>::new());
    assert_eq!(cache.highest_modseq(ACCOUNT, "INBOX").await.unwrap(), None);
}

#[tokio::test]
async fn a_folder_not_in_the_cache_is_skipped() {
    let pool = cache_with_two_messages().await;
    let changes = FlagChanges {
        highest_modseq: Some(1),
        flags: vec![(1, vec!["Seen".into()])],
    };
    assert_eq!(
        FlagCache::new(&pool)
            .save(ACCOUNT, "Elsewhere", &changes)
            .await
            .unwrap(),
        0
    );
    assert_eq!(unread(&pool).await, vec![1, 2]);
}
