use imap_api_rust::imap::client::{ImapClient, ImapSessionTrait, ZeroCopy};
use imap_api_rust::error::ImapApiError;
use mockall::predicate::*;
use std::sync::Arc;
use std::sync::Mutex;
use crate::mocks::mock::MockImapSession;
use imap_api_rust::imap::client::Error as ImapError;
use crate::mocks::mock::{MockImapSessionWrapper};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_folders() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.list_folders().await;
        assert!(result.is_ok());
        let folders = result.unwrap();
        assert!(folders.inner.contains(&"INBOX".to_string()));
    }

    #[tokio::test]
    async fn test_create_folder() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.create_folder("TestFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_folder() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.delete_folder("TestFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_select_folder() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.select_folder("INBOX").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_search_emails() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.search("ALL").await;
        assert!(result.is_ok());
        let uids = result.unwrap();
        assert_eq!(uids, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_fetch_emails() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.fetch("1:3").await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails.inner[0], "Email content");
    }

    #[tokio::test]
    async fn test_connection_error_list() {
        let mock = MockImapSession::new(true);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.list_folders().await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Bad(msg)) => assert_eq!(msg, "Connection error"),
            _ => panic!("Expected Bad error"),
        }
    }

    #[tokio::test]
    async fn test_search_unread() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.search("UNSEEN").await;
        assert!(result.is_ok());
        let uids = result.unwrap();
        assert_eq!(uids, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_get_single_email_fetch() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.get_email("INBOX", "1").await;
        assert!(result.is_ok());
        let email = result.unwrap();
        assert_eq!(email[0], "Email content");
    }

    #[tokio::test]
    async fn test_invalid_folder_select() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.select_folder("InvalidFolder").await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Bad(msg)) => assert_eq!(msg, "Folder does not exist"),
            _ => panic!("Expected Bad error"),
        }
    }

    #[tokio::test]
    async fn test_invalid_uid_fetch() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.get_email("INBOX", "99999").await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Bad(msg)) => assert_eq!(msg, "Message not found"),
            _ => panic!("Expected Bad error"),
        }
    }

    #[tokio::test]
    async fn test_invalid_move() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.move_messages("99999", "Trash").await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Bad(msg)) => assert_eq!(msg, "Message not found"),
            _ => panic!("Expected Bad error"),
        }
    }

    #[tokio::test]
    async fn test_delete_non_empty_folder() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.delete_folder("NonEmptyFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rename_folder() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.rename_folder("OldFolder", "NewFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_folder_stats() {
        let mock = MockImapSession::new(false);
        let wrapper = MockImapSessionWrapper::new(mock);
        let client = ImapClient::new(Arc::new(wrapper));
        let result = client.get_folder_stats("INBOX").await;
        assert!(result.is_ok());
    }
}