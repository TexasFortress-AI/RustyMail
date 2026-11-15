// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Drop cached folders, and cached messages, that no longer exist on the
//! mail server (moved or deleted by another client, or by triage).
//!
//! Only the local cache is touched, never the mailbox. Cached messages in a
//! removed folder go with it (emails.folder_id has ON DELETE CASCADE).

use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Remove cached folders of `account_id` that are not in `server_folders`.
/// Returns the removed names. An empty server listing is treated as suspect
/// and removes nothing, so a failed or truncated LIST cannot empty the cache.
pub async fn prune_missing_folders(
    pool: &SqlitePool,
    account_id: &str,
    server_folders: &[String],
) -> Result<Vec<String>, sqlx::Error> {
    if server_folders.is_empty() {
        return Ok(Vec::new());
    }
    let on_server: HashSet<&str> = server_folders.iter().map(String::as_str).collect();
    let cached = sqlx::query("SELECT name FROM folders WHERE account_id = ?")
        .bind(account_id)
        .fetch_all(pool)
        .await?;
    let mut removed = Vec::new();
    for row in cached {
        let name: String = row.get("name");
        if on_server.contains(name.as_str()) {
            continue;
        }
        sqlx::query("DELETE FROM folders WHERE account_id = ? AND name = ?")
            .bind(account_id)
            .bind(&name)
            .execute(pool)
            .await?;
        removed.push(name);
    }
    Ok(removed)
}

/// Remove cached messages of one folder whose UIDs are not in `server_uids`
/// (the folder's full `UID SEARCH ALL`). Returns how many were removed.
///
/// `server_exists` is the folder's EXISTS count from SELECT; when the search
/// result does not match it, the listing is suspect and nothing is removed.
pub async fn prune_missing_messages(
    pool: &SqlitePool,
    account_id: &str,
    folder_name: &str,
    server_uids: &[u32],
    server_exists: u32,
) -> Result<u64, sqlx::Error> {
    if server_uids.len() as u64 != u64::from(server_exists) {
        return Ok(0);
    }
    let on_server: HashSet<i64> = server_uids.iter().map(|&uid| i64::from(uid)).collect();
    let cached = sqlx::query(
        "SELECT e.id, e.uid FROM emails e JOIN folders f ON f.id = e.folder_id
         WHERE f.account_id = ? AND f.name = ?",
    )
    .bind(account_id)
    .bind(folder_name)
    .fetch_all(pool)
    .await?;
    let mut removed = 0;
    for row in cached {
        if on_server.contains(&row.get::<i64, _>("uid")) {
            continue;
        }
        removed += sqlx::query("DELETE FROM emails WHERE id = ?")
            .bind(row.get::<i64, _>("id"))
            .execute(pool)
            .await?
            .rows_affected();
    }
    Ok(removed)
}

/// Remove one cached message, e.g. right after it was moved out of `folder_name`.
pub async fn remove_cached_message(
    pool: &SqlitePool,
    account_id: &str,
    folder_name: &str,
    uid: u32,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM emails WHERE uid = ? AND folder_id IN
             (SELECT id FROM folders WHERE account_id = ? AND name = ?)",
    )
    .bind(i64::from(uid))
    .bind(account_id)
    .bind(folder_name)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
