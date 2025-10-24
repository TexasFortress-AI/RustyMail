// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustymail::dashboard::services::{
    account::{Account, AccountService},
    smtp::{SendEmailRequest, SendEmailResponse, SmtpError, SmtpService},
};
use rustymail::prelude::CloneableImapSessionFactory;
use serial_test::serial;
use sqlx::SqlitePool;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

// Helper function to create test database pool
async fn create_test_db_pool(test_name: &str) -> SqlitePool {
    let db_file_path = format!("test_data/smtp_{}_test.db", test_name);

    // Remove existing database files
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));

    // Create test_data directory if it doesn't exist
    fs::create_dir_all("test_data").unwrap();

    // Create the database file
    fs::File::create(&db_file_path).unwrap();

    // Connect to database
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
    let db_file_path = format!("test_data/smtp_{}_test.db", test_name);
    let _ = fs::remove_file(&db_file_path);
    let _ = fs::remove_file(format!("{}-shm", db_file_path));
    let _ = fs::remove_file(format!("{}-wal", db_file_path));
}

// Helper to create a test account with SMTP config
fn create_test_account_with_smtp(email: &str) -> Account {
    Account {
        email_address: email.to_string(),
        id: email.to_string(),
        display_name: "Test User".to_string(),
        provider_type: Some("test".to_string()),
        imap_host: "imap.test.com".to_string(),
        imap_port: 993,
        imap_user: email.to_string(),
        imap_pass: "test_password".to_string(),
        imap_use_tls: true,
        smtp_host: Some("smtp.test.com".to_string()),
        smtp_port: Some(587),
        smtp_user: Some(email.to_string()),
        smtp_pass: Some("test_password".to_string()),
        smtp_use_tls: Some(true),
        smtp_use_starttls: Some(true),
        is_active: true,
        is_default: true,
        connection_status: None,
    }
}

// Helper to create a test account WITHOUT SMTP config
fn create_test_account_without_smtp(email: &str) -> Account {
    Account {
        email_address: email.to_string(),
        id: email.to_string(),
        display_name: "Test User".to_string(),
        provider_type: Some("test".to_string()),
        imap_host: "imap.test.com".to_string(),
        imap_port: 993,
        imap_user: email.to_string(),
        imap_pass: "test_password".to_string(),
        imap_use_tls: true,
        smtp_host: None,
        smtp_port: None,
        smtp_user: None,
        smtp_pass: None,
        smtp_use_tls: None,
        smtp_use_starttls: None,
        is_active: true,
        is_default: false,
        connection_status: None,
    }
}

// Helper to create basic SendEmailRequest
fn create_test_email_request() -> SendEmailRequest {
    SendEmailRequest {
        to: vec!["recipient@test.com".to_string()],
        cc: None,
        bcc: None,
        subject: "Test Subject".to_string(),
        body: "Test email body".to_string(),
        body_html: None,
    }
}

#[tokio::test]
#[serial]
#[ignore] // Requires mock SMTP server
async fn test_account_not_found_error() {
    let test_name = "account_not_found";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut account_service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    account_service.initialize(pool).await.unwrap();

    // Create SMTP service without creating any accounts
    let account_service_arc = Arc::new(TokioMutex::new(account_service));

    // Create a mock IMAP session factory
    // Note: We'll need to implement a mock factory for testing
    // For now, this test is marked as #[ignore]

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
#[ignore] // Requires mock SMTP server
async fn test_missing_smtp_credentials_error() {
    let test_name = "missing_smtp_credentials";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut account_service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    account_service.initialize(pool).await.unwrap();

    // Create account WITHOUT SMTP configuration
    let account = create_test_account_without_smtp("test@test.com");
    account_service.create_account(account).await.unwrap();

    let account_service_arc = Arc::new(TokioMutex::new(account_service));

    // TODO: Create SmtpService and test sending email
    // Should fail with SmtpError::MissingCredentials

    cleanup_test_db(test_name);
}

#[tokio::test]
#[serial]
async fn test_send_email_request_validation() {
    // Test that SendEmailRequest can be created with valid data
    let request = create_test_email_request();
    assert_eq!(request.to.len(), 1);
    assert_eq!(request.to[0], "recipient@test.com");
    assert_eq!(request.subject, "Test Subject");
    assert_eq!(request.body, "Test email body");
    assert!(request.cc.is_none());
    assert!(request.bcc.is_none());
    assert!(request.body_html.is_none());
}

#[tokio::test]
#[serial]
async fn test_send_email_request_with_cc_bcc() {
    // Test SendEmailRequest with CC and BCC recipients
    let request = SendEmailRequest {
        to: vec!["recipient@test.com".to_string()],
        cc: Some(vec!["cc@test.com".to_string()]),
        bcc: Some(vec!["bcc@test.com".to_string()]),
        subject: "Test Subject".to_string(),
        body: "Test email body".to_string(),
        body_html: None,
    };

    assert!(request.cc.is_some());
    assert_eq!(request.cc.as_ref().unwrap().len(), 1);
    assert_eq!(request.cc.as_ref().unwrap()[0], "cc@test.com");

    assert!(request.bcc.is_some());
    assert_eq!(request.bcc.as_ref().unwrap().len(), 1);
    assert_eq!(request.bcc.as_ref().unwrap()[0], "bcc@test.com");
}

#[tokio::test]
#[serial]
async fn test_send_email_request_with_html_body() {
    // Test SendEmailRequest with HTML body
    let request = SendEmailRequest {
        to: vec!["recipient@test.com".to_string()],
        cc: None,
        bcc: None,
        subject: "Test Subject".to_string(),
        body: "Plain text body".to_string(),
        body_html: Some("<p>HTML body</p>".to_string()),
    };

    assert!(request.body_html.is_some());
    assert_eq!(request.body_html.as_ref().unwrap(), "<p>HTML body</p>");
}

#[tokio::test]
#[serial]
async fn test_smtp_error_types() {
    // Test that all SmtpError variants can be created
    use rustymail::dashboard::services::smtp::SmtpError;

    let config_err = SmtpError::ConfigError("test".to_string());
    assert!(matches!(config_err, SmtpError::ConfigError(_)));

    let account_err = SmtpError::AccountNotFound("test@test.com".to_string());
    assert!(matches!(account_err, SmtpError::AccountNotFound(_)));

    let creds_err = SmtpError::MissingCredentials("test@test.com".to_string());
    assert!(matches!(creds_err, SmtpError::MissingCredentials(_)));
}

#[tokio::test]
#[serial]
async fn test_account_with_smtp_config_creation() {
    let test_name = "smtp_account_creation";
    cleanup_test_db(test_name);

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("accounts.json");

    let mut account_service = AccountService::new(config_path.to_str().unwrap());
    let pool = create_test_db_pool(test_name).await;
    account_service.initialize(pool).await.unwrap();

    // Create account with SMTP configuration
    let account = create_test_account_with_smtp("test@test.com");
    account_service.create_account(account).await.unwrap();

    // Verify SMTP config was saved correctly
    let retrieved = account_service.get_account("test@test.com").await.unwrap();
    assert!(retrieved.smtp_host.is_some());
    assert_eq!(retrieved.smtp_host.as_ref().unwrap(), "smtp.test.com");
    assert_eq!(retrieved.smtp_port, Some(587));
    assert_eq!(retrieved.smtp_user.as_ref().unwrap(), "test@test.com");
    assert_eq!(retrieved.smtp_use_tls, Some(true));
    assert_eq!(retrieved.smtp_use_starttls, Some(true));

    cleanup_test_db(test_name);
}

// TODO: Add these tests once we have mock SMTP server and IMAP session factory:
// - test_send_email_success
// - test_send_email_smtp_connection_error
// - test_send_email_smtp_auth_error
// - test_send_email_invalid_recipient_error
// - test_send_email_timeout_handling
// - test_outbox_pattern_success
// - test_outbox_cleanup_on_success
// - test_sent_folder_storage
// - test_send_email_with_special_characters_in_display_name
// - test_test_smtp_connection_success
// - test_test_smtp_connection_failure

#[test]
fn test_smtp_service_tests_exist() {
    // This is a placeholder test to ensure the file compiles
    // Real tests will be added once mock infrastructure is in place
    assert!(true, "SMTP service test file exists and compiles");
}
