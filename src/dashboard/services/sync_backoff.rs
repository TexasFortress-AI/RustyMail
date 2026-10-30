// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Per-account sync backoff, stored in the database because the sync binary is a new
//! process on every run.

use sqlx::{Row, SqlitePool};

use crate::backoff::BackoffPolicy;

/// Tracks which accounts are waiting out a failure before their next sync.
pub struct SyncBackoffStore {
    pool: SqlitePool,
    policy: BackoffPolicy,
}

impl SyncBackoffStore {
    pub fn new(pool: SqlitePool, policy: BackoffPolicy) -> Self {
        Self { pool, policy }
    }

    pub async fn is_due(&self, account_id: &str, now_unix: i64) -> Result<bool, sqlx::Error> {
        let row = sqlx::query("SELECT next_attempt_at FROM sync_backoff WHERE account_id = ?")
            .bind(account_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_none_or(|r| now_unix >= r.get::<i64, _>("next_attempt_at")))
    }

    /// Records a failure and returns how many seconds the account will wait.
    pub async fn record_failure(
        &self,
        account_id: &str,
        now_unix: i64,
    ) -> Result<u64, sqlx::Error> {
        let row = sqlx::query("SELECT consecutive_failures FROM sync_backoff WHERE account_id = ?")
            .bind(account_id)
            .fetch_optional(&self.pool)
            .await?;
        let previous_failures = row.map_or(0, |r| r.get::<i64, _>("consecutive_failures"));
        let consecutive_failures = previous_failures.saturating_add(1);
        let delay = self.policy.delay_seconds(consecutive_failures as u32);

        sqlx::query(
            "INSERT INTO sync_backoff (account_id, consecutive_failures, next_attempt_at)
             VALUES (?, ?, ?)
             ON CONFLICT(account_id) DO UPDATE SET
                consecutive_failures = excluded.consecutive_failures,
                next_attempt_at = excluded.next_attempt_at",
        )
        .bind(account_id)
        .bind(consecutive_failures)
        .bind(now_unix.saturating_add(delay as i64))
        .execute(&self.pool)
        .await?;
        Ok(delay)
    }

    pub async fn record_success(&self, account_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sync_backoff WHERE account_id = ?")
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
