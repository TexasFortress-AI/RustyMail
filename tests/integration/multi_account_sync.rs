// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Integration tests for multi-account email synchronization
//!
//! This test suite verifies that the email caching system properly isolates data
//! between multiple accounts, using email addresses as account identifiers throughout.

#[cfg(test)]
mod multi_account_sync_tests {
    use rustymail::dashboard::services::cache::{CacheService, CacheConfig};
    use rustymail::dashboard::services::account::AccountService;
    use rustymail::imap::types::{Email, Address, Envelope};
    use serial_test::serial;
    use sqlx::SqlitePool;
    use chrono::Utc;

    const TEST_DB_PATH: &str = "sqlite:file::memory:?cache=shared";
    const ACCOUNT1_EMAIL: &str = "chris@texasfortress.ai";
    const ACCOUNT2_EMAIL: &str = "shannon@texasfortress.ai";

    /// Set up a test database with the schema and two test accounts
    async fn setup_test_database() -> (SqlitePool, CacheService, AccountService) {
        // Create in-memory database
        let pool = SqlitePool::connect(TEST_DB_PATH)
            .await
            .expect("Failed to create test database pool");

        // Run migrations to create schema
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // Insert test account records (required for foreign key constraints)
        // Use INSERT OR REPLACE to handle shared in-memory database across serial tests
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO accounts (email_address, display_name, imap_host, imap_port, imap_user, imap_pass)
            VALUES
                (?, 'Test Account 1', 'test.imap.com', 993, ?, 'test_pass_1'),
                (?, 'Test Account 2', 'test.imap.com', 993, ?, 'test_pass_2')
            "#
        )
        .bind(ACCOUNT1_EMAIL)
        .bind(ACCOUNT1_EMAIL)
        .bind(ACCOUNT2_EMAIL)
        .bind(ACCOUNT2_EMAIL)
        .execute(&pool)
        .await
        .expect("Failed to insert test accounts");

        // Create cache service
        let cache_config = CacheConfig {
            database_url: TEST_DB_PATH.to_string(),
            max_memory_items: 100,
            max_cache_size_mb: 100,
            max_email_age_days: 30,
            sync_interval_seconds: 300,
        };

        let mut cache_service = CacheService::new(cache_config);
        cache_service.initialize().await.expect("Failed to initialize cache service");

        // Create account service
        let account_service = AccountService::new("config/accounts.json");

        (pool, cache_service, account_service)
    }

    /// Create mock email data for testing
    fn create_mock_email(uid: u32, subject: &str, from: &str) -> Email {
        Email {
            uid,
            flags: vec![],
            internal_date: Some(Utc::now()),
            envelope: Some(Envelope {
                subject: Some(subject.to_string()),
                from: vec![Address {
                    name: Some(from.to_string()),
                    mailbox: Some(from.to_string()),
                    host: Some("test.com".to_string()),
                }],
                to: vec![],
                cc: vec![],
                bcc: vec![],
                reply_to: vec![],
                date: None,
                in_reply_to: None,
                message_id: Some(format!("<{}@test.com>", uid)),
            }),
            body: Some(format!("This is test email {}", uid).into_bytes()),
            mime_parts: vec![],
            text_body: Some(format!("This is test email {}", uid)),
            html_body: None,
            attachments: vec![],
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_account_folder_relationships() {
        println!("=== Testing Account-Folder Relationships ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders for account 1
        let folder1_acc1 = cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        let folder2_acc1 = cache_service
            .get_or_create_folder_for_account("Sent", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        // Create folders for account 2
        let folder1_acc2 = cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        let folder2_acc2 = cache_service
            .get_or_create_folder_for_account("Sent", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        // Verify folders have different IDs even with same names
        assert_ne!(folder1_acc1.id, folder1_acc2.id, "INBOX folders should have different IDs for different accounts");
        assert_ne!(folder2_acc1.id, folder2_acc2.id, "Sent folders should have different IDs for different accounts");

        // Verify folder names are correct
        assert_eq!(folder1_acc1.name, "INBOX");
        assert_eq!(folder1_acc2.name, "INBOX");
        assert_eq!(folder2_acc1.name, "Sent");
        assert_eq!(folder2_acc2.name, "Sent");

        println!("✓ Folders properly isolated by account_id");
        println!("  Account 1 INBOX: folder_id={}", folder1_acc1.id);
        println!("  Account 2 INBOX: folder_id={}", folder1_acc2.id);
    }

    #[tokio::test]
    #[serial]
    async fn test_email_caching_isolation() {
        println!("=== Testing Email Caching Isolation ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders for both accounts
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        // Cache emails for account 1
        for i in 1..=5 {
            let email = create_mock_email(i, &format!("Account 1 Email {}", i), "user1@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT1_EMAIL)
                .await
                .expect(&format!("Failed to cache email {} for account 1", i));
        }

        // Cache emails for account 2
        for i in 1..=3 {
            let email = create_mock_email(i, &format!("Account 2 Email {}", i), "user2@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT2_EMAIL)
                .await
                .expect(&format!("Failed to cache email {} for account 2", i));
        }

        // Retrieve emails for account 1
        let emails_acc1 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT1_EMAIL, 100, 0, false)
            .await
            .expect("Failed to get emails for account 1");

        // Retrieve emails for account 2
        let emails_acc2 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT2_EMAIL, 100, 0, false)
            .await
            .expect("Failed to get emails for account 2");

        // Verify correct counts
        assert_eq!(emails_acc1.len(), 5, "Account 1 should have 5 emails");
        assert_eq!(emails_acc2.len(), 3, "Account 2 should have 3 emails");

        // Verify email subjects match expected accounts
        for email in &emails_acc1 {
            assert!(email.subject.as_ref().unwrap().contains("Account 1"),
                   "Account 1 emails should not contain Account 2 data");
        }

        for email in &emails_acc2 {
            assert!(email.subject.as_ref().unwrap().contains("Account 2"),
                   "Account 2 emails should not contain Account 1 data");
        }

        println!("✓ Email data properly isolated between accounts");
        println!("  Account 1: {} emails", emails_acc1.len());
        println!("  Account 2: {} emails", emails_acc2.len());
    }

    #[tokio::test]
    #[serial]
    async fn test_pagination_per_account() {
        println!("=== Testing Pagination Per Account ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folder for account 1
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder");

        // Cache 20 emails for account 1
        for i in 1..=20 {
            let email = create_mock_email(i, &format!("Test Email {}", i), "sender@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT1_EMAIL)
                .await
                .expect(&format!("Failed to cache email {}", i));
        }

        // Test pagination: first page (0-9)
        let page1 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT1_EMAIL, 10, 0, false)
            .await
            .expect("Failed to get page 1");

        assert_eq!(page1.len(), 10, "First page should have 10 emails");

        // Test pagination: second page (10-19)
        let page2 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT1_EMAIL, 10, 10, false)
            .await
            .expect("Failed to get page 2");

        assert_eq!(page2.len(), 10, "Second page should have 10 emails");

        // Verify no overlap between pages
        let page1_uids: Vec<u32> = page1.iter().map(|e| e.uid).collect();
        let page2_uids: Vec<u32> = page2.iter().map(|e| e.uid).collect();

        for uid in &page1_uids {
            assert!(!page2_uids.contains(uid), "Pages should not overlap");
        }

        println!("✓ Pagination works correctly per account");
        println!("  Page 1 UIDs: {:?}", &page1_uids[..std::cmp::min(5, page1_uids.len())]);
        println!("  Page 2 UIDs: {:?}", &page2_uids[..std::cmp::min(5, page2_uids.len())]);
    }

    #[tokio::test]
    #[serial]
    async fn test_cross_account_access_prevention() {
        println!("=== Testing Cross-Account Access Prevention ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders for both accounts
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        // Cache email for account 1
        let email = create_mock_email(1, "Secret Account 1 Email", "secret@test.com");
        cache_service
            .cache_email("INBOX", &email, ACCOUNT1_EMAIL)
            .await
            .expect("Failed to cache email");

        // Try to retrieve account 1's emails using account 2's ID
        let emails_acc2 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT2_EMAIL, 100, 0, false)
            .await
            .expect("Failed to get emails for account 2");

        // Verify account 2 cannot see account 1's emails
        assert_eq!(emails_acc2.len(), 0, "Account 2 should not see Account 1's emails");

        // Verify account 1 can still see its own email
        let emails_acc1 = cache_service
            .get_cached_emails_for_account("INBOX", ACCOUNT1_EMAIL, 100, 0, false)
            .await
            .expect("Failed to get emails for account 1");

        assert_eq!(emails_acc1.len(), 1, "Account 1 should see its own email");

        println!("✓ Cross-account access properly prevented");
        println!("  Account 1 can see: {} emails", emails_acc1.len());
        println!("  Account 2 can see: {} emails (should be 0)", emails_acc2.len());
    }

    #[tokio::test]
    #[serial]
    async fn test_email_address_based_identification() {
        println!("=== Testing Email Address-Based Identification ===");

        let (pool, cache_service, _account_service) = setup_test_database().await;

        // Test that email addresses work as account identifiers
        let result1 = cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await;

        let result2 = cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await;

        assert!(result1.is_ok(), "Should accept email address as account_id");
        assert!(result2.is_ok(), "Should accept email address as account_id");

        // Test with non-existent account - need to insert it first for foreign key constraint
        const TEST_EMAIL: &str = "nonexistent@example.com";
        sqlx::query(
            "INSERT INTO accounts (email_address, display_name, imap_host, imap_port, imap_user, imap_pass) VALUES (?, 'Test', 'test.com', 993, ?, 'pass')"
        )
        .bind(TEST_EMAIL)
        .bind(TEST_EMAIL)
        .execute(&pool)
        .await
        .expect("Failed to insert test account");

        let result3 = cache_service
            .get_or_create_folder_for_account("INBOX", TEST_EMAIL)
            .await;

        assert!(result3.is_ok(), "Should handle additional account gracefully");

        println!("✓ Email addresses work correctly as account identifiers");
    }

    #[tokio::test]
    #[serial]
    async fn test_count_emails_per_account() {
        println!("=== Testing Email Counts Per Account ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        // Cache different numbers of emails for each account
        for i in 1..=7 {
            let email = create_mock_email(i, &format!("Email {}", i), "user@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT1_EMAIL)
                .await
                .expect("Failed to cache email for account 1");
        }

        for i in 1..=4 {
            let email = create_mock_email(i, &format!("Email {}", i), "user@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT2_EMAIL)
                .await
                .expect("Failed to cache email for account 2");
        }

        // Verify counts
        let count1 = cache_service
            .count_emails_in_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to count emails for account 1");

        let count2 = cache_service
            .count_emails_in_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to count emails for account 2");

        assert_eq!(count1, 7, "Account 1 should have 7 emails");
        assert_eq!(count2, 4, "Account 2 should have 4 emails");

        println!("✓ Email counts properly isolated per account");
        println!("  Account 1 count: {}", count1);
        println!("  Account 2 count: {}", count2);
    }

    #[tokio::test]
    #[serial]
    async fn test_folder_stats_per_account() {
        println!("=== Testing Folder Stats Per Account ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders and cache emails
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        for i in 1..=10 {
            let email = create_mock_email(i, &format!("Email {}", i), "user@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT1_EMAIL)
                .await
                .expect("Failed to cache email for account 1");
        }

        for i in 1..=5 {
            let email = create_mock_email(i, &format!("Email {}", i), "user@test.com");
            cache_service
                .cache_email("INBOX", &email, ACCOUNT2_EMAIL)
                .await
                .expect("Failed to cache email for account 2");
        }

        // Get stats for each account
        let stats1 = cache_service
            .get_folder_stats_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to get stats for account 1");

        let stats2 = cache_service
            .get_folder_stats_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to get stats for account 2");

        // Verify stats are different and correct
        let total1 = stats1.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
        let total2 = stats2.get("total").and_then(|v| v.as_i64()).unwrap_or(0);

        assert_eq!(total1, 10, "Account 1 stats should show 10 emails");
        assert_eq!(total2, 5, "Account 2 stats should show 5 emails");

        println!("✓ Folder stats properly isolated per account");
        println!("  Account 1 stats: {:?}", stats1);
        println!("  Account 2 stats: {:?}", stats2);
    }

    #[tokio::test]
    #[serial]
    async fn test_search_emails_per_account() {
        println!("=== Testing Search Emails Per Account ===");

        let (_pool, cache_service, _account_service) = setup_test_database().await;

        // Create folders
        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT1_EMAIL)
            .await
            .expect("Failed to create folder for account 1");

        cache_service
            .get_or_create_folder_for_account("INBOX", ACCOUNT2_EMAIL)
            .await
            .expect("Failed to create folder for account 2");

        // Cache emails with different search terms
        let email1 = create_mock_email(1, "Important meeting tomorrow", "boss@test.com");
        cache_service
            .cache_email("INBOX", &email1, ACCOUNT1_EMAIL)
            .await
            .expect("Failed to cache email");

        let email2 = create_mock_email(2, "Lunch plans", "friend@test.com");
        cache_service
            .cache_email("INBOX", &email2, ACCOUNT1_EMAIL)
            .await
            .expect("Failed to cache email");

        let email3 = create_mock_email(1, "Important project update", "colleague@test.com");
        cache_service
            .cache_email("INBOX", &email3, ACCOUNT2_EMAIL)
            .await
            .expect("Failed to cache email");

        // Search for "Important" in account 1
        let results1 = cache_service
            .search_cached_emails_for_account("INBOX", "Important", 100, ACCOUNT1_EMAIL)
            .await
            .expect("Failed to search account 1");

        // Search for "Important" in account 2
        let results2 = cache_service
            .search_cached_emails_for_account("INBOX", "Important", 100, ACCOUNT2_EMAIL)
            .await
            .expect("Failed to search account 2");

        // Verify search results are isolated
        assert_eq!(results1.len(), 1, "Account 1 should find 1 'Important' email");
        assert_eq!(results2.len(), 1, "Account 2 should find 1 'Important' email");

        // Verify the correct emails were found
        assert!(results1[0].subject.as_ref().unwrap().contains("meeting"));
        assert!(results2[0].subject.as_ref().unwrap().contains("project"));

        println!("✓ Search results properly isolated per account");
        println!("  Account 1 found: {:?}", results1[0].subject);
        println!("  Account 2 found: {:?}", results2[0].subject);
    }
}
