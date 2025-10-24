// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustymail::dashboard::services::account::{AccountService, Account, AccountError};
use rustymail::dashboard::services::account_store::{AccountStore, StoredAccount, ImapConfig, SmtpConfig};
use chrono::Utc;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;
use sqlx::SqlitePool;

// Helper function to create test database pool
async fn create_test_db_pool(test_name: &str) -> SqlitePool {
    // Use relative path like cache_service_tests does
    let db_file_path = format!("test_data/account_{}_test.db", test_name);

    // Remove existing database files
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));

    // Create test_data directory if it doesn't exist
    fs::create_dir_all("test_data").unwrap();

    // Create the database file (important! SQLite needs the file to exist)
    fs::File::create(&db_file_path).unwrap();

    // Now connect to it with sqlite: prefix
    let db_url = format!("sqlite:{}", db_file_path);
    let pool = SqlitePool::connect(&db_url).await.unwrap();

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    pool
}

// Helper function to cleanup test database
fn cleanup_test_db(test_name: &str) {
    let db_file_path = format!("test_data/account_{}_test.db", test_name);
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));
}

// Helper to create a test account
fn create_test_account(email: &str, display_name: &str) -> Account {
    Account {
        email_address: email.to_string(),
        id: email.to_string(), // id mirrors email_address
        display_name: display_name.to_string(),
        provider_type: Some("gmail".to_string()),
        imap_host: "imap.gmail.com".to_string(),
        imap_port: 993,
        imap_user: email.to_string(),
        imap_pass: "test_password".to_string(),
        imap_use_tls: true,
        smtp_host: Some("smtp.gmail.com".to_string()),
        smtp_port: Some(587),
        smtp_user: Some(email.to_string()),
        smtp_pass: Some("test_password".to_string()),
        smtp_use_tls: Some(true),
        smtp_use_starttls: Some(true),
        is_active: true,
        is_default: false,
        connection_status: None,
    }
}

#[tokio::test]
#[serial]
async fn test_account_service_initialization() {
    let test_name = "init";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;

    let result = service.initialize(pool).await;
    assert!(result.is_ok(), "Account service initialization should succeed");

    // Verify config file was created
    assert!(config_path.exists(), "Config file should exist");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_creation() {
    let test_name = "create";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    let account = create_test_account("test@gmail.com", "Test Account");

    // Create account
    let account_id = service.create_account(account.clone()).await;
    assert!(account_id.is_ok(), "Account creation should succeed");
    assert_eq!(account_id.unwrap(), "test@gmail.com", "Account ID should be email address");

    // Verify account was created
    let retrieved = service.get_account("test@gmail.com").await.unwrap();
    assert_eq!(retrieved.email_address, "test@gmail.com");
    assert_eq!(retrieved.display_name, "Test Account");
    assert_eq!(retrieved.imap_host, "imap.gmail.com");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_duplicate_prevention() {
    let test_name = "duplicate";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    let account = create_test_account("test@gmail.com", "Test Account");

    // Create first account
    service.create_account(account.clone()).await.unwrap();

    // Try to create duplicate
    let result = service.create_account(account.clone()).await;
    assert!(result.is_err(), "Duplicate account creation should fail");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_list() {
    let test_name = "list";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create multiple accounts
    service.create_account(create_test_account("user1@gmail.com", "User 1")).await.unwrap();
    service.create_account(create_test_account("user2@gmail.com", "User 2")).await.unwrap();
    service.create_account(create_test_account("user3@gmail.com", "User 3")).await.unwrap();

    // List accounts
    let accounts = service.list_accounts().await.unwrap();
    assert_eq!(accounts.len(), 3, "Should have 3 accounts");

    let emails: Vec<String> = accounts.iter().map(|a| a.email_address.clone()).collect();
    assert!(emails.contains(&"user1@gmail.com".to_string()));
    assert!(emails.contains(&"user2@gmail.com".to_string()));
    assert!(emails.contains(&"user3@gmail.com".to_string()));

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_update() {
    let test_name = "update";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create account
    let account = create_test_account("test@gmail.com", "Original Name");
    service.create_account(account.clone()).await.unwrap();

    // Update account
    let mut updated = create_test_account("test@gmail.com", "Updated Name");
    updated.imap_host = "new.imap.gmail.com".to_string();

    let result = service.update_account("test@gmail.com", updated).await;
    assert!(result.is_ok(), "Account update should succeed");

    // Verify update
    let retrieved = service.get_account("test@gmail.com").await.unwrap();
    assert_eq!(retrieved.display_name, "Updated Name");
    assert_eq!(retrieved.imap_host, "new.imap.gmail.com");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_deletion() {
    let test_name = "delete";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create accounts
    service.create_account(create_test_account("user1@gmail.com", "User 1")).await.unwrap();
    service.create_account(create_test_account("user2@gmail.com", "User 2")).await.unwrap();

    // Delete one account
    let result = service.delete_account("user1@gmail.com").await;
    assert!(result.is_ok(), "Account deletion should succeed");

    // Verify deletion
    let accounts = service.list_accounts().await.unwrap();
    assert_eq!(accounts.len(), 1, "Should have 1 account remaining");
    assert_eq!(accounts[0].email_address, "user2@gmail.com");

    // Try to get deleted account
    let result = service.get_account("user1@gmail.com").await;
    assert!(result.is_err(), "Getting deleted account should fail");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_default_account_management() {
    let test_name = "default";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create accounts
    service.create_account(create_test_account("user1@gmail.com", "User 1")).await.unwrap();
    service.create_account(create_test_account("user2@gmail.com", "User 2")).await.unwrap();

    // Initially no default
    let default = service.get_default_account().await.unwrap();
    assert!(default.is_none(), "Initially should have no default account");

    // Set default account
    service.set_default_account("user1@gmail.com").await.unwrap();

    // Verify default is set
    let default = service.get_default_account().await.unwrap();
    assert!(default.is_some());
    assert_eq!(default.unwrap().email_address, "user1@gmail.com");

    // Verify is_default flag in list
    let accounts = service.list_accounts().await.unwrap();
    let user1 = accounts.iter().find(|a| a.email_address == "user1@gmail.com").unwrap();
    let user2 = accounts.iter().find(|a| a.email_address == "user2@gmail.com").unwrap();
    assert!(user1.is_default, "User1 should be marked as default");
    assert!(!user2.is_default, "User2 should not be marked as default");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_default_account_deletion_clears_default() {
    let test_name = "default_delete";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create accounts
    service.create_account(create_test_account("user1@gmail.com", "User 1")).await.unwrap();
    service.create_account(create_test_account("user2@gmail.com", "User 2")).await.unwrap();

    // Set default and delete it
    service.set_default_account("user1@gmail.com").await.unwrap();
    service.delete_account("user1@gmail.com").await.unwrap();

    // Verify default was cleared
    let default = service.get_default_account().await.unwrap();
    assert!(default.is_none(), "Default should be cleared after deleting default account");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
#[ignore] // Requires provider_templates table in database (not yet migrated)
async fn test_auto_configure_gmail() {
    let test_name = "autoconfig_gmail";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Test Gmail auto-configuration
    let result = service.auto_configure("user@gmail.com").await.unwrap();

    assert!(result.provider_found, "Gmail provider should be found");
    assert_eq!(result.provider_type.as_deref(), Some("gmail"));
    assert_eq!(result.imap_host.as_deref(), Some("imap.gmail.com"));
    assert_eq!(result.imap_port, Some(993));
    assert_eq!(result.smtp_host.as_deref(), Some("smtp.gmail.com"));
    assert_eq!(result.smtp_port, Some(587));

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
#[ignore] // Requires provider_templates table in database (not yet migrated)
async fn test_auto_configure_unknown_provider() {
    let test_name = "autoconfig_unknown";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Test unknown provider
    let result = service.auto_configure("user@unknown-provider-xyz.com").await.unwrap();

    assert!(!result.provider_found, "Unknown provider should not be found");
    assert!(result.provider_type.is_none());
    assert!(result.imap_host.is_none());

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_invalid_email_format() {
    let test_name = "invalid_email";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Test invalid email formats
    let result = service.auto_configure("not-an-email").await;
    assert!(result.is_err(), "Invalid email should fail");

    let result = service.auto_configure("@domain.com").await;
    assert!(result.is_err(), "Missing username should fail");

    let result = service.auto_configure("user@").await;
    assert!(result.is_err(), "Missing domain should fail");

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_with_smtp_config() {
    let test_name = "smtp_config";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    let account = create_test_account("test@gmail.com", "Test Account");
    service.create_account(account).await.unwrap();

    // Verify SMTP config was saved
    let retrieved = service.get_account("test@gmail.com").await.unwrap();
    assert_eq!(retrieved.smtp_host.as_deref(), Some("smtp.gmail.com"));
    assert_eq!(retrieved.smtp_port, Some(587));
    assert_eq!(retrieved.smtp_user.as_deref(), Some("test@gmail.com"));
    assert_eq!(retrieved.smtp_use_tls, Some(true));
    assert_eq!(retrieved.smtp_use_starttls, Some(true));

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_account_without_smtp_config() {
    let test_name = "no_smtp";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    // Create account without SMTP config
    let mut account = create_test_account("test@gmail.com", "Test Account");
    account.smtp_host = None;
    account.smtp_port = None;
    account.smtp_user = None;
    account.smtp_pass = None;

    service.create_account(account).await.unwrap();

    // Verify account was created without SMTP
    let retrieved = service.get_account("test@gmail.com").await.unwrap();
    assert!(retrieved.smtp_host.is_none());
    assert!(retrieved.smtp_port.is_none());

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_password_storage_and_file_security() {
    let test_name = "password_security";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    service.initialize(pool).await.unwrap();

    let account = create_test_account("test@gmail.com", "Test Account");
    service.create_account(account).await.unwrap();

    // Verify passwords ARE stored in JSON (required for account persistence)
    // accounts.json is the source of truth, database is ephemeral cache
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("test_password"), "Password must be in accounts.json for persistence");

    // Verify file permissions are restrictive (0600 - owner only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&config_path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        // Check that only owner has read/write, no group or other permissions
        assert_eq!(mode & 0o777, 0o600, "accounts.json should have 0600 permissions (owner read/write only)");
    }

    // Verify we can retrieve the password through the service API
    let retrieved = service.get_account("test@gmail.com").await.unwrap();
    assert_eq!(retrieved.imap_pass, "test_password", "Password should be accessible through API");

    cleanup_test_db(test_name);
}
