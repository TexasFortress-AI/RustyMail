// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Drop cached folders that no longer exist on the mail server.
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
