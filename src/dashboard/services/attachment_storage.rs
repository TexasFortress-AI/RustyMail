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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    pub size_bytes: i64,
    pub content_type: Option<String>,
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

/// Get the storage path for an attachment
/// Format: attachments/{account_email}/{sanitized_message_id}/{filename}
pub fn get_attachment_path(account: &str, message_id: &str, filename: &str) -> PathBuf {
    let sanitized_id = sanitize_message_id(message_id);
    Path::new("attachments")
        .join(account)
        .join(sanitized_id)
        .join(filename)
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

    let storage_path = get_attachment_path(account, message_id, &filename);

    // Create directory if it doesn't exist
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write attachment to filesystem
    let mut file = fs::File::create(&storage_path)?;
    file.write_all(&mime_part.body)?;

    debug!("Saved attachment {} to {:?}", filename, storage_path);

    let size_bytes = mime_part.body.len() as i64;
    let content_type = Some(mime_part.content_type.mime_type());
    let relative_path = storage_path.to_string_lossy().to_string();

    // Insert metadata into database
    sqlx::query(
        r#"
        INSERT INTO attachment_metadata
            (message_id, account_email, filename, size_bytes, content_type, storage_path)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(message_id, account_email, filename) DO UPDATE SET
            size_bytes = excluded.size_bytes,
            content_type = excluded.content_type,
            storage_path = excluded.storage_path,
            downloaded_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(message_id)
    .bind(account)
    .bind(&filename)
    .bind(size_bytes)
    .bind(&content_type)
    .bind(&relative_path)
    .execute(pool)
    .await?;

    info!("Saved attachment metadata for {} (message: {})", filename, message_id);

    Ok(AttachmentInfo {
        filename,
        size_bytes,
        content_type,
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
    let attachments = sqlx::query_as::<_, (String, i64, Option<String>, DateTime<Utc>, String)>(
        r#"
        SELECT filename, size_bytes, content_type, downloaded_at, storage_path
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
        .map(|(filename, size_bytes, content_type, downloaded_at, storage_path)| AttachmentInfo {
            filename,
            size_bytes,
            content_type,
            downloaded_at,
            storage_path,
        })
        .collect())
}

/// Delete all attachments for an email
pub async fn delete_attachments_for_email(
    pool: &SqlitePool,
    message_id: &str,
    account: &str,
) -> Result<(), AttachmentError> {
    // Get attachment metadata before deleting
    let attachments = get_attachments_metadata(pool, account, message_id).await?;

    // Delete from filesystem
    for attachment in &attachments {
        let path = Path::new(&attachment.storage_path);
        if path.exists() {
            if let Err(e) = fs::remove_file(path) {
                warn!("Failed to delete attachment file {:?}: {}", path, e);
            } else {
                debug!("Deleted attachment file: {:?}", path);
            }
        }
    }

    // Clean up empty directories
    let sanitized_id = sanitize_message_id(message_id);
    let message_dir = Path::new("attachments")
        .join(account)
        .join(sanitized_id);

    if message_dir.exists() {
        if let Err(e) = fs::remove_dir(&message_dir) {
            debug!("Could not remove message dir {:?}: {} (may not be empty)", message_dir, e);
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

    let attachment_count = attachments.len();
    for attachment in &attachments {
        let path = Path::new(&attachment.storage_path);
        if path.exists() {
            zip.start_file(&attachment.filename, options)?;
            let content = fs::read(path)?;
            zip.write_all(&content)?;
            debug!("Added {} to ZIP archive", attachment.filename);
        } else {
            warn!("Attachment file not found: {:?}", path);
        }
    }

    zip.finish()?;
    info!("Created ZIP archive at {:?} with {} files", output_path, attachment_count);

    Ok(output_path.to_path_buf())
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
        );

        assert!(path.to_string_lossy().contains("attachments"));
        assert!(path.to_string_lossy().contains("user@example.com"));
        assert!(path.to_string_lossy().contains("invoice.pdf"));
        assert!(!path.to_string_lossy().contains("<")); // Should be sanitized
        assert!(!path.to_string_lossy().contains(">"));
    }
}
