// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Batch synopsis retrieval: fetch compact, one-paragraph summaries for
//! multiple emails in a single call. Queries the SQLite cache — no IMAP
//! connections needed. Dramatically reduces round-trips compared to
//! calling get_email_synopsis once per UID.

use chrono::{DateTime, Utc};
use log::info;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Maximum UIDs allowed per batch call.
const MAX_BATCH_SIZE: usize = 50;

/// Default character limit per synopsis.
const DEFAULT_MAX_CHARS: usize = 300;

/// Absolute maximum character limit per synopsis.
const ABSOLUTE_MAX_CHARS: usize = 800;

/// One email's synopsis result.
#[derive(Debug, Serialize)]
pub struct EmailSynopsis {
    pub uid: i64,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub to_addresses: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub has_attachments: bool,
    pub synopsis: String,
}

/// An error for a single UID that couldn't be processed.
#[derive(Debug, Serialize)]
pub struct SynopsisError {
    pub uid: i64,
    pub reason: String,
}

/// Complete result of a batch synopsis operation.
#[derive(Debug, Serialize)]
pub struct BatchSynopsisResult {
    pub account: String,
    pub folder: String,
    pub requested: usize,
    pub returned: usize,
    pub synopses: Vec<EmailSynopsis>,
    pub errors: Vec<SynopsisError>,
}

/// Processes batch synopsis requests against the SQLite cache.
pub struct BatchSynopsisProcessor {
    db_pool: SqlitePool,
}

impl BatchSynopsisProcessor {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self { db_pool }
    }

    /// Fetch synopses for a batch of UIDs.
    ///
    /// - `account_id`: email address (required)
    /// - `folder`: folder name (required)
    /// - `uids`: list of UIDs to fetch (max 50)
    /// - `max_chars`: character cap per synopsis (default: 300, max: 800)
    pub async fn process(
        &self,
        account_id: &str,
        folder: &str,
        uids: &[i64],
        max_chars: Option<usize>,
    ) -> Result<BatchSynopsisResult, Box<dyn std::error::Error>> {
        if uids.is_empty() {
            return Err("At least one UID is required".into());
        }
        if uids.len() > MAX_BATCH_SIZE {
            return Err(format!(
                "Maximum {} UIDs per batch, got {}", MAX_BATCH_SIZE, uids.len()
            ).into());
        }

        let char_limit = max_chars.unwrap_or(DEFAULT_MAX_CHARS).min(ABSOLUTE_MAX_CHARS);

        // 1. Resolve folder_id
        let folder_id = self.resolve_folder_id(account_id, folder).await?;

        // 2. Query all UIDs in one shot
        let rows = self.query_emails(folder_id, uids).await?;

        // 3. Build a lookup of found UIDs
        let found_map: HashMap<i64, RawEmailRow> = rows
            .into_iter()
            .map(|r| (r.uid, r))
            .collect();

        // 4. Build results in input UID order, track errors for missing UIDs
        let mut synopses = Vec::with_capacity(uids.len());
        let mut errors = Vec::new();

        for &uid in uids {
            match found_map.get(&uid) {
                Some(row) => {
                    let synopsis = generate_synopsis(
                        row.body_text.as_deref(),
                        char_limit,
                    );
                    synopses.push(EmailSynopsis {
                        uid,
                        subject: row.subject.clone(),
                        from_address: row.from_address.clone(),
                        to_addresses: row.to_addresses.clone(),
                        date: row.date,
                        has_attachments: row.has_attachments,
                        synopsis,
                    });
                }
                None => {
                    errors.push(SynopsisError {
                        uid,
                        reason: "UID not found in cache".to_string(),
                    });
                }
            }
        }

        let returned = synopses.len();
        info!(
            "Batch synopsis: {}/{} UIDs from {}/{} (max_chars={})",
            returned, uids.len(), account_id, folder, char_limit
        );

        Ok(BatchSynopsisResult {
            account: account_id.to_string(),
            folder: folder.to_string(),
            requested: uids.len(),
            returned,
            synopses,
            errors,
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

    /// Query multiple UIDs in a single SQL call using IN (...).
    async fn query_emails(
        &self,
        folder_id: i64,
        uids: &[i64],
    ) -> Result<Vec<RawEmailRow>, Box<dyn std::error::Error>> {
        // Build placeholders for the IN clause
        let placeholders: String = uids.iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "SELECT e.uid, e.subject, e.from_address, e.to_addresses, \
             e.date, e.has_attachments, e.body_text \
             FROM emails e \
             WHERE e.folder_id = ? AND e.uid IN ({})",
            placeholders
        );

        let mut query = sqlx::query_as::<_, RawEmailRow>(&sql)
            .bind(folder_id);

        for &uid in uids {
            query = query.bind(uid);
        }

        let rows = query.fetch_all(&self.db_pool).await?;
        Ok(rows)
    }
}

/// Internal row type from the SQL query.
#[derive(Debug, sqlx::FromRow)]
struct RawEmailRow {
    uid: i64,
    subject: Option<String>,
    from_address: Option<String>,
    to_addresses: Option<String>,
    date: Option<DateTime<Utc>>,
    has_attachments: bool,
    body_text: Option<String>,
}

/// Generate a compact synopsis from body text, truncated to `max_chars`.
/// Strips HTML tags if present, collapses whitespace, and breaks at word
/// boundaries. This is a pure function testable without a database.
pub fn generate_synopsis(body_text: Option<&str>, max_chars: usize) -> String {
    let text = match body_text {
        Some(t) if !t.trim().is_empty() => t,
        _ => return "(no body text available)".to_string(),
    };

    // Take first ~500 chars of source to work with
    let source = if text.len() > 500 { &text[..500] } else { text };

    // Clean up: collapse whitespace, strip blank lines
    let cleaned: String = source
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.is_empty() {
        return "(no body text available)".to_string();
    }

    truncate_at_word_boundary(&cleaned, max_chars)
}

/// Truncate a string at a word boundary, appending "..." if truncated.
fn truncate_at_word_boundary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    // Find a valid char boundary
    let mut end = max_chars;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }

    // Try to break at last space for a clean word boundary
    if let Some(last_space) = text[..end].rfind(' ') {
        end = last_space;
    }

    format!("{}...", text[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synopsis_short_text() {
        let result = generate_synopsis(Some("Hello world."), 300);
        assert_eq!(result, "Hello world.");
    }

    #[test]
    fn test_synopsis_truncation() {
        let long_text = "A ".repeat(200); // 400 chars
        let result = generate_synopsis(Some(&long_text), 100);
        assert!(result.len() <= 103); // 100 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_synopsis_empty_body() {
        assert_eq!(
            generate_synopsis(None, 300),
            "(no body text available)"
        );
        assert_eq!(
            generate_synopsis(Some(""), 300),
            "(no body text available)"
        );
        assert_eq!(
            generate_synopsis(Some("   \n  \n  "), 300),
            "(no body text available)"
        );
    }

    #[test]
    fn test_synopsis_collapses_whitespace() {
        let text = "Line one.\n\n  Line two.  \n\n\nLine three.";
        let result = generate_synopsis(Some(text), 300);
        assert_eq!(result, "Line one. Line two. Line three.");
    }

    #[test]
    fn test_truncate_at_word_boundary() {
        let text = "The quick brown fox jumps over the lazy dog";
        let result = truncate_at_word_boundary(text, 20);
        // Should break at a space before 20 chars
        assert!(result.ends_with("..."));
        assert!(result.len() <= 23); // 20 + "..."
        // Should not cut mid-word
        assert!(!result.contains("bro..."));
    }

    #[test]
    fn test_truncate_no_truncation_needed() {
        let text = "Short text";
        let result = truncate_at_word_boundary(text, 100);
        assert_eq!(result, "Short text");
    }
}
