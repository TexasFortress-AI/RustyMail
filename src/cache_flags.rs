// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The cached flags of a folder's messages, and the HIGHESTMODSEQ they were
//! refreshed at (see `imap::flag_changes`).
//!
//! The message sync fetches bodies without FLAGS, so its upserts never let an
//! empty flag list overwrite stored flags; this writes what the server reported.
//! Flags are stored as the JSON list RustyMail queries everywhere, e.g.
//! `["Seen","Answered"]`.

use crate::imap::flag_changes::FlagChanges;
use sqlx::{Row, SqlitePool};

pub struct FlagCache<'a> {
    pool: &'a SqlitePool,
}

impl<'a> FlagCache<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// The HIGHESTMODSEQ of the last refresh, if there was one with CONDSTORE.
    pub async fn highest_modseq(
        &self,
        account: &str,
        folder: &str,
    ) -> Result<Option<u64>, sqlx::Error> {
        let row =
            sqlx::query("SELECT highest_modseq FROM folders WHERE account_id = ? AND name = ?")
                .bind(account)
                .bind(folder)
                .fetch_optional(self.pool)
                .await?;
        Ok(row
            .and_then(|r| r.get::<Option<i64>, _>("highest_modseq"))
            .map(|modseq| modseq as u64))
    }

    /// Writes the reported flags (only rows whose flags differ) and then the
    /// new HIGHESTMODSEQ, in one transaction. Returns how many rows changed;
    /// a folder not in the cache is skipped.
    pub async fn save(
        &self,
        account: &str,
        folder: &str,
        changes: &FlagChanges,
    ) -> Result<usize, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let folder_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM folders WHERE account_id = ? AND name = ?")
                .bind(account)
                .bind(folder)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(folder_id) = folder_id else {
            return Ok(0);
        };
        let mut changed = 0;
        for (uid, flags) in &changes.flags {
            let json = serde_json::to_string(flags).unwrap_or_else(|_| "[]".to_string());
            changed += sqlx::query(
                "UPDATE emails SET flags = ?, updated_at = CURRENT_TIMESTAMP
                 WHERE folder_id = ? AND uid = ? AND flags IS NOT ?",
            )
            .bind(&json)
            .bind(folder_id)
            .bind(*uid as i64)
            .bind(&json)
            .execute(&mut *tx)
            .await?
            .rows_affected() as usize;
        }
        sqlx::query("UPDATE folders SET highest_modseq = ? WHERE id = ?")
            .bind(changes.highest_modseq.map(|modseq| modseq as i64))
            .bind(folder_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(changed)
    }
}
