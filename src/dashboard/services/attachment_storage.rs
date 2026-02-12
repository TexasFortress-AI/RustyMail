// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use chrono::{DateTime, Utc};
use log::{info, debug, warn, error};
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use crate::imap::types::{Email, MimePart};

#[derive(Error, Debug)]
pub enum AttachmentError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("ZIP error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("Attachment not found: {0}")]
    NotFound(String),
    #[error("Invalid message ID: {0}")]
    InvalidMessageId(String),
    #[error("Path traversal attempt detected")]
    PathTraversal,
    #[error("Invalid filename: {0}")]
    InvalidFilename(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    pub size_bytes: i64,
    pub content_type: Option<String>,
    pub content_id: Option<String>,
    pub downloaded_at: DateTime<Utc>,
    pub storage_path: String,
}

/// Sanitize message-id for safe filesystem use
/// Removes angle brackets and replaces invalid filesystem characters
pub fn sanitize_message_id(message_id: &str) -> String {
    message_id
        .trim_matches('<')
        .trim_matches('>')
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c
        })
        .collect::<String>()
        .chars()
        .take(200) // Limit length for filesystem safety
        .collect()
}

/// Sanitize filename to prevent path traversal attacks
/// Rejects any filename containing path traversal patterns - fails closed for security
pub fn sanitize_filename(filename: &str) -> Result<String, AttachmentError> {
    // Reject null bytes
    if filename.contains('\0') {
        warn!("Path traversal attempt: null byte in filename");
        return Err(AttachmentError::InvalidFilename("Null byte in filename".to_string()));
    }

    // Reject path traversal patterns - fail closed rather than silently sanitizing
    if filename.contains("..") {
        warn!("Path traversal attempt: '..' in filename '{}'", filename);
        return Err(AttachmentError::PathTraversal);
    }

    // Reject path separators - filenames shouldn't contain directory components
    if filename.contains('/') || filename.contains('\\') {
        warn!("Path traversal attempt: path separator in filename '{}'", filename);
        return Err(AttachmentError::PathTraversal);
    }

    // Reject empty filenames
    if filename.is_empty() || filename == "." {
        warn!("Path traversal attempt: empty or dot filename");
        return Err(AttachmentError::InvalidFilename("Invalid filename".to_string()));
    }

    // Additional sanitization: replace dangerous characters
    let sanitized: String = filename
        .chars()
        .map(|c| match c {
            ':' => '_',  // Windows drive separator
            c => c
        })
        .collect();

    // Limit length
    let result: String = sanitized.chars().take(255).collect();

    Ok(result)
}

/// Get the attachments storage root directory
fn get_storage_root() -> PathBuf {
    std::env::var("ATTACHMENTS_STORAGE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("attachments"))
}

/// Validate that a path is safely contained within the storage root
/// Uses canonicalization to resolve symlinks and relative paths
fn validate_path_containment(storage_root: &Path, full_path: &Path) -> Result<PathBuf, AttachmentError> {
    // Ensure storage root exists and get its canonical form
    if !storage_root.exists() {
        fs::create_dir_all(storage_root)?;
    }
    let canonical_root = fs::canonicalize(storage_root)?;

    // For new files (that don't exist yet), we need to check the parent directory
    if full_path.exists() {
        // File exists - canonicalize it directly
        let canonical_path = fs::canonicalize(full_path)?;
        if !canonical_path.starts_with(&canonical_root) {
            warn!("Path traversal attempt: {:?} escapes storage root {:?}", full_path, canonical_root);
            return Err(AttachmentError::PathTraversal);
        }
        Ok(canonical_path)
    } else {
        // File doesn't exist - check parent directory and construct path
        let parent = full_path.parent()
            .ok_or_else(|| AttachmentError::InvalidFilename("No parent directory".to_string()))?;

        // Ensure parent directory exists
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }

        let canonical_parent = fs::canonicalize(parent)?;
        if !canonical_parent.starts_with(&canonical_root) {
            warn!("Path traversal attempt: parent {:?} escapes storage root {:?}", parent, canonical_root);
            return Err(AttachmentError::PathTraversal);
        }

        let filename = full_path.file_name()
            .ok_or_else(|| AttachmentError::InvalidFilename("No filename".to_string()))?;

        Ok(canonical_parent.join(filename))
    }
}

/// Ensure an email has a message-id, generating one if needed
pub fn ensure_message_id(email: &Email, account: &str) -> String {
    if let Some(envelope) = &email.envelope {
        if let Some(message_id) = &envelope.message_id {
            return message_id.clone();
        }
    }

    // Generate stable pseudo message-id from email metadata
    let uid = email.uid;
    let date = email.internal_date.map(|d| d.timestamp()).unwrap_or(0);
    format!("rustymail-{}-{}-{}@local",
            account.replace('@', "_"),
            uid,
            date)
}

/// Get the storage path for an attachment with secure path validation
/// Format: {storage_root}/{sanitized_account}/{sanitized_message_id}/{sanitized_filename}
/// Returns error if path would escape the storage root
pub fn get_attachment_path(account: &str, message_id: &str, filename: &str) -> Result<PathBuf, AttachmentError> {
    // Sanitize all path components
    let sanitized_account = sanitize_message_id(account); // Reuse for account sanitization
    let sanitized_id = sanitize_message_id(message_id);
    let sanitized_filename = sanitize_filename(filename)?;

    let storage_root = get_storage_root();
    let relative_path = Path::new(&sanitized_account)
        .join(&sanitized_id)
        .join(&sanitized_filename);

    let full_path = storage_root.join(&relative_path);

    // Validate the constructed path is within the storage root
    validate_path_containment(&storage_root, &full_path)
}

/// Save an attachment to filesystem and record metadata in database
pub async fn save_attachment(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
    mime_part: &MimePart,
) -> Result<AttachmentInfo, AttachmentError> {
    // Get filename from content disposition or generate one
    let filename = mime_part.content_disposition
        .as_ref()
        .and_then(|cd| cd.filename())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate filename from content type
            let ext = match mime_part.content_type.sub_type.as_str() {
                "pdf" => "pdf",
                "jpeg" | "jpg" => "jpg",
                "png" => "png",
                "gif" => "gif",
                "plain" => "txt",
                "html" => "html",
                _ => "bin",
            };
            format!("attachment_{}.{}", Utc::now().timestamp(), ext)
        });

    // Get secure storage path with validation
    let storage_path = get_attachment_path(account, message_id, &filename)?;

    // Note: validate_path_containment already creates directories as needed
    // Write attachment to filesystem using the validated path
    let mut file = fs::File::create(&storage_path)?;
    file.write_all(&mime_part.body)?;

    debug!("Saved attachment {} to {:?}", filename, storage_path);

    let size_bytes = mime_part.body.len() as i64;
    let content_type = Some(mime_part.content_type.mime_type());
    let content_id = mime_part.content_id.clone();
    let relative_path = storage_path.to_string_lossy().to_string();

    // Insert metadata into database
    sqlx::query(
        r#"
        INSERT INTO attachment_metadata
            (message_id, account_email, filename, size_bytes, content_type, content_id, storage_path)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(message_id, account_email, filename) DO UPDATE SET
            size_bytes = excluded.size_bytes,
            content_type = excluded.content_type,
            content_id = excluded.content_id,
            storage_path = excluded.storage_path,
            downloaded_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(message_id)
    .bind(account)
    .bind(&filename)
    .bind(size_bytes)
    .bind(&content_type)
    .bind(&content_id)
    .bind(&relative_path)
    .execute(pool)
    .await?;

    info!("Saved attachment metadata for {} (message: {}, content_id: {:?})", filename, message_id, content_id);

    Ok(AttachmentInfo {
        filename,
        size_bytes,
        content_type,
        content_id,
        downloaded_at: Utc::now(),
        storage_path: relative_path,
    })
}

/// Get attachment metadata for an email by message-id
pub async fn get_attachments_metadata(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
) -> Result<Vec<AttachmentInfo>, AttachmentError> {
    let attachments = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, DateTime<Utc>, String)>(
        r#"
        SELECT filename, size_bytes, content_type, content_id, downloaded_at, storage_path
        FROM attachment_metadata
        WHERE message_id = ? AND account_email = ?
        ORDER BY downloaded_at ASC
        "#
    )
    .bind(message_id)
    .bind(account)
    .fetch_all(pool)
    .await?;

    Ok(attachments
        .into_iter()
        .map(|(filename, size_bytes, content_type, content_id, downloaded_at, storage_path)| AttachmentInfo {
            filename,
            size_bytes,
            content_type,
            content_id,
            downloaded_at,
            storage_path,
        })
        .collect())
}

/// Get an inline attachment by Content-ID
pub async fn get_attachment_by_content_id(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
    content_id: &str,
) -> Result<Option<AttachmentInfo>, AttachmentError> {
    // Normalize content_id by removing angle brackets if present
    let normalized_cid = content_id
        .trim_start_matches('<')
        .trim_end_matches('>');

    let attachment = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, DateTime<Utc>, String)>(
        r#"
        SELECT filename, size_bytes, content_type, content_id, downloaded_at, storage_path
        FROM attachment_metadata
        WHERE message_id = ? AND account_email = ?
          AND (content_id = ? OR content_id = ? OR content_id = ?)
        LIMIT 1
        "#
    )
    .bind(message_id)
    .bind(account)
    .bind(content_id) // Try exact match
    .bind(normalized_cid) // Try without brackets
    .bind(format!("<{}>", normalized_cid)) // Try with brackets
    .fetch_optional(pool)
    .await?;

    Ok(attachment.map(|(filename, size_bytes, content_type, content_id, downloaded_at, storage_path)| AttachmentInfo {
        filename,
        size_bytes,
        content_type,
        content_id,
        downloaded_at,
        storage_path,
    }))
}

/// Delete all attachments for an email
pub async fn delete_attachments_for_email(
    pool: &SqlitePool,
    message_id: &str,
    account: &str,
) -> Result<(), AttachmentError> {
    let storage_root = get_storage_root();

    // Get attachment metadata before deleting
    let attachments = get_attachments_metadata(pool, account, message_id).await?;

    // Delete from filesystem with path validation
    for attachment in &attachments {
        // Re-validate path containment before deletion to prevent symlink attacks
        let path = Path::new(&attachment.storage_path);
        match validate_path_containment(&storage_root, path) {
            Ok(validated_path) => {
                if validated_path.exists() {
                    if let Err(e) = fs::remove_file(&validated_path) {
                        warn!("Failed to delete attachment file {:?}: {}", validated_path, e);
                    } else {
                        debug!("Deleted attachment file: {:?}", validated_path);
                    }
                }
            }
            Err(e) => {
                warn!("Skipping deletion of suspicious path {:?}: {}", path, e);
            }
        }
    }

    // Clean up empty directories with sanitization
    let sanitized_account = sanitize_message_id(account);
    let sanitized_id = sanitize_message_id(message_id);
    let message_dir = storage_root
        .join(&sanitized_account)
        .join(&sanitized_id);

    // Validate the directory path before attempting removal
    if let Ok(validated_dir) = validate_path_containment(&storage_root, &message_dir) {
        if validated_dir.exists() {
            if let Err(e) = fs::remove_dir(&validated_dir) {
                debug!("Could not remove message dir {:?}: {} (may not be empty)", validated_dir, e);
            }
        }
    }

    // Delete from database
    let deleted = sqlx::query(
        "DELETE FROM attachment_metadata WHERE message_id = ? AND account_email = ?"
    )
    .bind(message_id)
    .bind(account)
    .execute(pool)
    .await?
    .rows_affected();

    info!("Deleted {} attachment(s) for message {} (account: {})", deleted, message_id, account);

    Ok(())
}

/// Create a ZIP archive of all attachments for an email
pub async fn create_zip_archive(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
    output_path: &Path,
) -> Result<PathBuf, AttachmentError> {
    use zip::write::FileOptions;
    use zip::ZipWriter;

    let storage_root = get_storage_root();
    let attachments = get_attachments_metadata(pool, account, message_id).await?;

    if attachments.is_empty() {
        return Err(AttachmentError::NotFound("No attachments found".to_string()));
    }

    // Create output directory if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let mut files_added = 0;
    for attachment in &attachments {
        let path = Path::new(&attachment.storage_path);

        // Validate path containment before reading
        match validate_path_containment(&storage_root, path) {
            Ok(validated_path) => {
                if validated_path.exists() {
                    // Sanitize filename for ZIP entry (prevent zip slip attacks)
                    let safe_filename = sanitize_filename(&attachment.filename)
                        .unwrap_or_else(|_| format!("attachment_{}", files_added));

                    zip.start_file(&safe_filename, options)?;
                    let content = fs::read(&validated_path)?;
                    zip.write_all(&content)?;
                    debug!("Added {} to ZIP archive", safe_filename);
                    files_added += 1;
                } else {
                    warn!("Attachment file not found: {:?}", validated_path);
                }
            }
            Err(e) => {
                warn!("Skipping suspicious attachment path {:?}: {}", path, e);
            }
        }
    }

    zip.finish()?;
    info!("Created ZIP archive at {:?} with {} files", output_path, files_added);

    Ok(output_path.to_path_buf())
}

/// Search for emails that have attachments matching given MIME types.
/// Supports wildcard patterns like "image/*" or exact types like "application/pdf".
/// Returns a list of (message_id, filename, content_type, size_bytes) tuples.
pub async fn search_by_attachment_type(
    pool: &SqlitePool,
    account: &str,
    mime_patterns: &[String],
    limit: usize,
) -> Result<Vec<AttachmentInfo>, AttachmentError> {
    if mime_patterns.is_empty() {
        return Ok(Vec::new());
    }

    // Build LIKE clauses for each pattern
    // "image/*" becomes "image/%" for SQL LIKE
    let like_patterns: Vec<String> = mime_patterns.iter()
        .map(|p| p.replace('*', "%"))
        .collect();

    let placeholders: Vec<String> = like_patterns.iter()
        .map(|_| "content_type LIKE ?".to_string())
        .collect();
    let where_clause = placeholders.join(" OR ");

    let query_str = format!(
        r#"
        SELECT filename, size_bytes, content_type, content_id, downloaded_at, storage_path
        FROM attachment_metadata
        WHERE account_email = ? AND ({})
        ORDER BY downloaded_at DESC
        LIMIT ?
        "#,
        where_clause
    );

    let mut query = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, DateTime<Utc>, String)>(&query_str)
        .bind(account);

    for pattern in &like_patterns {
        query = query.bind(pattern);
    }
    query = query.bind(limit as i64);

    let rows = query.fetch_all(pool).await?;

    Ok(rows.into_iter()
        .map(|(filename, size_bytes, content_type, content_id, downloaded_at, storage_path)| AttachmentInfo {
            filename, size_bytes, content_type, content_id, downloaded_at, storage_path,
        })
        .collect())
}

/// Store attachment metadata during email sync without downloading file contents.
/// Uses empty storage_path since files are not yet downloaded.
/// On conflict, only updates metadata fields (size, content_type, content_id)
/// and preserves any existing storage_path from a prior download.
pub async fn store_attachment_metadata_from_mime(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
    attachments: &[MimePart],
) -> Result<usize, AttachmentError> {
    let mut stored = 0;
    for mime_part in attachments {
        let filename = mime_part.content_disposition.as_ref()
            .and_then(|d| d.filename().cloned())
            .unwrap_or_else(|| {
                let ext = match mime_part.content_type.sub_type.as_str() {
                    "pdf" => "pdf",
                    "jpeg" | "jpg" => "jpg",
                    "png" => "png",
                    "gif" => "gif",
                    "plain" => "txt",
                    "html" => "html",
                    _ => "bin",
                };
                format!("attachment_{}.{}", stored, ext)
            });

        let size_bytes = mime_part.body.len() as i64;
        let content_type = Some(mime_part.content_type.mime_type());
        let content_id = mime_part.content_id.clone();

        sqlx::query(
            r#"
            INSERT INTO attachment_metadata
                (message_id, account_email, filename, size_bytes, content_type, content_id, storage_path)
            VALUES (?, ?, ?, ?, ?, ?, '')
            ON CONFLICT(message_id, account_email, filename) DO UPDATE SET
                size_bytes = excluded.size_bytes,
                content_type = excluded.content_type,
                content_id = excluded.content_id
            "#
        )
        .bind(message_id)
        .bind(account)
        .bind(&filename)
        .bind(size_bytes)
        .bind(&content_type)
        .bind(&content_id)
        .execute(pool)
        .await?;

        stored += 1;
    }
    Ok(stored)
}

/// Read a single attachment's content from disk, returning base64-encoded data
pub async fn read_attachment_content(
    pool: &SqlitePool,
    account: &str,
    message_id: &str,
    filename: &str,
) -> Result<(String, String, Vec<u8>), AttachmentError> {
    // Look up attachment metadata from DB
    let row = sqlx::query(
        "SELECT storage_path, content_type FROM attachment_metadata
         WHERE message_id = ? AND account_email = ? AND filename = ?"
    )
    .bind(message_id)
    .bind(account)
    .bind(filename)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AttachmentError::NotFound(
        format!("Attachment '{}' not found for message {}", filename, message_id)
    ))?;

    let storage_path: String = sqlx::Row::get(&row, "storage_path");
    let content_type: Option<String> = sqlx::Row::get(&row, "content_type");

    if storage_path.is_empty() {
        return Err(AttachmentError::NotFound(
            format!("Attachment '{}' has metadata only (not yet downloaded from IMAP)", filename)
        ));
    }

    // Validate path is within storage root before reading
    let storage_root = get_storage_root();
    let full_path = PathBuf::from(&storage_path);
    validate_path_containment(&storage_root, &full_path)?;

    let content = fs::read(&full_path)?;
    let mime = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    Ok((filename.to_string(), mime, content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_message_id() {
        assert_eq!(
            sanitize_message_id("<abc@example.com>"),
            "abc@example.com"
        );
        assert_eq!(
            sanitize_message_id("<abc/def:123*456?.com>"),
            "abc_def_123_456_.com"
        );
        assert_eq!(
            sanitize_message_id("simple@example.com"),
            "simple@example.com"
        );
    }

    #[test]
    fn test_ensure_message_id() {
        use crate::imap::types::{Email, Envelope};

        let mut email = Email {
            uid: 123,
            flags: vec![],
            internal_date: None,
            envelope: None,
            body: None,
            mime_parts: vec![],
            text_body: None,
            html_body: None,
            attachments: vec![],
        };

        // Test with no envelope - should generate ID
        let generated = ensure_message_id(&email, "test@example.com");
        assert!(generated.starts_with("rustymail-"));
        assert!(generated.contains("test_example.com"));

        // Test with message-id in envelope
        email.envelope = Some(Envelope {
            message_id: Some("<real-id@example.com>".to_string()),
            subject: None,
            from: vec![],
            to: vec![],
            cc: vec![],
            bcc: vec![],
            reply_to: vec![],
            date: None,
            in_reply_to: None,
        });

        let real = ensure_message_id(&email, "test@example.com");
        assert_eq!(real, "<real-id@example.com>");
    }

    #[test]
    fn test_get_attachment_path() {
        let path = get_attachment_path(
            "user@example.com",
            "<msg123@server.com>",
            "invoice.pdf"
        ).expect("Should return valid path for normal inputs");

        assert!(path.to_string_lossy().contains("attachments"));
        assert!(path.to_string_lossy().contains("user@example.com"));
        assert!(path.to_string_lossy().contains("invoice.pdf"));
        assert!(!path.to_string_lossy().contains("<")); // Should be sanitized
        assert!(!path.to_string_lossy().contains(">")); // Should be sanitized
    }

    #[test]
    fn test_get_attachment_path_rejects_traversal() {
        // Path traversal in filename should be rejected
        let result = get_attachment_path(
            "user@example.com",
            "<msg123@server.com>",
            "../../../etc/passwd"
        );
        assert!(result.is_err());

        // Null byte in filename should be rejected
        let result = get_attachment_path(
            "user@example.com",
            "<msg123@server.com>",
            "file\0.pdf"
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        // Normal filename should pass
        assert!(sanitize_filename("document.pdf").is_ok());

        // Path traversal should be rejected
        assert!(sanitize_filename("../secret.txt").is_err());
        assert!(sanitize_filename("..\\secret.txt").is_err());

        // Null bytes should be rejected
        assert!(sanitize_filename("file\0.txt").is_err());

        // Empty filename should be rejected
        assert!(sanitize_filename("").is_err());
    }
}
