use crate::imap::client::ImapClient;
use crate::imap::error::ImapError;
use crate::imap::session::MockAsyncImapOps;
use crate::imap::types::{Email, Folder, MailboxInfo};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_client_initialization() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let client = ImapClient::new(mock_session.clone());
    assert!(client.session.try_lock().is_ok());
}

#[tokio::test]
async fn test_client_login() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.login("test@example.com", "password").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_logout() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.logout().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_list_folders() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.list_folders().await;
    assert!(result.is_ok());
    let folders = result.unwrap();
    assert_eq!(folders.len(), 0); // Mock implementation returns empty vec
}

#[tokio::test]
async fn test_client_create_folder() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.create_folder("INBOX.Test").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_delete_folder() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.delete_folder("INBOX.Test").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_rename_folder() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.rename_folder("INBOX.Old", "INBOX.New").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_select_folder() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.select_folder("INBOX").await;
    assert!(result.is_ok());
    let mailbox_info = result.unwrap();
    assert_eq!(mailbox_info.exists, 0); // Mock implementation returns empty mailbox
}

#[tokio::test]
async fn test_client_search_emails() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.search_emails("ALL").await;
    assert!(result.is_ok());
    let uids = result.unwrap();
    assert_eq!(uids.len(), 0); // Mock implementation returns empty vec
}

#[tokio::test]
async fn test_client_fetch_emails() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.fetch_emails(&[1, 2, 3]).await;
    assert!(result.is_ok());
    let emails = result.unwrap();
    assert_eq!(emails.len(), 0); // Mock implementation returns empty vec
}

#[tokio::test]
async fn test_client_fetch_raw_message() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.fetch_raw_message(1).await;
    assert!(result.is_ok());
    let message = result.unwrap();
    assert_eq!(message.len(), 0); // Mock implementation returns empty vec
}

#[tokio::test]
async fn test_client_move_email() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.move_email(1, "INBOX.Archive").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_store_flags() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.store_flags(1, "\\Seen").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_append() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let content = b"Test email content";
    let result = client.append("INBOX", content, "\\Seen").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_client_expunge() {
    let mock_session = Arc::new(Mutex::new(MockAsyncImapOps::new()));
    let mut client = ImapClient::new(mock_session.clone());
    let result = client.expunge().await;
    assert!(result.is_ok());
} 