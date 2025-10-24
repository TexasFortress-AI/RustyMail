// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustymail::dashboard::services::attachment_storage::{
    self, AttachmentError, AttachmentInfo,
};
use rustymail::imap::types::{Email, Envelope, MimePart, ContentType, ContentDisposition};
use serial_test::serial;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// Helper function to create test database pool
async fn create_test_db_pool(test_name: &str) -> SqlitePool {
    // Get the project root directory (where Cargo.toml is located)
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_data_dir = project_root.join("test_data");

    // Create test_data directory if it doesn't exist
    fs::create_dir_all(&test_data_dir).unwrap();

    let db_file_path = test_data_dir.join(format!("attachment_{}_test.db", test_name));

    // Remove existing database files
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(db_file_path.with_extension("db-shm"));
    let _ = fs::remove_file(db_file_path.with_extension("db-wal"));

    // Create the database file
    fs::File::create(&db_file_path).unwrap();

    // Connect to database
    let db_url = format!("sqlite:{}", db_file_path.display());
    let pool = SqlitePool::connect(&db_url).await.unwrap();

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    // Insert test account to satisfy foreign key constraints
    sqlx::query(
        r#"
        INSERT INTO accounts (
            email_address, display_name, imap_host, imap_port, imap_user, imap_pass,
            imap_use_tls, smtp_host, smtp_port, smtp_user, smtp_pass, smtp_use_tls
        ) VALUES (
            'test@example.com', 'Test Account', 'imap.example.com', 993, 'test@example.com', 'password',
            1, 'smtp.example.com', 587, 'test@example.com', 'password', 1
        )
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

// Helper function to cleanup test database
fn cleanup_test_db(test_name: &str) {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_data_dir = project_root.join("test_data");
    let db_file_path = test_data_dir.join(format!("attachment_{}_test.db", test_name));

    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(db_file_path.with_extension("db-shm"));
    let _ = fs::remove_file(db_file_path.with_extension("db-wal"));
}

// Helper to parse content type from string (e.g., "application/pdf")
fn parse_content_type(mime_type: &str) -> ContentType {
    let parts: Vec<&str> = mime_type.split('/').collect();
    ContentType {
        main_type: parts.get(0).unwrap_or(&"application").to_string(),
        sub_type: parts.get(1).unwrap_or(&"octet-stream").to_string(),
        parameters: HashMap::new(),
    }
}

// Helper to create a test email with attachments
fn create_test_email_with_attachments() -> Email {
    Email {
        uid: 123,
        flags: vec![],
        internal_date: None,
        envelope: Some(Envelope {
            message_id: Some("<test-msg@example.com>".to_string()),
            subject: Some("Test Email".to_string()),
            from: vec![],
            to: vec![],
            cc: vec![],
            bcc: vec![],
            reply_to: vec![],
            date: None,
            in_reply_to: None,
        }),
        body: None,
        mime_parts: vec![
            MimePart {
                content_type: parse_content_type("application/pdf"),
                content_transfer_encoding: Some("base64".to_string()),
                content_disposition: Some(ContentDisposition {
                    disposition_type: "attachment".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("filename".to_string(), "invoice.pdf".to_string());
                        params
                    },
                }),
                content_id: None,
                content_description: None,
                headers: HashMap::new(),
                body: b"PDF content here".to_vec(),
                text_content: None,
                parts: vec![],
            },
            MimePart {
                content_type: parse_content_type("image/png"),
                content_transfer_encoding: Some("base64".to_string()),
                content_disposition: Some(ContentDisposition {
                    disposition_type: "attachment".to_string(),
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("filename".to_string(), "screenshot.png".to_string());
                        params
                    },
                }),
                content_id: None,
                content_description: None,
                headers: HashMap::new(),
                body: b"PNG image data".to_vec(),
                text_content: None,
                parts: vec![],
            },
        ],
        text_body: Some("Email body".to_string()),
        html_body: None,
        attachments: vec![],
    }
}

// Helper to create a MIME part with specific properties
fn create_mime_part(content_type: &str, filename: &str, body: Vec<u8>) -> MimePart {
    MimePart {
        content_type: parse_content_type(content_type),
        content_transfer_encoding: Some("base64".to_string()),
        content_disposition: Some(ContentDisposition {
            disposition_type: "attachment".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("filename".to_string(), filename.to_string());
                params
            },
        }),
        content_id: None,
        content_description: None,
        headers: HashMap::new(),
        body,
        text_content: None,
        parts: vec![],
    }
}

#[tokio::test]
#[serial]
async fn test_sanitize_message_id() {
    // Test removing angle brackets
    assert_eq!(
        attachment_storage::sanitize_message_id("<test@example.com>"),
        "test@example.com"
    );

    // Test replacing invalid filesystem characters
    assert_eq!(
        attachment_storage::sanitize_message_id("<test/id:123*456?.com>"),
        "test_id_123_456_.com"
    );

    // Test normal message ID
    assert_eq!(
        attachment_storage::sanitize_message_id("simple@example.com"),
        "simple@example.com"
    );
}

#[tokio::test]
#[serial]
async fn test_ensure_message_id_with_existing() {
    let email = create_test_email_with_attachments();
    let message_id = attachment_storage::ensure_message_id(&email, "test@example.com");

    assert_eq!(message_id, "<test-msg@example.com>");
}

#[tokio::test]
#[serial]
async fn test_ensure_message_id_without_envelope() {
    let mut email = create_test_email_with_attachments();
    email.envelope = None;

    let message_id = attachment_storage::ensure_message_id(&email, "test@example.com");

    // Should generate a message ID
    assert!(message_id.starts_with("rustymail-"));
    assert!(message_id.contains("test_example.com"));
    assert!(message_id.contains("-123-")); // Contains UID
}

#[tokio::test]
#[serial]
async fn test_get_attachment_path() {
    let path = attachment_storage::get_attachment_path(
        "user@example.com",
        "<msg123@server.com>",
        "invoice.pdf"
    );

    let path_str = path.to_string_lossy();
    assert!(path_str.contains("attachments"));
    assert!(path_str.contains("user@example.com"));
    assert!(path_str.contains("invoice.pdf"));
    // Should be sanitized (no angle brackets)
    assert!(!path_str.contains("<"));
    assert!(!path_str.contains(">"));
}

#[tokio::test]
#[serial]
async fn test_save_and_retrieve_attachment() {
    let test_name = "save_retrieve";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();

    // Change to temp directory for attachment storage
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<test123@example.com>";

    // Create and save an attachment
    let mime_part = create_mime_part("application/pdf", "test.pdf", b"PDF content".to_vec());

    let result = attachment_storage::save_attachment(&pool, account, message_id, &mime_part).await;
    assert!(result.is_ok());

    let attachment_info = result.unwrap();
    assert_eq!(attachment_info.filename, "test.pdf");
    assert_eq!(attachment_info.size_bytes, 11); // "PDF content" length
    assert_eq!(attachment_info.content_type, Some("application/pdf".to_string()));

    // Retrieve the attachment metadata
    let attachments = attachment_storage::get_attachments_metadata(&pool, account, message_id).await;
    assert!(attachments.is_ok());

    let attachments = attachments.unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.pdf");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_save_multiple_attachments() {
    let test_name = "save_multiple";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<multi123@example.com>";

    // Save multiple attachments
    let pdf = create_mime_part("application/pdf", "doc.pdf", b"PDF".to_vec());
    let png = create_mime_part("image/png", "img.png", b"PNG".to_vec());
    let txt = create_mime_part("text/plain", "note.txt", b"TXT".to_vec());

    attachment_storage::save_attachment(&pool, account, message_id, &pdf).await.unwrap();
    attachment_storage::save_attachment(&pool, account, message_id, &png).await.unwrap();
    attachment_storage::save_attachment(&pool, account, message_id, &txt).await.unwrap();

    // Retrieve all attachments
    let attachments = attachment_storage::get_attachments_metadata(&pool, account, message_id)
        .await
        .unwrap();

    assert_eq!(attachments.len(), 3);

    // Verify filenames
    let filenames: Vec<&str> = attachments.iter().map(|a| a.filename.as_str()).collect();
    assert!(filenames.contains(&"doc.pdf"));
    assert!(filenames.contains(&"img.png"));
    assert!(filenames.contains(&"note.txt"));

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_delete_attachments() {
    let test_name = "delete_attachments";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<delete123@example.com>";

    // Save an attachment
    let mime_part = create_mime_part("application/pdf", "delete.pdf", b"DELETE ME".to_vec());
    attachment_storage::save_attachment(&pool, account, message_id, &mime_part).await.unwrap();

    // Verify it was saved
    let attachments = attachment_storage::get_attachments_metadata(&pool, account, message_id)
        .await
        .unwrap();
    assert_eq!(attachments.len(), 1);

    // Verify file exists on filesystem
    let file_path = PathBuf::from(&attachments[0].storage_path);
    assert!(file_path.exists());

    // Delete attachments
    let result = attachment_storage::delete_attachments_for_email(&pool, message_id, account).await;
    assert!(result.is_ok());

    // Verify no attachments in database
    let attachments = attachment_storage::get_attachments_metadata(&pool, account, message_id)
        .await
        .unwrap();
    assert_eq!(attachments.len(), 0);

    // Verify file was deleted from filesystem
    assert!(!file_path.exists());

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_attachment_with_special_characters_in_filename() {
    let test_name = "special_chars";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<special123@example.com>";

    // Try to save attachment with special characters in filename
    let mime_part = create_mime_part(
        "application/pdf",
        "my file (with) special [chars].pdf",
        b"CONTENT".to_vec()
    );

    let result = attachment_storage::save_attachment(&pool, account, message_id, &mime_part).await;
    assert!(result.is_ok());

    // The filename should be preserved as-is
    let attachment_info = result.unwrap();
    assert_eq!(attachment_info.filename, "my file (with) special [chars].pdf");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_attachment_without_filename() {
    let test_name = "no_filename";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<noname123@example.com>";

    // Create MIME part without content disposition (no filename)
    let mime_part = MimePart {
        content_type: parse_content_type("application/pdf"),
        content_transfer_encoding: Some("base64".to_string()),
        content_disposition: None,
        content_id: None,
        content_description: None,
        headers: HashMap::new(),
        body: b"PDF content".to_vec(),
        text_content: None,
        parts: vec![],
    };

    let result = attachment_storage::save_attachment(&pool, account, message_id, &mime_part).await;
    assert!(result.is_ok());

    // Should generate a filename
    let attachment_info = result.unwrap();
    assert!(attachment_info.filename.starts_with("attachment_"));
    assert!(attachment_info.filename.ends_with(".pdf"));

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_create_zip_archive() {
    let test_name = "create_zip";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<zip123@example.com>";

    // Save multiple attachments
    let pdf = create_mime_part("application/pdf", "file1.pdf", b"PDF1".to_vec());
    let txt = create_mime_part("text/plain", "file2.txt", b"TXT2".to_vec());

    attachment_storage::save_attachment(&pool, account, message_id, &pdf).await.unwrap();
    attachment_storage::save_attachment(&pool, account, message_id, &txt).await.unwrap();

    // Create ZIP archive
    let zip_path = temp_dir.path().join("attachments.zip");
    let result = attachment_storage::create_zip_archive(&pool, account, message_id, &zip_path).await;

    assert!(result.is_ok());
    assert!(zip_path.exists());

    // Verify ZIP file is not empty
    let metadata = fs::metadata(&zip_path).unwrap();
    assert!(metadata.len() > 0);

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_create_zip_with_no_attachments() {
    let test_name = "zip_no_attachments";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<empty123@example.com>";

    // Try to create ZIP with no attachments
    let zip_path = temp_dir.path().join("empty.zip");
    let result = attachment_storage::create_zip_archive(&pool, account, message_id, &zip_path).await;

    assert!(result.is_err());
    if let Err(AttachmentError::NotFound(_)) = result {
        // Expected error
    } else {
        panic!("Expected NotFound error");
    }

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_get_attachments_for_nonexistent_email() {
    let test_name = "nonexistent_email";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;

    let account = "test@example.com";
    let message_id = "<nonexistent@example.com>";

    // Try to get attachments for email that doesn't exist
    let result = attachment_storage::get_attachments_metadata(&pool, account, message_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0); // Should return empty list

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_delete_nonexistent_attachments() {
    let test_name = "delete_nonexistent";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;

    let account = "test@example.com";
    let message_id = "<nonexistent@example.com>";

    // Try to delete attachments for email that doesn't exist
    let result = attachment_storage::delete_attachments_for_email(&pool, message_id, account).await;

    // Should succeed (no-op) without error
    assert!(result.is_ok());

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_attachment_content_type_preservation() {
    let test_name = "content_type";
    cleanup_test_db(test_name);

    let pool = create_test_db_pool(test_name).await;
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let account = "test@example.com";
    let message_id = "<types123@example.com>";

    // Test various content types
    let types = vec![
        ("application/pdf", "doc.pdf"),
        ("image/jpeg", "photo.jpg"),
        ("image/png", "screenshot.png"),
        ("text/plain", "notes.txt"),
        ("application/zip", "archive.zip"),
    ];

    for (content_type, filename) in types {
        let mime_part = create_mime_part(content_type, filename, b"DATA".to_vec());
        attachment_storage::save_attachment(&pool, account, message_id, &mime_part).await.unwrap();
    }

    // Retrieve and verify content types
    let attachments = attachment_storage::get_attachments_metadata(&pool, account, message_id)
        .await
        .unwrap();

    assert_eq!(attachments.len(), 5);

    for attachment in &attachments {
        assert!(attachment.content_type.is_some());
        let ct = attachment.content_type.as_ref().unwrap();

        if attachment.filename == "doc.pdf" {
            assert_eq!(ct, "application/pdf");
        } else if attachment.filename == "photo.jpg" {
            assert_eq!(ct, "image/jpeg");
        } else if attachment.filename == "screenshot.png" {
            assert_eq!(ct, "image/png");
        } else if attachment.filename == "notes.txt" {
            assert_eq!(ct, "text/plain");
        } else if attachment.filename == "archive.zip" {
            assert_eq!(ct, "application/zip");
        }
    }

    cleanup_test_db(test_name);
}

#[test]
fn test_attachment_tests_exist() {
    // This is a placeholder test to ensure the file compiles
    assert!(true, "Attachment test file exists and compiles");
}
