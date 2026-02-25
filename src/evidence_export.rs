// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Evidence export: packages cached emails and attachments into organized
//! directories for attorney review. Creates JSON email files, copies
//! attachment files, a CSV manifest, and a markdown summary.

use chrono::Utc;
use log::info;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;

use crate::dashboard::services::cache::CacheService;
use crate::dashboard::services::attachment_storage;

/// Default output directory when env var is not set.
const DEFAULT_EXPORT_DIR: &str = "data/evidence_exports";
/// Default max emails per export.
const DEFAULT_MAX_EMAILS: usize = 10000;

/// Result returned by a successful export.
pub struct ExportResult {
    pub export_path: String,
    pub email_count: usize,
    pub attachment_count: usize,
}

/// Orchestrates packaging cached emails into a browsable evidence directory.
pub struct EvidenceExporter {
    cache_service: Arc<CacheService>,
    db_pool: SqlitePool,
}

impl EvidenceExporter {
    pub fn new(cache_service: Arc<CacheService>, db_pool: SqlitePool) -> Self {
        Self { cache_service, db_pool }
    }

    /// Export emails and attachments into an organized evidence directory.
    ///
    /// - `account_id`: email address of the account (required)
    /// - `folder`: optional folder name to limit export
    /// - `search_query`: optional search string to filter emails
    /// - `output_path`: optional override for the output directory
    pub async fn export(
        &self,
        account_id: &str,
        folder: Option<&str>,
        search_query: Option<&str>,
        output_path: Option<&str>,
    ) -> Result<ExportResult, Box<dyn std::error::Error>> {
        let max_emails = std::env::var("EVIDENCE_EXPORT_MAX_EMAILS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_EMAILS);

        // 1. Resolve output directory
        let base_dir = match output_path {
            Some(p) => p.to_string(),
            None => std::env::var("EVIDENCE_EXPORT_DIR")
                .unwrap_or_else(|_| DEFAULT_EXPORT_DIR.to_string()),
        };

        // 2. Create timestamped subdirectory
        let timestamp = Utc::now().format("%Y-%m-%d_%H-%M-%S");
        let safe_account = sanitize_filename(account_id);
        let subdir_name = format!("{}_{}", timestamp, safe_account);
        let export_dir = PathBuf::from(&base_dir).join(&subdir_name);
        let emails_dir = export_dir.join("emails");
        let attachments_dir = export_dir.join("attachments");
        std::fs::create_dir_all(&emails_dir)?;
        std::fs::create_dir_all(&attachments_dir)?;

        // 3. Collect emails
        let emails = self.collect_emails(account_id, folder, search_query, max_emails).await?;

        // 4. Write email JSON files and copy attachments
        let mut attachment_count: usize = 0;
        let mut manifest_rows: Vec<String> = Vec::new();

        // CSV header
        manifest_rows.push(
            "uid,message_id,subject,from,to,date,has_attachments,synopsis".to_string()
        );

        for email in &emails {
            // Write email JSON
            let safe_subj = sanitize_filename(
                email.subject.as_deref().unwrap_or("no_subject")
            );
            let json_filename = format!("{}_{}.json", email.uid, safe_subj);
            let json_path = emails_dir.join(&json_filename);
            let json_str = serde_json::to_string_pretty(&email)?;
            std::fs::write(&json_path, json_str)?;

            // Copy attachments if present
            if email.has_attachments {
                if let Some(ref message_id) = email.message_id {
                    if let Ok(attachments) = attachment_storage::get_attachments_metadata(
                        &self.db_pool, account_id, message_id
                    ).await {
                        for att in &attachments {
                            let dest_name = format!("{}_{}", email.uid, att.filename);
                            let dest_path = attachments_dir.join(&dest_name);
                            // storage_path is relative; resolve from working dir
                            let src_path = PathBuf::from(&att.storage_path);
                            if src_path.exists() {
                                if let Err(e) = std::fs::copy(&src_path, &dest_path) {
                                    log::warn!(
                                        "Failed to copy attachment {}: {}",
                                        att.filename, e
                                    );
                                } else {
                                    attachment_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            // Build CSV row
            let synopsis = generate_synopsis(&email.body_text);
            let to_str = email.to_addresses.join("; ");
            let date_str = email.date.map(|d| d.to_rfc3339()).unwrap_or_default();
            let row = format!(
                "{},{},{},{},{},{},{},{}",
                email.uid,
                csv_escape(email.message_id.as_deref().unwrap_or("")),
                csv_escape(email.subject.as_deref().unwrap_or("")),
                csv_escape(email.from_address.as_deref().unwrap_or("")),
                csv_escape(&to_str),
                csv_escape(&date_str),
                email.has_attachments,
                csv_escape(&synopsis),
            );
            manifest_rows.push(row);
        }

        // 5. Write manifest.csv
        let manifest_path = export_dir.join("manifest.csv");
        std::fs::write(&manifest_path, manifest_rows.join("\n"))?;

        // 6. Write summary.md
        let summary = format!(
            "# Evidence Export\n\n\
             - **Account:** {}\n\
             - **Folder:** {}\n\
             - **Search query:** {}\n\
             - **Exported at:** {}\n\
             - **Email count:** {}\n\
             - **Attachment count:** {}\n\
             - **Export path:** {}\n",
            account_id,
            folder.unwrap_or("(all folders)"),
            search_query.unwrap_or("(none)"),
            Utc::now().to_rfc3339(),
            emails.len(),
            attachment_count,
            export_dir.display(),
        );
        let summary_path = export_dir.join("summary.md");
        std::fs::write(&summary_path, summary)?;

        info!(
            "Evidence export complete: {} emails, {} attachments -> {}",
            emails.len(),
            attachment_count,
            export_dir.display()
        );

        Ok(ExportResult {
            export_path: export_dir.display().to_string(),
            email_count: emails.len(),
            attachment_count,
        })
    }

    /// Collect emails based on the provided filters.
    async fn collect_emails(
        &self,
        account_id: &str,
        folder: Option<&str>,
        search_query: Option<&str>,
        max_emails: usize,
    ) -> Result<Vec<crate::dashboard::services::cache::CachedEmail>, Box<dyn std::error::Error>> {
        use crate::dashboard::services::cache::CachedEmail;

        let mut all_emails: Vec<CachedEmail> = Vec::new();

        if let Some(query) = search_query {
            let folder_name = folder.unwrap_or("");
            let results = self.cache_service
                .search_cached_emails_for_account(folder_name, query, max_emails, account_id)
                .await?;
            all_emails = results;
        } else if let Some(folder_name) = folder {
            let results = self.cache_service
                .get_cached_emails_for_account(folder_name, account_id, max_emails, 0, false)
                .await?;
            all_emails = results;
        } else {
            // No folder or query: iterate all folders
            let folders = self.cache_service
                .get_all_cached_folders_for_account(account_id)
                .await?;
            for cached_folder in &folders {
                if all_emails.len() >= max_emails {
                    break;
                }
                let remaining = max_emails - all_emails.len();
                let results = self.cache_service
                    .get_cached_emails_for_account(
                        &cached_folder.name, account_id, remaining, 0, false
                    )
                    .await?;
                all_emails.extend(results);
            }
        }

        Ok(all_emails)
    }
}

/// Generate a brief synopsis from the body text: first 5 non-empty lines, max 500 chars.
fn generate_synopsis(body_text: &Option<String>) -> String {
    let text = match body_text {
        Some(t) => t,
        None => return String::new(),
    };
    let mut lines_collected = 0;
    let mut result = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if lines_collected > 0 {
            result.push(' ');
        }
        result.push_str(trimmed);
        lines_collected += 1;
        if lines_collected >= 5 || result.len() >= 500 {
            break;
        }
    }
    if result.len() > 500 {
        result.truncate(500);
    }
    result
}

/// RFC 4180 CSV field escaping: wrap in quotes if the field contains commas,
/// quotes, or newlines. Double any internal quotes.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_string()
    }
}

/// Sanitize a string for safe use as a filename component.
/// Keeps alphanumeric, hyphens, and underscores; replaces everything else.
/// Truncates to 50 characters.
fn sanitize_filename(input: &str) -> String {
    let sanitized: String = input.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            c
        } else {
            '_'
        }
    }).collect();
    if sanitized.len() > 50 {
        sanitized[..50].to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_synopsis_none() {
        assert_eq!(generate_synopsis(&None), "");
    }

    #[test]
    fn test_generate_synopsis_basic() {
        let text = Some("Hello world.\nThis is line two.\n\nLine four.".to_string());
        let result = generate_synopsis(&text);
        assert_eq!(result, "Hello world. This is line two. Line four.");
    }

    #[test]
    fn test_generate_synopsis_max_lines() {
        let text = Some("A\nB\nC\nD\nE\nF\nG".to_string());
        let result = generate_synopsis(&text);
        assert_eq!(result, "A B C D E");
    }

    #[test]
    fn test_generate_synopsis_truncates_at_500() {
        let long_line = "x".repeat(600);
        let text = Some(long_line);
        let result = generate_synopsis(&text);
        assert_eq!(result.len(), 500);
    }

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_comma() {
        assert_eq!(csv_escape("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn test_csv_escape_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("hello@world.com"), "hello_world_com");
    }

    #[test]
    fn test_sanitize_filename_preserves_allowed() {
        assert_eq!(sanitize_filename("my-file_name123"), "my-file_name123");
    }

    #[test]
    fn test_sanitize_filename_truncates() {
        let long_input = "a".repeat(100);
        let result = sanitize_filename(&long_input);
        assert_eq!(result.len(), 50);
    }
}
