// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Subject-line filtering: queries the SQLite email cache for emails matching
//! one or more subject patterns, returning metadata only (no body content).
//! Designed for fast triage of large folders without loading bodies into context.

use chrono::{DateTime, Utc};
use log::info;
use serde::Serialize;
use sqlx::SqlitePool;

/// Default maximum results if not specified by the caller.
const DEFAULT_MAX_RESULTS: usize = 500;

/// A single email result from subject filtering.
#[derive(Debug, Serialize)]
pub struct FilteredEmail {
    pub uid: i64,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub to_addresses: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub has_attachments: bool,
    pub matched_patterns: Vec<String>,
}

/// Result of a subject filter operation.
#[derive(Debug, Serialize)]
pub struct SubjectFilterResult {
    pub account: String,
    pub folder: String,
    pub patterns_used: Vec<String>,
    pub match_mode: String,
    pub total_matched: usize,
    pub results: Vec<FilteredEmail>,
}

/// Filters emails by subject line patterns against the SQLite cache.
pub struct SubjectFilter {
    db_pool: SqlitePool,
}

impl SubjectFilter {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self { db_pool }
    }

    /// Filter emails by subject patterns.
    ///
    /// - `account_id`: email address (required)
    /// - `folder`: folder name (required)
    /// - `patterns`: subject substring patterns (required, at least one)
    /// - `match_mode`: "any" (default) or "all"
    /// - `sender_filter`: optional sender address/domain substring
    /// - `recipient_filter`: optional recipient address/domain substring
    /// - `date_after`: optional lower bound (inclusive)
    /// - `date_before`: optional upper bound (inclusive)
    /// - `max_results`: optional cap (default: 500)
    #[allow(clippy::too_many_arguments)]
    pub async fn filter(
        &self,
        account_id: &str,
        folder: &str,
        patterns: &[String],
        match_mode: Option<&str>,
        sender_filter: Option<&str>,
        recipient_filter: Option<&str>,
        date_after: Option<&str>,
        date_before: Option<&str>,
        max_results: Option<usize>,
    ) -> Result<SubjectFilterResult, Box<dyn std::error::Error>> {
        if patterns.is_empty() {
            return Err("At least one subject pattern is required".into());
        }

        let mode = match_mode.unwrap_or("any");
        let limit = max_results.unwrap_or(DEFAULT_MAX_RESULTS);

        // 1. Resolve folder_id
        let folder_id = self.resolve_folder_id(account_id, folder).await?;

        // 2. Build and execute query
        let rows = self
            .query_filtered(folder_id, patterns, mode, sender_filter, recipient_filter, date_after, date_before, limit)
            .await?;

        // 3. Compute matched_patterns for each result
        let results = compute_matched_patterns(rows, patterns);

        let total = results.len();
        info!(
            "Subject filter: {} matches from {}/{} (patterns={:?}, mode={})",
            total, account_id, folder, patterns, mode
        );

        Ok(SubjectFilterResult {
            account: account_id.to_string(),
            folder: folder.to_string(),
            patterns_used: patterns.to_vec(),
            match_mode: mode.to_string(),
            total_matched: total,
            results,
        })
    }

    /// Look up the folder_id from the folders table.
    async fn resolve_folder_id(
        &self,
        account_id: &str,
        folder_name: &str,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM folders WHERE account_id = ? AND name = ?"
        )
        .bind(account_id)
        .bind(folder_name)
        .fetch_optional(&self.db_pool)
        .await?;

        match row {
            Some((id,)) => Ok(id),
            None => Err(format!(
                "Folder '{}' not found for account '{}'", folder_name, account_id
            ).into()),
        }
    }

    /// Build a dynamic SQL query with pattern conditions and optional filters.
    #[allow(clippy::too_many_arguments)]
    async fn query_filtered(
        &self,
        folder_id: i64,
        patterns: &[String],
        match_mode: &str,
        sender_filter: Option<&str>,
        recipient_filter: Option<&str>,
        date_after: Option<&str>,
        date_before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RawFilterRow>, Box<dyn std::error::Error>> {
        // We build the query dynamically because the number of pattern
        // conditions varies. sqlx doesn't support binding a dynamic number
        // of parameters, so we use query_as with a fully-formed SQL string.
        // The LIKE values are escaped below to prevent SQL injection.

        let mut sql = String::from(
            "SELECT e.uid, e.subject, e.from_address, e.to_addresses, \
             e.date, e.has_attachments \
             FROM emails e \
             WHERE e.folder_id = ?"
        );

        // Subject pattern conditions
        let joiner = if match_mode == "all" { " AND " } else { " OR " };
        let pattern_clauses: Vec<String> = patterns.iter()
            .map(|_| "e.subject LIKE ? COLLATE NOCASE".to_string())
            .collect();
        sql.push_str(&format!(" AND ({})", pattern_clauses.join(joiner)));

        // Optional filters
        if sender_filter.is_some() {
            sql.push_str(" AND e.from_address LIKE ? COLLATE NOCASE");
        }
        if recipient_filter.is_some() {
            sql.push_str(" AND e.to_addresses LIKE ? COLLATE NOCASE");
        }
        if date_after.is_some() {
            sql.push_str(" AND e.date >= ?");
        }
        if date_before.is_some() {
            sql.push_str(" AND e.date <= ?");
        }

        sql.push_str(" ORDER BY e.date DESC");
        sql.push_str(&format!(" LIMIT {}", limit));

        // Bind parameters in order
        let mut query = sqlx::query_as::<_, RawFilterRow>(&sql)
            .bind(folder_id);

        for pattern in patterns {
            query = query.bind(format!("%{}%", pattern));
        }
        if let Some(sender) = sender_filter {
            query = query.bind(format!("%{}%", sender));
        }
        if let Some(recipient) = recipient_filter {
            query = query.bind(format!("%{}%", recipient));
        }
        if let Some(after) = date_after {
            query = query.bind(after.to_string());
        }
        if let Some(before) = date_before {
            query = query.bind(before.to_string());
        }

        let rows = query.fetch_all(&self.db_pool).await?;
        Ok(rows)
    }
}

/// Internal row type from the SQL query.
#[derive(Debug, sqlx::FromRow)]
struct RawFilterRow {
    uid: i64,
    subject: Option<String>,
    from_address: Option<String>,
    to_addresses: Option<String>,
    date: Option<DateTime<Utc>>,
    has_attachments: bool,
}

/// For each row, determine which of the input patterns matched its subject.
/// This is a pure function that can be tested without a database.
fn compute_matched_patterns(
    rows: Vec<RawFilterRow>,
    patterns: &[String],
) -> Vec<FilteredEmail> {
    rows.into_iter()
        .map(|row| {
            let subject_lower = row.subject.as_deref().unwrap_or("").to_lowercase();
            let matched: Vec<String> = patterns.iter()
                .filter(|p| subject_lower.contains(&p.to_lowercase()))
                .cloned()
                .collect();
            FilteredEmail {
                uid: row.uid,
                subject: row.subject,
                from_address: row.from_address,
                to_addresses: row.to_addresses,
                date: row.date,
                has_attachments: row.has_attachments,
                matched_patterns: matched,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(uid: i64, subject: &str) -> RawFilterRow {
        RawFilterRow {
            uid,
            subject: Some(subject.to_string()),
            from_address: Some("alice@example.com".to_string()),
            to_addresses: Some("bob@example.com".to_string()),
            date: None,
            has_attachments: false,
        }
    }

    #[test]
    fn test_matched_patterns_any() {
        let rows = vec![
            make_row(1, "FW: Resume for John Doe"),
            make_row(2, "Meeting notes from today"),
            make_row(3, "Candidate submittal - Jane"),
        ];
        let patterns = vec!["resume".to_string(), "candidate".to_string()];
        let results = compute_matched_patterns(rows, &patterns);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].matched_patterns, vec!["resume"]);
        assert!(results[1].matched_patterns.is_empty());
        assert_eq!(results[2].matched_patterns, vec!["candidate"]);
    }

    #[test]
    fn test_matched_patterns_case_insensitive() {
        let rows = vec![make_row(1, "RESUME ATTACHED")];
        let patterns = vec!["resume".to_string()];
        let results = compute_matched_patterns(rows, &patterns);

        assert_eq!(results[0].matched_patterns, vec!["resume"]);
    }

    #[test]
    fn test_matched_patterns_multiple_match() {
        let rows = vec![make_row(1, "Candidate Resume Submittal")];
        let patterns = vec![
            "candidate".to_string(),
            "resume".to_string(),
            "invoice".to_string(),
        ];
        let results = compute_matched_patterns(rows, &patterns);

        assert_eq!(results[0].matched_patterns.len(), 2);
        assert!(results[0].matched_patterns.contains(&"candidate".to_string()));
        assert!(results[0].matched_patterns.contains(&"resume".to_string()));
    }

    #[test]
    fn test_matched_patterns_null_subject() {
        let row = RawFilterRow {
            uid: 1,
            subject: None,
            from_address: None,
            to_addresses: None,
            date: None,
            has_attachments: false,
        };
        let patterns = vec!["test".to_string()];
        let results = compute_matched_patterns(vec![row], &patterns);

        assert!(results[0].matched_patterns.is_empty());
    }

    #[test]
    fn test_matched_patterns_proper_nouns_and_acronyms() {
        // Acronyms should match
        let rows = vec![
            make_row(1, "FW: NCHCR Candidate DR Saima Yasir inquiring about your Ultrasound Tech"),
            make_row(2, "MLee Healthcare - Candidate Submittal - Yazen Amra"),
        ];
        let patterns = vec!["NCHCR".to_string()];
        let results = compute_matched_patterns(rows, &patterns);
        assert_eq!(results[0].matched_patterns, vec!["NCHCR"]);
        assert!(results[1].matched_patterns.is_empty());

        // Proper names should match
        let rows = vec![make_row(3, "FW: Kentrail Conyers for Laboratory Leadership")];
        let patterns = vec!["Kentrail".to_string()];
        let results = compute_matched_patterns(rows, &patterns);
        assert_eq!(results[0].matched_patterns, vec!["Kentrail"]);

        // Both a generic term and a proper noun on the same email
        let rows = vec![
            make_row(1, "FW: NCHCR Candidate DR Saima Yasir"),
        ];
        let patterns = vec!["candidate".to_string(), "NCHCR".to_string()];
        let results = compute_matched_patterns(rows, &patterns);
        assert_eq!(results[0].matched_patterns.len(), 2);
        assert!(results[0].matched_patterns.contains(&"candidate".to_string()));
        assert!(results[0].matched_patterns.contains(&"NCHCR".to_string()));
    }
}
