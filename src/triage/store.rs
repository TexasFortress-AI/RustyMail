// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Persistence for triage watermarks and proposals (migration 017).

use super::Proposal;
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

/// A proposal as stored, for reporting.
#[derive(Debug, Clone)]
pub struct StoredProposal {
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
    pub from_address: String,
    pub subject: String,
    pub bucket: String,
    pub confidence: Option<f64>,
    pub source: String,
    pub model_id: Option<String>,
    pub reason: String,
    pub created_at: String,
}

pub struct TriageStore {
    pool: SqlitePool,
}

impl TriageStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
                  bucket, confidence, source, model_id, reason)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        let rows = sqlx::query(
            "SELECT account_id, folder, uid, from_address, subject, bucket, confidence, source,
                    model_id, reason, created_at
             FROM triage_proposals
             WHERE applied_at IS NULL AND (?1 IS NULL OR account_id = ?1)
             ORDER BY created_at DESC, uid DESC
             LIMIT ?2",
        )
        .bind(account_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| StoredProposal {
                account_id: r.get("account_id"),
                folder: r.get("folder"),
                uid: r.get::<i64, _>("uid") as u32,
                from_address: r.get("from_address"),
                subject: r.get("subject"),
                bucket: r.get("bucket"),
                confidence: r.get("confidence"),
                source: r.get("source"),
                model_id: r.get("model_id"),
                reason: r.get("reason"),
                created_at: r.get::<String, _>("created_at"),
            })
            .collect())
    }
}
