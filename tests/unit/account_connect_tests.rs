// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! load_active_accounts: active-only, optional single-account filter.

use rustymail::imap::account_connect::load_active_accounts;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn pool_with_accounts() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    for (email, active) in [
        ("a@example.test", true),
        ("b@example.test", true),
        ("off@example.test", false),
    ] {
        sqlx::query(
            "INSERT INTO accounts (email_address, display_name, imap_host, imap_port, imap_user, imap_pass, is_active)
             VALUES (?1, ?1, 'imap.example.test', 993, ?1, 'plain-password', ?2)",
        )
        .bind(email)
        .bind(active)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

fn emails(accounts: &[rustymail::imap::account_connect::ImapAccount]) -> Vec<&str> {
    let mut v: Vec<&str> = accounts.iter().map(|a| a.email_address.as_str()).collect();
    v.sort();
    v
}

#[tokio::test]
async fn loads_only_active_accounts_without_filter() {
    let pool = pool_with_accounts().await;
    let accounts = load_active_accounts(&pool, None).await.unwrap();
    assert_eq!(emails(&accounts), vec!["a@example.test", "b@example.test"]);
    assert_eq!(accounts[0].imap_pass, "plain-password");
}

#[tokio::test]
async fn filter_returns_only_the_named_active_account() {
    let pool = pool_with_accounts().await;
    let accounts = load_active_accounts(&pool, Some("b@example.test"))
        .await
        .unwrap();
    assert_eq!(emails(&accounts), vec!["b@example.test"]);
    let inactive = load_active_accounts(&pool, Some("off@example.test"))
        .await
        .unwrap();
    assert!(inactive.is_empty());
}
