// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Folders and messages that are gone from the server are dropped from the cache.

use rustymail::dashboard::services::cache_prune::{
    prune_missing_folders, prune_missing_messages, remove_cached_message,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn pool_with_folders() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    for (account, folder) in [
        ("a@example.test", "INBOX"),
        ("a@example.test", "INBOX.Old"),
        ("b@example.test", "INBOX.Old"),
    ] {
        sqlx::query(
            "INSERT INTO accounts (email_address, display_name, imap_host, imap_user, imap_pass)
             VALUES (?1, ?1, 'imap.example.test', ?1, 'pw') ON CONFLICT DO NOTHING",
        )
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
        let folder_id: i64 =
            sqlx::query_scalar("INSERT INTO folders (account_id, name) VALUES (?, ?) RETURNING id")
                .bind(account)
                .bind(folder)
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query("INSERT INTO emails (folder_id, uid, subject) VALUES (?, 1, 'cached')")
            .bind(folder_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    pool
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn removes_missing_folders_and_their_messages_only_for_that_account() {
    let pool = pool_with_folders().await;
    let removed = prune_missing_folders(&pool, "a@example.test", &["INBOX".to_string()])
        .await
        .unwrap();
    assert_eq!(removed, vec!["INBOX.Old".to_string()]);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM folders WHERE account_id = 'a@example.test'"
        )
        .await,
        1
    );
    // b@'s folder with the same name is untouched.
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM folders WHERE account_id = 'b@example.test'"
        )
        .await,
        1
    );
    // The removed folder's cached message went with it.
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM emails").await, 2);
}

#[tokio::test]
async fn empty_server_listing_removes_nothing() {
    let pool = pool_with_folders().await;
    let removed = prune_missing_folders(&pool, "a@example.test", &[])
        .await
        .unwrap();
    assert!(removed.is_empty());
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM folders").await, 3);
}

/// a@'s INBOX caches UIDs 1, 2 and 3; the other folders cache UID 1.
async fn pool_with_inbox() -> SqlitePool {
    let pool = pool_with_folders().await;
    for uid in [2, 3] {
        sqlx::query(
            "INSERT INTO emails (folder_id, uid, subject) SELECT id, ?, 'cached' FROM folders
             WHERE account_id = 'a@example.test' AND name = 'INBOX'",
        )
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

async fn inbox_uids(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT e.uid FROM emails e JOIN folders f ON f.id = e.folder_id
         WHERE f.account_id = 'a@example.test' AND f.name = 'INBOX' ORDER BY e.uid",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn removes_messages_no_longer_in_the_folder() {
    let pool = pool_with_inbox().await;
    // UID 2 was moved away; UID 9 is new and not cached yet.
    let removed = prune_missing_messages(&pool, "a@example.test", "INBOX", &[1, 3, 9], 3)
        .await
        .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(inbox_uids(&pool).await, vec![1, 3]);
    // Other folders and accounts keep their one cached message each.
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM emails").await, 4);
}

#[tokio::test]
async fn a_search_that_disagrees_with_exists_removes_nothing() {
    let pool = pool_with_inbox().await;
    let removed = prune_missing_messages(&pool, "a@example.test", "INBOX", &[1], 3)
        .await
        .unwrap();
    assert_eq!(removed, 0);
    assert_eq!(inbox_uids(&pool).await, vec![1, 2, 3]);
}

#[tokio::test]
async fn an_emptied_folder_empties_its_cache() {
    let pool = pool_with_inbox().await;
    let removed = prune_missing_messages(&pool, "a@example.test", "INBOX", &[], 0)
        .await
        .unwrap();
    assert_eq!(removed, 3);
    assert!(inbox_uids(&pool).await.is_empty());
}

#[tokio::test]
async fn remove_cached_message_removes_only_that_message() {
    let pool = pool_with_inbox().await;
    let removed = remove_cached_message(&pool, "a@example.test", "INBOX", 2)
        .await
        .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(inbox_uids(&pool).await, vec![1, 3]);
    // Same UID in another folder or account stays.
    let other = remove_cached_message(&pool, "a@example.test", "INBOX", 2)
        .await
        .unwrap();
    assert_eq!(other, 0);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM emails").await, 4);
}
