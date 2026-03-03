// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Integration tests for the three new MCP tools:
//! - export_folder_metadata
//! - filter_emails_by_subject
//! - batch_get_synopsis
//!
//! These tests create a real SQLite database with test data and exercise
//! the tool logic directly (not through HTTP).

use chrono::Utc;
use serial_test::serial;
use sqlx::SqlitePool;
use std::fs;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

async fn create_test_pool(test_name: &str) -> SqlitePool {
    let db_path = format!("test_data/new_tools_{}_test.db", test_name);
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(format!("{}-shm", db_path));
    let _ = fs::remove_file(format!("{}-wal", db_path));
    fs::create_dir_all("test_data").unwrap();
    fs::File::create(&db_path).unwrap();

    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

fn cleanup_test_db(test_name: &str) {
    let db_path = format!("test_data/new_tools_{}_test.db", test_name);
    let _ = fs::remove_file(&db_path);
    let _ = fs::remove_file(format!("{}-shm", db_path));
    let _ = fs::remove_file(format!("{}-wal", db_path));
}

/// Insert a test account, folder, and emails into the database.
/// Returns the folder_id.
async fn seed_test_data(pool: &SqlitePool, account_id: &str, folder_name: &str) -> i64 {
    // Insert account
    sqlx::query(
        "INSERT INTO accounts (email_address, display_name, imap_host, imap_port, imap_user, imap_pass) \
         VALUES (?, ?, 'test.imap.com', 993, ?, 'testpass')"
    )
    .bind(account_id)
    .bind(format!("Test {}", account_id))
    .bind(account_id)
    .execute(pool)
    .await
    .unwrap();

    // Insert folder
    sqlx::query("INSERT INTO folders (account_id, name) VALUES (?, ?)")
        .bind(account_id)
        .bind(folder_name)
        .execute(pool)
        .await
        .unwrap();

    let (folder_id,): (i64,) = sqlx::query_as(
        "SELECT id FROM folders WHERE account_id = ? AND name = ?"
    )
    .bind(account_id)
    .bind(folder_name)
    .fetch_one(pool)
    .await
    .unwrap();

    // Insert test emails with varying subjects
    let emails = vec![
        (1, "FW: Resume for John Doe", "mason@example.com", "aaron@client.com",
         "2024-03-14T10:00:00Z", true, "Forwarded candidate resume. John has 10 years experience in histology."),
        (2, "MLee Candidate Submittal - Jane Smith", "mason@example.com", "aaron@client.com",
         "2024-03-13T14:00:00Z", true, "Candidate submittal for Jane Smith, Lab Tech with 5 years experience."),
        (3, "Meeting notes from Thursday", "bob@example.com", "team@example.com",
         "2024-03-12T09:00:00Z", false, "Notes from the weekly standup meeting. Action items discussed."),
        (4, "RE: Invoice #12345", "billing@vendor.com", "mason@example.com",
         "2024-03-11T16:00:00Z", true, "Please find attached the updated invoice for March services."),
        (5, "Candidate Resume - Alex Johnson", "hr@example.com", "mason@example.com",
         "2024-03-10T11:00:00Z", true, "Alex Johnson resume attached. Strong background in medical lab work."),
    ];

    for (uid, subject, from_addr, to_addr, date, has_attach, body) in &emails {
        sqlx::query(
            "INSERT INTO emails (folder_id, uid, subject, from_address, to_addresses, date, has_attachments, body_text, message_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(folder_id)
        .bind(*uid as i64)
        .bind(*subject)
        .bind(*from_addr)
        .bind(*to_addr)
        .bind(*date)
        .bind(*has_attach)
        .bind(*body)
        .bind(format!("<msg-{}@test.com>", uid))
        .execute(pool)
        .await
        .unwrap();
    }

    folder_id
}

// ---------------------------------------------------------------------------
// export_folder_metadata tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_metadata_export_json() {
    let pool = create_test_pool("meta_json").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let exporter = rustymail::metadata_export::MetadataExporter::new(pool.clone());
    let result = exporter
        .export("test@example.com", "Sent Items", None, None, None)
        .await
        .unwrap();

    assert_eq!(result.email_count, 5);
    assert_eq!(result.format, "json");
    assert!(std::path::Path::new(&result.file_path).exists());

    // Verify file content is valid JSON with 5 items
    let content = fs::read_to_string(&result.file_path).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.len(), 5);

    // Cleanup
    let _ = fs::remove_file(&result.file_path);
    cleanup_test_db("meta_json");
}

#[tokio::test]
#[serial]
async fn test_metadata_export_csv_filtered_fields() {
    let pool = create_test_pool("meta_csv").await;
    seed_test_data(&pool, "test@example.com", "INBOX").await;

    let exporter = rustymail::metadata_export::MetadataExporter::new(pool.clone());
    let result = exporter
        .export("test@example.com", "INBOX", Some("csv"), Some("uid,subject"), None)
        .await
        .unwrap();

    assert_eq!(result.email_count, 5);
    assert_eq!(result.format, "csv");

    let content = fs::read_to_string(&result.file_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "uid,subject"); // header
    assert_eq!(lines.len(), 6); // header + 5 data rows

    let _ = fs::remove_file(&result.file_path);
    cleanup_test_db("meta_csv");
}

#[tokio::test]
#[serial]
async fn test_metadata_export_folder_not_found() {
    let pool = create_test_pool("meta_err").await;
    seed_test_data(&pool, "test@example.com", "INBOX").await;

    let exporter = rustymail::metadata_export::MetadataExporter::new(pool.clone());
    let result = exporter
        .export("test@example.com", "NonexistentFolder", None, None, None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    cleanup_test_db("meta_err");
}

// ---------------------------------------------------------------------------
// filter_emails_by_subject tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_filter_any_mode() {
    let pool = create_test_pool("filter_any").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    let patterns = vec!["resume".to_string(), "candidate".to_string()];
    let result = filter
        .filter("test@example.com", "Sent Items", &patterns, None, None, None, None, None, None)
        .await
        .unwrap();

    // Emails 1, 2, 5 match ("resume" or "candidate")
    assert_eq!(result.total_matched, 3);
    assert_eq!(result.match_mode, "any");

    // Verify matched_patterns are correct
    for email in &result.results {
        assert!(
            !email.matched_patterns.is_empty(),
            "UID {} should have matched patterns", email.uid
        );
    }

    cleanup_test_db("filter_any");
}

#[tokio::test]
#[serial]
async fn test_filter_all_mode() {
    let pool = create_test_pool("filter_all").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    // Only email 5 has both "candidate" AND "resume" in subject
    let patterns = vec!["candidate".to_string(), "resume".to_string()];
    let result = filter
        .filter(
            "test@example.com", "Sent Items", &patterns, Some("all"),
            None, None, None, None, None,
        )
        .await
        .unwrap();

    assert_eq!(result.total_matched, 1);
    assert_eq!(result.match_mode, "all");
    assert_eq!(result.results[0].uid, 5); // "Candidate Resume - Alex Johnson"

    cleanup_test_db("filter_all");
}

#[tokio::test]
#[serial]
async fn test_filter_sender_filter() {
    let pool = create_test_pool("filter_sender").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    // Match "resume" or "candidate" but only from mason@
    let patterns = vec!["resume".to_string(), "candidate".to_string()];
    let result = filter
        .filter(
            "test@example.com", "Sent Items", &patterns, None,
            Some("mason@"), None, None, None, None,
        )
        .await
        .unwrap();

    // Only emails 1 and 2 are from mason@ with matching subjects
    assert_eq!(result.total_matched, 2);
    for email in &result.results {
        assert!(email.from_address.as_ref().unwrap().contains("mason@"));
    }

    cleanup_test_db("filter_sender");
}

#[tokio::test]
#[serial]
async fn test_filter_max_results() {
    let pool = create_test_pool("filter_limit").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    // All 5 emails contain something, limit to 2
    let patterns = vec!["e".to_string()]; // matches all subjects
    let result = filter
        .filter(
            "test@example.com", "Sent Items", &patterns, None,
            None, None, None, None, Some(2),
        )
        .await
        .unwrap();

    assert_eq!(result.total_matched, 2);

    cleanup_test_db("filter_limit");
}

#[tokio::test]
#[serial]
async fn test_filter_empty_patterns_error() {
    let pool = create_test_pool("filter_empty").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    let result = filter
        .filter("test@example.com", "Sent Items", &[], None, None, None, None, None, None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("At least one subject pattern"));

    cleanup_test_db("filter_empty");
}

#[tokio::test]
#[serial]
async fn test_filter_folder_not_found() {
    let pool = create_test_pool("filter_nofolder").await;
    seed_test_data(&pool, "test@example.com", "INBOX").await;

    let filter = rustymail::filter_emails::SubjectFilter::new(pool.clone());
    let patterns = vec!["test".to_string()];
    let result = filter
        .filter("test@example.com", "NonExistent", &patterns, None, None, None, None, None, None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    cleanup_test_db("filter_nofolder");
}

// ---------------------------------------------------------------------------
// batch_get_synopsis tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_batch_synopsis_basic() {
    let pool = create_test_pool("synopsis_basic").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    let uids = vec![1, 2, 3];
    let result = processor
        .process("test@example.com", "Sent Items", &uids, None)
        .await
        .unwrap();

    assert_eq!(result.requested, 3);
    assert_eq!(result.returned, 3);
    assert!(result.errors.is_empty());

    // Verify synopses are non-empty and within default char limit
    for synopsis in &result.synopses {
        assert!(!synopsis.synopsis.is_empty());
        assert!(synopsis.synopsis.len() <= 303); // 300 + "..."
    }

    // Verify order matches input UID order
    assert_eq!(result.synopses[0].uid, 1);
    assert_eq!(result.synopses[1].uid, 2);
    assert_eq!(result.synopses[2].uid, 3);

    cleanup_test_db("synopsis_basic");
}

#[tokio::test]
#[serial]
async fn test_batch_synopsis_missing_uids() {
    let pool = create_test_pool("synopsis_missing").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    // UIDs 1 and 3 exist, 999 does not
    let uids = vec![1, 999, 3];
    let result = processor
        .process("test@example.com", "Sent Items", &uids, None)
        .await
        .unwrap();

    assert_eq!(result.requested, 3);
    assert_eq!(result.returned, 2);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].uid, 999);
    assert!(result.errors[0].reason.contains("not found"));

    // Found UIDs should maintain order
    assert_eq!(result.synopses[0].uid, 1);
    assert_eq!(result.synopses[1].uid, 3);

    cleanup_test_db("synopsis_missing");
}

#[tokio::test]
#[serial]
async fn test_batch_synopsis_max_chars() {
    let pool = create_test_pool("synopsis_chars").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    let uids = vec![1];
    let result = processor
        .process("test@example.com", "Sent Items", &uids, Some(50))
        .await
        .unwrap();

    assert_eq!(result.returned, 1);
    // Synopsis should be truncated to ~50 chars
    assert!(result.synopses[0].synopsis.len() <= 53); // 50 + "..."

    cleanup_test_db("synopsis_chars");
}

#[tokio::test]
#[serial]
async fn test_batch_synopsis_too_many_uids() {
    let pool = create_test_pool("synopsis_limit").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    let uids: Vec<i64> = (1..=51).collect(); // 51 exceeds max 50
    let result = processor
        .process("test@example.com", "Sent Items", &uids, None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Maximum 50"));

    cleanup_test_db("synopsis_limit");
}

#[tokio::test]
#[serial]
async fn test_batch_synopsis_empty_uids_error() {
    let pool = create_test_pool("synopsis_empty").await;
    seed_test_data(&pool, "test@example.com", "Sent Items").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    let result = processor
        .process("test@example.com", "Sent Items", &[], None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("At least one UID"));

    cleanup_test_db("synopsis_empty");
}

#[tokio::test]
#[serial]
async fn test_batch_synopsis_folder_not_found() {
    let pool = create_test_pool("synopsis_nofolder").await;
    seed_test_data(&pool, "test@example.com", "INBOX").await;

    let processor = rustymail::batch_synopsis::BatchSynopsisProcessor::new(pool.clone());
    let uids = vec![1];
    let result = processor
        .process("test@example.com", "NonExistent", &uids, None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    cleanup_test_db("synopsis_nofolder");
}
