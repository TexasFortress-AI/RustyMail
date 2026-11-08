// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Folders deleted on the server are dropped from the cache, with their messages.

use rustymail::dashboard::services::cache_prune::prune_missing_folders;
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
