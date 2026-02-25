// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Forensic archiving of cached emails before destructive operations.
//!
//! Creates ZIP archives of email data (JSON-serialized) so that forensically
//! valuable messages are preserved even when the IMAP cache must be flushed
//! (e.g. due to a UIDVALIDITY change on the server).

use chrono::Utc;
use log::info;
use serde_json::json;
use sqlx::SqlitePool;
use std::io::Write;
use std::path::PathBuf;
use zip::write::FileOptions;
use zip::ZipWriter;

/// Default directory for forensic archives when env var is not set.
const DEFAULT_ARCHIVE_DIR: &str = "data/forensic_archives";

/// Create a forensic ZIP archive of all cached emails for a folder.
///
/// Called before flushing a folder's email cache (e.g. on UIDVALIDITY change).
/// Returns the path to the created archive, or `None` if there were no emails
/// to archive.
pub async fn create_forensic_archive(
    pool: &SqlitePool,
    folder_id: i64,
    folder_name: &str,
    account_email: &str,
    old_uidvalidity: i64,
    new_uidvalidity: i64,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    // Query all emails for this folder
    let rows = sqlx::query_as::<_, EmailRow>(
        "SELECT uid, message_id, subject, from_address, from_name, \
         to_addresses, cc_addresses, date, body_text, body_html, \
         flags, size, has_attachments \
         FROM emails WHERE folder_id = ?"
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        info!("No emails to archive for folder '{}' (folder_id={})", folder_name, folder_id);
        return Ok(None);
    }

    // Determine archive directory
    let archive_dir = std::env::var("FORENSIC_ARCHIVE_DIR")
        .unwrap_or_else(|_| DEFAULT_ARCHIVE_DIR.to_string());
    let archive_dir = PathBuf::from(&archive_dir);
    std::fs::create_dir_all(&archive_dir)?;

    // Build filename: {timestamp}_{account}_{folder}_{old}_to_{new}.zip
    let timestamp = Utc::now().format("%Y-%m-%d_%H-%M-%S");
    let safe_folder = sanitize_for_filename(folder_name);
    let safe_account = sanitize_for_filename(account_email);
    let filename = format!(
        "{}_{}_{}_{}_to_{}.zip",
        timestamp, safe_account, safe_folder, old_uidvalidity, new_uidvalidity
    );
    let archive_path = archive_dir.join(&filename);

    // Create ZIP file
    let file = std::fs::File::create(&archive_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // Write manifest
    let manifest = json!({
        "account": account_email,
        "folder": folder_name,
        "folder_id": folder_id,
        "old_uidvalidity": old_uidvalidity,
        "new_uidvalidity": new_uidvalidity,
        "archived_at": Utc::now().to_rfc3339(),
        "email_count": rows.len(),
    });
    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    // Write each email as a separate JSON file
    for row in &rows {
        let email_json = json!({
            "uid": row.uid,
            "message_id": row.message_id,
            "subject": row.subject,
            "from_address": row.from_address,
            "from_name": row.from_name,
            "to_addresses": row.to_addresses,
            "cc_addresses": row.cc_addresses,
            "date": row.date,
            "body_text": row.body_text,
            "body_html": row.body_html,
            "flags": row.flags,
            "size": row.size,
            "has_attachments": row.has_attachments,
        });
        let entry_name = format!("email_{}.json", row.uid);
        zip.start_file(&entry_name, options)?;
        zip.write_all(serde_json::to_string_pretty(&email_json)?.as_bytes())?;
    }

    zip.finish()?;
    info!(
        "Created forensic archive: {} ({} emails from '{}' for {})",
        archive_path.display(), rows.len(), folder_name, account_email
    );

    Ok(Some(archive_path))
}

/// Replace characters that are unsafe in filenames with underscores.
fn sanitize_for_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '/' | '\\' | '.' | ':' | ' ' | '@' => '_',
            _ => c,
        })
        .collect()
}

/// Row struct for the email query (maps to DB columns).
#[derive(sqlx::FromRow)]
struct EmailRow {
    uid: i64,
    message_id: Option<String>,
    subject: Option<String>,
    from_address: Option<String>,
    from_name: Option<String>,
    to_addresses: Option<String>,
    cc_addresses: Option<String>,
    date: Option<String>,
    body_text: Option<String>,
    body_html: Option<String>,
    flags: Option<String>,
    size: Option<i64>,
    has_attachments: bool,
}
