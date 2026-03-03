// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Metadata-only export: writes email metadata (no body content) to a file
//! on disk, returning just the file path. Designed for large folder inventories
//! where inline results would overwhelm the AI agent's context window.

use chrono::{DateTime, Utc};
use log::info;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::evidence_export::{csv_escape, sanitize_filename};

/// Maximum emails to export in a single call.
const DEFAULT_MAX_EMAILS: usize = 10000;

/// One row of email metadata (no body content).
#[derive(Debug, Serialize)]
pub struct EmailMetadataRow {
    pub uid: i64,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub to_addresses: Option<String>,
    pub cc_addresses: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub has_attachments: bool,
    pub attachment_names: Option<String>,
    pub flags: Option<String>,
    pub size_bytes: Option<i64>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
}

/// Result returned after a successful metadata export.
#[derive(Debug)]
pub struct MetadataExportResult {
    pub file_path: String,
    pub email_count: usize,
    pub format: String,
}

/// Exports email metadata (no bodies) to a file on disk.
pub struct MetadataExporter {
    db_pool: SqlitePool,
}

impl MetadataExporter {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self { db_pool }
    }

    /// Export metadata for all emails in the given account+folder to a file.
    ///
    /// - `account_id`: email address (required)
    /// - `folder`: folder name (required)
    /// - `format`: "json" (default) or "csv"
    /// - `fields`: optional comma-separated field names to include
    /// - `limit`: optional max rows (default: DEFAULT_MAX_EMAILS)
    pub async fn export(
        &self,
        account_id: &str,
        folder: &str,
        format: Option<&str>,
        fields: Option<&str>,
        limit: Option<usize>,
    ) -> Result<MetadataExportResult, Box<dyn std::error::Error>> {
        let max_rows = limit.unwrap_or(DEFAULT_MAX_EMAILS);
        let out_format = format.unwrap_or("json");

        // 1. Resolve folder_id
        let folder_id = self.resolve_folder_id(account_id, folder).await?;

        // 2. Query metadata rows (no body columns)
        let rows = self.query_metadata(folder_id, max_rows).await?;

        // 3. Write to file
        let file_path = self.write_to_file(
            &rows, account_id, folder, out_format, fields,
        )?;

        let count = rows.len();
        info!(
            "Metadata export: {} emails from {}/{} -> {}",
            count, account_id, folder, file_path
        );

        Ok(MetadataExportResult {
            file_path,
            email_count: count,
            format: out_format.to_string(),
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

    /// Query metadata-only columns with LEFT JOIN for attachment names.
    async fn query_metadata(
        &self,
        folder_id: i64,
        max_rows: usize,
    ) -> Result<Vec<EmailMetadataRow>, Box<dyn std::error::Error>> {
        // We use a subquery approach: first get emails, then join attachments.
        // GROUP_CONCAT aggregates attachment filenames with '||' separator.
        let sql = r#"
            SELECT
                e.uid,
                e.subject,
                e.from_address,
                e.to_addresses,
                e.cc_addresses,
                e.date,
                e.has_attachments,
                GROUP_CONCAT(am.filename, '||') as attachment_names,
                e.flags,
                e.size,
                e.message_id,
                NULL as in_reply_to
            FROM emails e
            LEFT JOIN attachment_metadata am
                ON e.message_id = am.message_id
                AND am.account_email = (
                    SELECT account_id FROM folders WHERE id = ?1
                )
            WHERE e.folder_id = ?1
            GROUP BY e.id
            ORDER BY e.date DESC
            LIMIT ?2
        "#;

        let rows = sqlx::query_as::<_, (
            i64,                          // uid
            Option<String>,               // subject
            Option<String>,               // from_address
            Option<String>,               // to_addresses
            Option<String>,               // cc_addresses
            Option<DateTime<Utc>>,        // date
            bool,                         // has_attachments
            Option<String>,               // attachment_names
            Option<String>,               // flags
            Option<i64>,                  // size
            Option<String>,               // message_id
            Option<String>,               // in_reply_to
        )>(sql)
        .bind(folder_id)
        .bind(max_rows as i64)
        .fetch_all(&self.db_pool)
        .await?;

        let metadata: Vec<EmailMetadataRow> = rows.into_iter().map(|r| {
            EmailMetadataRow {
                uid: r.0,
                subject: r.1,
                from_address: r.2,
                to_addresses: r.3,
                cc_addresses: r.4,
                date: r.5,
                has_attachments: r.6,
                attachment_names: r.7,
                flags: r.8,
                size_bytes: r.9,
                message_id: r.10,
                in_reply_to: r.11,
            }
        }).collect();

        Ok(metadata)
    }

    /// Write metadata rows to a temp file in the requested format.
    fn write_to_file(
        &self,
        rows: &[EmailMetadataRow],
        account_id: &str,
        folder: &str,
        format: &str,
        fields: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let safe_account = sanitize_filename(account_id);
        let safe_folder = sanitize_filename(folder);
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let extension = if format == "csv" { "csv" } else { "json" };
        let filename = format!(
            "metadata_{}_{}_{}.{}", safe_account, safe_folder, timestamp, extension
        );

        let out_dir = std::env::temp_dir();
        let file_path = out_dir.join(&filename);

        let field_filter: Option<Vec<&str>> = fields.map(|f| {
            f.split(',').map(|s| s.trim()).collect()
        });

        if format == "csv" {
            let content = self.render_csv(rows, &field_filter);
            std::fs::write(&file_path, content)?;
        } else {
            let content = self.render_json(rows, &field_filter)?;
            std::fs::write(&file_path, content)?;
        }

        Ok(file_path.display().to_string())
    }

    /// Render rows as JSON. Delegates to standalone function.
    fn render_json(
        &self,
        rows: &[EmailMetadataRow],
        field_filter: &Option<Vec<&str>>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        render_metadata_json(rows, field_filter)
    }

    /// Render rows as CSV. Delegates to standalone function.
    fn render_csv(
        &self,
        rows: &[EmailMetadataRow],
        field_filter: &Option<Vec<&str>>,
    ) -> String {
        render_metadata_csv(rows, field_filter)
    }
}

/// Render metadata rows as JSON. If fields filter is set, output only those keys.
fn render_metadata_json(
    rows: &[EmailMetadataRow],
    field_filter: &Option<Vec<&str>>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(ref fields) = field_filter {
        let filtered: Vec<serde_json::Value> = rows.iter().map(|row| {
            let full = serde_json::to_value(row).unwrap_or(serde_json::Value::Null);
            if let serde_json::Value::Object(map) = full {
                let filtered_map: serde_json::Map<String, serde_json::Value> = map
                    .into_iter()
                    .filter(|(k, _)| fields.contains(&k.as_str()))
                    .collect();
                serde_json::Value::Object(filtered_map)
            } else {
                full
            }
        }).collect();
        Ok(serde_json::to_string_pretty(&filtered)?)
    } else {
        Ok(serde_json::to_string_pretty(&rows)?)
    }
}

/// Render metadata rows as CSV. If fields filter is set, output only those columns.
fn render_metadata_csv(
    rows: &[EmailMetadataRow],
    field_filter: &Option<Vec<&str>>,
) -> String {
    let all_fields = [
        "uid", "subject", "from_address", "to_addresses", "cc_addresses",
        "date", "has_attachments", "attachment_names", "flags",
        "size_bytes", "message_id", "in_reply_to",
    ];

    let active_fields: Vec<&str> = if let Some(ref filter) = field_filter {
        all_fields.iter().copied().filter(|f| filter.contains(f)).collect()
    } else {
        all_fields.to_vec()
    };

    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
    lines.push(active_fields.join(","));

    for row in rows {
        let values: Vec<String> = active_fields.iter().map(|&field| {
            match field {
                "uid" => row.uid.to_string(),
                "subject" => csv_escape(row.subject.as_deref().unwrap_or("")),
                "from_address" => csv_escape(row.from_address.as_deref().unwrap_or("")),
                "to_addresses" => csv_escape(row.to_addresses.as_deref().unwrap_or("")),
                "cc_addresses" => csv_escape(row.cc_addresses.as_deref().unwrap_or("")),
                "date" => row.date.map(|d| d.to_rfc3339()).unwrap_or_default(),
                "has_attachments" => row.has_attachments.to_string(),
                "attachment_names" => csv_escape(
                    row.attachment_names.as_deref().unwrap_or("")
                ),
                "flags" => csv_escape(row.flags.as_deref().unwrap_or("")),
                "size_bytes" => row.size_bytes.map(|s| s.to_string()).unwrap_or_default(),
                "message_id" => csv_escape(row.message_id.as_deref().unwrap_or("")),
                "in_reply_to" => csv_escape(row.in_reply_to.as_deref().unwrap_or("")),
                _ => String::new(),
            }
        }).collect();
        lines.push(values.join(","));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> EmailMetadataRow {
        EmailMetadataRow {
            uid: 42,
            subject: Some("Test Subject".to_string()),
            from_address: Some("alice@example.com".to_string()),
            to_addresses: Some("bob@example.com".to_string()),
            cc_addresses: None,
            date: None,
            has_attachments: true,
            attachment_names: Some("doc.pdf||img.png".to_string()),
            flags: Some("[\"Seen\"]".to_string()),
            size_bytes: Some(12345),
            message_id: Some("<msg001@example.com>".to_string()),
            in_reply_to: None,
        }
    }

    #[test]
    fn test_render_json_all_fields() {
        let rows = vec![sample_row()];
        let json = render_metadata_json(&rows, &None).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["uid"], 42);
        assert_eq!(parsed[0]["subject"], "Test Subject");
        assert_eq!(parsed[0]["has_attachments"], true);
    }

    #[test]
    fn test_render_json_filtered_fields() {
        let rows = vec![sample_row()];
        let filter = Some(vec!["uid", "subject"]);
        let json = render_metadata_json(&rows, &filter).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0].as_object().unwrap().len(), 2);
        assert!(parsed[0].get("uid").is_some());
        assert!(parsed[0].get("subject").is_some());
        assert!(parsed[0].get("from_address").is_none());
    }

    #[test]
    fn test_render_csv_all_fields() {
        let rows = vec![sample_row()];
        let csv = render_metadata_csv(&rows, &None);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2); // header + 1 data row
        assert!(lines[0].starts_with("uid,"));
        assert!(lines[1].starts_with("42,"));
    }

    #[test]
    fn test_render_csv_filtered_fields() {
        let rows = vec![sample_row()];
        let filter = Some(vec!["uid", "subject"]);
        let csv = render_metadata_csv(&rows, &filter);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "uid,subject");
        assert!(lines[1].starts_with("42,"));
    }

    #[test]
    fn test_render_csv_escapes_commas() {
        let mut row = sample_row();
        row.subject = Some("Hello, World".to_string());
        let csv = render_metadata_csv(&[row], &None);
        assert!(csv.contains("\"Hello, World\""));
    }
}
