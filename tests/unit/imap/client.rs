#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::client::ImapClient;
    use crate::imap::error::ImapError;
    use crate::imap::types::{Email, Folder, FlagOperation};
    use crate::utils::MockImapSession;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Connection Tests
    #[tokio::test]
    async fn test_connection_management() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        // Test session reuse
        let result1 = client.list_folders().await;
        assert!(result1.is_ok());
        
        let result2 = client.list_folders().await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new_with_session(Arc::new(Mutex::new(mock_session))));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.list_folders().await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.search_emails("INBOX", "ALL").await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    // Operation Tests
    #[tokio::test]
    async fn test_folder_operations_sequence() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        // Create folder
        let result = client.create_folder("TestFolder").await;
        assert!(result.is_ok());
        
        // Rename folder
        let result = client.rename_folder("TestFolder", "NewFolder").await;
        assert!(result.is_ok());
        
        // Delete folder
        let result = client.delete_folder("NewFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_email_operations_sequence() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        // Search emails
        let result = client.search_emails("INBOX", "ALL").await;
        assert!(result.is_ok());
        let uids = result.unwrap();
        assert_eq!(uids, vec![1, 2, 3]);
        
        // Fetch emails
        let result = client.fetch_emails(&uids).await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails.len(), 1);
        
        // Store flags
        let result = client.store_flags(1, FlagOperation::Add, "\\Seen").await;
        assert!(result.is_ok());
        
        // Move email
        let result = client.move_email(1, "Archive").await;
        assert!(result.is_ok());
    }

    // Error Handling Tests
    #[tokio::test]
    async fn test_connection_error_handling() {
        let mock_session = MockImapSession::new_failing();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::Connection(_))));
    }

    #[tokio::test]
    async fn test_operation_error_handling() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::OperationError("Test error".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_folder_error_handling() {
        let mock_session = MockImapSession::new()
            .set_select_folder_result(Err(ImapError::FolderError("Folder not found".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.search_emails("NonExistentFolder", "ALL").await;
        assert!(matches!(result, Err(ImapError::FolderError(_))));
    }

    // Edge Cases
    #[tokio::test]
    async fn test_empty_folder_list() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Ok(vec![]));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.list_folders().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_search_results() {
        let mock_session = MockImapSession::new()
            .set_search_emails_result(Ok(vec![]));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.search_emails("INBOX", "ALL").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_fetch_results() {
        let mock_session = MockImapSession::new()
            .set_fetch_emails_result(Ok(vec![]));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.fetch_emails(&[1, 2, 3]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // State Management
    #[tokio::test]
    async fn test_session_state_after_error() {
        let mock_session = MockImapSession::new_failing();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        // First operation fails
        let result1 = client.list_folders().await;
        assert!(result1.is_err());
        
        // Second operation should still work (mock is stateless)
        let result2 = client.list_folders().await;
        assert!(result2.is_err());
    }

    // Timeout Tests
    #[tokio::test]
    async fn test_operation_timeout() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.list_folders()
        ).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    // Append Tests
    #[tokio::test]
    async fn test_append_message() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let message = b"From: test@example.com\r\n\r\nTest message";
        let result = client.append("INBOX", message, &["\\Seen"]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_append_message_error() {
        let mock_session = MockImapSession::new()
            .set_append_result(Err(ImapError::OperationError("Append failed".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let message = b"From: test@example.com\r\n\r\nTest message";
        let result = client.append("INBOX", message, &["\\Seen"]).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_append_large_message() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        // Create a 1MB message
        let mut message = Vec::with_capacity(1024 * 1024);
        message.extend_from_slice(b"From: test@example.com\r\n\r\n");
        message.extend(std::iter::repeat(b'X').take(1024 * 1024 - message.len()));
        
        let result = client.append("INBOX", &message, &["\\Seen"]).await;
        assert!(result.is_ok());
    }

    // Fetch Raw Message Tests
    #[tokio::test]
    async fn test_fetch_raw_message() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fetch_raw_message_error() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Err(ImapError::OperationError("Fetch failed".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.fetch_raw_message(1).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_fetch_raw_message_not_found() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Err(ImapError::EmailNotFound("Message not found".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        
        let result = client.fetch_raw_message(999).await;
        assert!(matches!(result, Err(ImapError::EmailNotFound(_))));
    }

    // TODO: Authentication Tests
    /*
    #[tokio::test]
    async fn test_login_success() {
        // Test successful login with valid credentials
    }

    #[tokio::test]
    async fn test_login_failure() {
        // Test login failure with invalid credentials
    }

    #[tokio::test]
    async fn test_logout() {
        // Test proper session cleanup on logout
    }

    #[tokio::test]
    async fn test_session_expiry() {
        // Test handling of expired sessions
    }
    */

    // TODO: Complex Error Scenarios
    /*
    #[tokio::test]
    async fn test_network_timeout() {
        // Test handling of network timeouts
    }

    #[tokio::test]
    async fn test_server_unavailable() {
        // Test handling of server unavailability
    }

    #[tokio::test]
    async fn test_malformed_response() {
        // Test handling of malformed server responses
    }

    #[tokio::test]
    async fn test_partial_response() {
        // Test handling of incomplete server responses
    }
    */

    // TODO: Unicode/International Character Tests
    /*
    #[tokio::test]
    async fn test_unicode_folder_names() {
        // Test handling of folder names with Unicode characters
    }

    #[tokio::test]
    async fn test_unicode_email_content() {
        // Test handling of email content with Unicode characters
    }

    #[tokio::test]
    async fn test_unicode_search_criteria() {
        // Test handling of search criteria with Unicode characters
    }

    #[tokio::test]
    async fn test_unicode_message_ids() {
        // Test handling of message IDs with Unicode characters
    }
    */

    // TODO: Session Cleanup Tests
    /*
    #[tokio::test]
    async fn test_session_cleanup_on_error() {
        // Test proper cleanup when operations fail
    }

    #[tokio::test]
    async fn test_session_cleanup_on_timeout() {
        // Test proper cleanup when operations timeout
    }

    #[tokio::test]
    async fn test_session_cleanup_on_disconnect() {
        // Test proper cleanup when connection is lost
    }

    #[tokio::test]
    async fn test_resource_cleanup() {
        // Test proper cleanup of all resources
    }
    */

    // TODO: Concurrent Access Tests
    /*
    #[tokio::test]
    async fn test_concurrent_folder_access() {
        // Test multiple clients accessing the same folder
    }

    #[tokio::test]
    async fn test_concurrent_email_access() {
        // Test multiple clients accessing the same email
    }

    #[tokio::test]
    async fn test_concurrent_flag_updates() {
        // Test concurrent updates to email flags
    }

    #[tokio::test]
    async fn test_concurrent_move_operations() {
        // Test concurrent move operations on the same email
    }
    */

    // TODO: Message Content Tests
    /*
    #[tokio::test]
    async fn test_message_attachments() {
        // Test handling of email attachments
    }

    #[tokio::test]
    async fn test_message_headers() {
        // Test handling of email headers
    }

    #[tokio::test]
    async fn test_message_mime_types() {
        // Test handling of different MIME types
    }

    #[tokio::test]
    async fn test_message_encoding() {
        // Test handling of different message encodings
    }

    #[tokio::test]
    async fn test_message_size_limits() {
        // Test handling of large messages
    }

    #[tokio::test]
    async fn test_message_integrity() {
        // Test message integrity during operations
    }
    */
} 