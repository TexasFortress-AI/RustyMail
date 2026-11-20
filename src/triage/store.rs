// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Persistence for triage watermarks and proposals (migration 017).

use super::Proposal;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Highest UID already triaged in a folder, valid only for `uidvalidity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watermark {
    pub uidvalidity: u32,
    pub last_uid: u32,
}

/// Identifies the message a proposal is about.
pub struct MessageRef<'a> {
    pub account_id: &'a str,
    pub folder: &'a str,
    pub uidvalidity: u32,
    pub uid: u32,
    pub message_id: Option<&'a str>,
    pub from_address: &'a str,
    pub subject: &'a str,
}

/// A proposal as stored, for reporting and applying.
#[derive(Debug, Clone)]
pub struct StoredProposal {
    pub id: i64,
    pub account_id: String,
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
    pub message_id: Option<String>,
    pub from_address: String,
    pub subject: String,
    pub bucket: String,
    pub confidence: Option<f64>,
    pub source: String,
    pub model_id: Option<String>,
    pub reason: String,
    pub not_before: Option<DateTime<Utc>>,
    /// The list or repo folder name for `Lists` and `Repos` proposals.
    pub list_name: Option<String>,
    /// The bucket the owner chose on review, when the proposal was wrong.
    pub correction: Option<String>,
    pub apply_error: Option<String>,
    pub created_at: String,
}

/// Proposals per account and bucket, pending and applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketCount {
    pub account_id: String,
    pub bucket: String,
    pub pending: u64,
    pub applied: u64,
}

const PROPOSAL_COLUMNS: &str = "id, account_id, folder, uidvalidity, uid, message_id, from_address,
     subject, bucket, confidence, source, model_id, reason, not_before, list_name, correction,
     apply_error, created_at";

pub struct TriageStore {
    pool: SqlitePool,
}

impl TriageStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn watermark(
        &self,
        account_id: &str,
        folder: &str,
    ) -> Result<Option<Watermark>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT uidvalidity, last_uid FROM triage_watermarks WHERE account_id = ? AND folder = ?",
        )
        .bind(account_id)
        .bind(folder)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Watermark {
            uidvalidity: r.get::<i64, _>("uidvalidity") as u32,
            last_uid: r.get::<i64, _>("last_uid") as u32,
        }))
    }

    pub async fn set_watermark(
        &self,
        account_id: &str,
        folder: &str,
        watermark: Watermark,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO triage_watermarks (account_id, folder, uidvalidity, last_uid) VALUES (?, ?, ?, ?)
             ON CONFLICT (account_id, folder) DO UPDATE SET
                 uidvalidity = excluded.uidvalidity,
                 last_uid = excluded.last_uid,
                 updated_at = CURRENT_TIMESTAMP",
        )
        .bind(account_id)
        .bind(folder)
        .bind(watermark.uidvalidity as i64)
        .bind(watermark.last_uid as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a proposal. Returns false if this message already has one.
    pub async fn insert_proposal(
        &self,
        message: &MessageRef<'_>,
        proposal: &Proposal,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO triage_proposals
                 (account_id, folder, uidvalidity, uid, message_id, from_address, subject,
                  bucket, confidence, source, model_id, reason, not_before, list_name)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(message.account_id)
        .bind(message.folder)
        .bind(message.uidvalidity as i64)
        .bind(message.uid as i64)
        .bind(message.message_id)
        .bind(message.from_address)
        .bind(message.subject)
        .bind(proposal.bucket.as_str())
        .bind(proposal.confidence)
        .bind(proposal.source.as_str())
        .bind(proposal.model_id.as_deref())
        .bind(&proposal.reason)
        .bind(proposal.not_before.map(|t| t.to_rfc3339()))
        .bind(proposal.list_name.as_deref())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// UIDs in this folder (and UIDVALIDITY) that already have a proposal.
    pub async fn proposed_uids(
        &self,
        account_id: &str,
        folder: &str,
        uidvalidity: u32,
    ) -> Result<HashSet<u32>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT uid FROM triage_proposals WHERE account_id = ? AND folder = ? AND uidvalidity = ?",
        )
        .bind(account_id)
        .bind(folder)
        .bind(uidvalidity as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("uid") as u32).collect())
    }

    /// Proposals not yet applied, newest first, optionally for one account.
    pub async fn pending_proposals(
        &self,
        account_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<StoredProposal>, sqlx::Error> {
        let sql = format!(
            "SELECT {} FROM triage_proposals
             WHERE applied_at IS NULL AND (?1 IS NULL OR account_id = ?1)
             ORDER BY created_at DESC, uid DESC
             LIMIT ?2",
            PROPOSAL_COLUMNS
        );
        let rows = sqlx::query(&sql)
            .bind(account_id)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(stored_proposal).collect())
    }

    /// Unapplied proposals for one account and folder, oldest UID first.
    pub async fn proposals_to_apply(
        &self,
        account_id: &str,
        folder: &str,
    ) -> Result<Vec<StoredProposal>, sqlx::Error> {
        let sql = format!(
            "SELECT {} FROM triage_proposals
             WHERE applied_at IS NULL AND account_id = ? AND folder = ?
             ORDER BY uid",
            PROPOSAL_COLUMNS
        );
        let rows = sqlx::query(&sql)
            .bind(account_id)
            .bind(folder)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(stored_proposal).collect())
    }

    /// Counted in SQL, so the totals are not limited by a report's row cap.
    pub async fn bucket_counts(
        &self,
        account_id: Option<&str>,
    ) -> Result<Vec<BucketCount>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT account_id, bucket,
                    SUM(applied_at IS NULL) AS pending,
                    SUM(applied_at IS NOT NULL) AS applied
             FROM triage_proposals
             WHERE ?1 IS NULL OR account_id = ?1
             GROUP BY account_id, bucket
             ORDER BY account_id, bucket",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| BucketCount {
                account_id: r.get("account_id"),
                bucket: r.get("bucket"),
                pending: r.get::<i64, _>("pending") as u64,
                applied: r.get::<i64, _>("applied") as u64,
            })
            .collect())
    }

    /// Record a verified move.
    pub async fn mark_applied(&self, id: i64, destination: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE triage_proposals
             SET applied_at = CURRENT_TIMESTAMP, applied_folder = ?, apply_error = NULL
             WHERE id = ?",
        )
        .bind(destination)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record why a move failed; the proposal stays pending.
    pub async fn mark_failed(&self, id: i64, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE triage_proposals SET apply_error = ? WHERE id = ?")
            .bind(error)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn stored_proposal(r: &SqliteRow) -> StoredProposal {
    StoredProposal {
        id: r.get("id"),
        account_id: r.get("account_id"),
        folder: r.get("folder"),
        uidvalidity: r.get::<i64, _>("uidvalidity") as u32,
        uid: r.get::<i64, _>("uid") as u32,
        message_id: r.get("message_id"),
        from_address: r.get("from_address"),
        subject: r.get("subject"),
        bucket: r.get("bucket"),
        confidence: r.get("confidence"),
        source: r.get("source"),
        model_id: r.get("model_id"),
        reason: r.get("reason"),
        not_before: r
            .get::<Option<String>, _>("not_before")
            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&Utc)),
        list_name: r.get("list_name"),
        correction: r.get("correction"),
        apply_error: r.get("apply_error"),
        created_at: r.get::<String, _>("created_at"),
    }
}
