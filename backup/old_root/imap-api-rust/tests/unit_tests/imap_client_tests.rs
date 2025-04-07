use imap_api_rust::imap::client::{ImapClient, ImapClientError, ImapSessionTrait};
use crate::mocks::mock::MockImapSession;
use std::sync::Arc;
use parking_lot::Mutex;

#[tokio::test]
async fn test_list_folders() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.list_folders().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_folder() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.create_folder("TestFolder").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_folder() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.delete_folder("TestFolder").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_search_emails() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.search_emails("INBOX", "ALL").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_fetch_email() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.fetch_email("1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_move_email() {
    let mock_session = MockImapSession::new();
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.move_email("1", "Archive").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_folder_not_found() {
    let mut mock_session = MockImapSession::new();
    mock_session.select_result = Err(ImapClientError::FolderError("Folder does not exist".to_string()));
    let client = ImapClient::new(Arc::new(Mutex::new(mock_session)));
    let result = client.search_emails("NonExistentFolder", "ALL").await;
    assert!(result.is_err());
    match result {
        Err(ImapClientError::FolderError(_)) => (),
        _ => panic!("Expected FolderError"),
    }
} 