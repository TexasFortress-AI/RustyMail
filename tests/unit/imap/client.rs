#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::client::ImapClient;
    use crate::imap::error::ImapError;
    use crate::imap::session::{AsyncImapOps, Email, Envelope, FlagOperation};
    use crate::utils::MockImapSession;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Connection Tests
    #[tokio::test]
    async fn test_connection_management() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new(mock_session);
        
        // Test session reuse
        let result1 = client.list_folders().await;
        assert!(result1.is_ok());
        
        let result2 = client.list_folders().await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new(mock_session));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.list_folders().await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.search_emails("ALL").await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    // Operation Tests
    #[tokio::test]
    async fn test_folder_operations_sequence() {
        let mock_session = MockImapSession::new();
        let client = ImapClient::new(mock_session);
        
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
        let client = ImapClient::new(mock_session);
        
        // Search emails
        let result = client.search_emails("ALL").await;
        assert!(result.is_ok());
        let uids = result.unwrap();
        assert_eq!(uids, vec![1, 2, 3]);
        
        // Fetch emails
        let result = client.fetch_emails(&uids).await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails.len(), 1);
        
        // Store flags
        let result = client.store_flags(&[1], FlagOperation::Add, &["\\Seen".to_string()]).await;
        assert!(result.is_ok());
        
        // Move email
        let result = client.move_email(1, "INBOX", "Archive").await;
        assert!(result.is_ok());
    }

    // Error Handling Tests
    #[tokio::test]
    async fn test_connection_error_handling() {
        let mock_session = MockImapSession::new_failing();
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::ConnectionError(_))));
    }

    #[tokio::test]
    async fn test_operation_error_handling() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::OperationError("Test error".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_folder_error_handling() {
        let mock_session = MockImapSession::new()
            .set_select_folder_result(Err(ImapError::NotFound));
        let client = ImapClient::new(mock_session);
        
        let result = client.select_folder("NonExistentFolder").await;
        assert!(matches!(result, Err(ImapError::NotFound)));
    }

    // Edge Cases
    #[tokio::test]
    async fn test_empty_folder_list() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Ok(vec![]));
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_search_results() {
        let mock_session = MockImapSession::new()
            .set_search_emails_result(Ok(vec![]));
        let client = ImapClient::new(mock_session);
        
        let result = client.search_emails("ALL").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_fetch_results() {
        let mock_session = MockImapSession::new()
            .set_fetch_emails_result(Ok(vec![]));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_emails(&[1, 2, 3]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // State Management
    #[tokio::test]
    async fn test_session_state_after_error() {
        let mock_session = MockImapSession::new_failing();
        let client = ImapClient::new(mock_session);
        
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
        let client = ImapClient::new(mock_session);
        
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
        let client = ImapClient::new(mock_session);
        
        let message = b"From: test@example.com\r\n\r\nTest message";
        let result = client.append("INBOX", message, &["\\Seen".to_string()]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_append_message_error() {
        let mock_session = MockImapSession::new()
            .set_append_result(Err(ImapError::OperationError("Append failed".to_string())));
        let client = ImapClient::new(mock_session);
        
        let message = b"From: test@example.com\r\n\r\nTest message";
        let result = client.append("INBOX", message, &["\\Seen".to_string()]).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    // Fetch Raw Message Tests
    #[tokio::test]
    async fn test_fetch_raw_message() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(b"Raw message content".to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"Raw message content".to_vec());
    }

    #[tokio::test]
    async fn test_fetch_raw_message_error() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Err(ImapError::OperationError("Fetch failed".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_fetch_raw_message_not_found() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Err(ImapError::NotFound));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(999).await;
        assert!(matches!(result, Err(ImapError::NotFound)));
    }

    // Authentication Tests
    #[tokio::test]
    async fn test_login_success() {
        let mock_session = MockImapSession::new();
        let mut client = ImapClient::new(mock_session);
        
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_login_failure() {
        let mock_session = MockImapSession::new_failing();
        let mut client = ImapClient::new(mock_session);
        
        let result = client.login("user@example.com", "wrong_password").await;
        assert!(matches!(result, Err(ImapError::AuthenticationError(_))));
    }

    #[tokio::test]
    async fn test_logout() {
        let mock_session = MockImapSession::new();
        let mut client = ImapClient::new(mock_session);
        
        // Login first
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
        
        // Then logout
        let result = client.logout().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_session_expiry() {
        let mock_session = MockImapSession::new()
            .set_login_result(Err(ImapError::AuthenticationError("Session expired".to_string())));
        let mut client = ImapClient::new(mock_session);
        
        // Try to login with expired session
        let result = client.login("user@example.com", "password").await;
        assert!(matches!(result, Err(ImapError::AuthenticationError(_))));
        
        // Verify subsequent operations fail
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::AuthenticationError(_))));
    }

    // Complex Error Scenarios
    #[tokio::test]
    async fn test_network_timeout() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::ConnectionError("Network timeout".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::ConnectionError(_))));
    }

    #[tokio::test]
    async fn test_server_unavailable() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::ConnectionError("Server unavailable".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::ConnectionError(_))));
    }

    #[tokio::test]
    async fn test_malformed_response() {
        let mock_session = MockImapSession::new()
            .set_fetch_emails_result(Err(ImapError::OperationError("Malformed response".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_emails(&[1]).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    #[tokio::test]
    async fn test_partial_response() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Err(ImapError::OperationError("Incomplete response".to_string())));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(matches!(result, Err(ImapError::OperationError(_))));
    }

    // Unicode/International Character Tests
    #[tokio::test]
    async fn test_unicode_folder_names() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Ok(vec!["收件箱".to_string(), "已发送".to_string()]));
        let client = ImapClient::new(mock_session);
        
        let result = client.list_folders().await;
        assert!(result.is_ok());
        let folders = result.unwrap();
        assert_eq!(folders, vec!["收件箱", "已发送"]);
    }

    #[tokio::test]
    async fn test_unicode_email_content() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok("Subject: 测试\r\n\r\n这是一封测试邮件".as_bytes().to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(String::from_utf8(content).unwrap().contains("测试"));
    }

    #[tokio::test]
    async fn test_unicode_search_criteria() {
        let mock_session = MockImapSession::new()
            .set_search_emails_result(Ok(vec![1, 2, 3]));
        let client = ImapClient::new(mock_session);
        
        let result = client.search_emails("SUBJECT 测试").await;
        assert!(result.is_ok());
        let uids = result.unwrap();
        assert_eq!(uids, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_unicode_message_ids() {
        let mock_session = MockImapSession::new()
            .set_fetch_emails_result(Ok(vec![Email {
                uid: 1,
                flags: vec![],
                envelope: Envelope {
                    message_id: Some("测试@example.com".to_string()),
                    subject: Some("测试".to_string()),
                    from: vec![],
                    to: vec![],
                    cc: vec![],
                    bcc: vec![],
                    date: None,
                    in_reply_to: None,
                    references: vec![],
                },
                body: None,
            }]));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_emails(&[1]).await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails[0].envelope.message_id, Some("测试@example.com".to_string()));
    }

    // Session Cleanup Tests
    #[tokio::test]
    async fn test_session_cleanup_on_error() {
        let mock_session = MockImapSession::new_failing();
        let mut client = ImapClient::new(mock_session);
        
        // Login first
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
        
        // Try an operation that fails
        let result = client.list_folders().await;
        assert!(result.is_err());
        
        // Verify we can still logout
        let result = client.logout().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_session_cleanup_on_timeout() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::ConnectionError("Timeout".to_string())));
        let mut client = ImapClient::new(mock_session);
        
        // Login first
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
        
        // Try an operation that times out
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::ConnectionError(_))));
        
        // Verify we can still logout
        let result = client.logout().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_session_cleanup_on_disconnect() {
        let mock_session = MockImapSession::new()
            .set_list_folders_result(Err(ImapError::ConnectionError("Disconnected".to_string())));
        let mut client = ImapClient::new(mock_session);
        
        // Login first
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
        
        // Try an operation that fails due to disconnect
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::ConnectionError(_))));
        
        // Verify we can still logout
        let result = client.logout().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resource_cleanup() {
        let mock_session = MockImapSession::new();
        let mut client = ImapClient::new(mock_session);
        
        // Login and perform some operations
        let result = client.login("user@example.com", "password").await;
        assert!(result.is_ok());
        
        let result = client.list_folders().await;
        assert!(result.is_ok());
        
        // Logout and verify resources are cleaned up
        let result = client.logout().await;
        assert!(result.is_ok());
        
        // Try to perform an operation after logout
        let result = client.list_folders().await;
        assert!(matches!(result, Err(ImapError::AuthenticationError(_))));
    }

    // Concurrent Access Tests
    #[tokio::test]
    async fn test_concurrent_folder_access() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new(mock_session));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.list_folders().await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.create_folder("TestFolder").await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_email_access() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new(mock_session));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.fetch_emails(&[1]).await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.fetch_emails(&[1]).await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_flag_updates() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new(mock_session));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.store_flags(&[1], FlagOperation::Add, &["\\Seen".to_string()]).await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.store_flags(&[1], FlagOperation::Add, &["\\Flagged".to_string()]).await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_move_operations() {
        let mock_session = MockImapSession::new();
        let client = Arc::new(ImapClient::new(mock_session));
        
        let client1 = client.clone();
        let client2 = client.clone();
        
        let handle1 = tokio::spawn(async move {
            client1.move_email(1, "INBOX", "Archive").await
        });
        
        let handle2 = tokio::spawn(async move {
            client2.move_email(1, "INBOX", "Trash").await
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        // One of these should succeed and the other should fail
        assert!(result1.unwrap().is_ok() || result2.unwrap().is_ok());
        assert!(result1.unwrap().is_err() || result2.unwrap().is_err());
    }

    // Message Content Tests
    #[tokio::test]
    async fn test_message_attachments() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(b"Content-Type: multipart/mixed; boundary=\"boundary\"\r\n\r\n--boundary\r\nContent-Type: text/plain\r\n\r\nTest message\r\n--boundary\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=\"test.pdf\"\r\n\r\nPDF content\r\n--boundary--".to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("multipart/mixed"));
        assert!(content_str.contains("application/pdf"));
        assert!(content_str.contains("filename=\"test.pdf\""));
    }

    #[tokio::test]
    async fn test_message_headers() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Test Subject\r\nDate: Mon, 1 Jan 2023 12:00:00 +0000\r\nMessage-ID: <test@example.com>\r\n\r\nTest message".to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("From: sender@example.com"));
        assert!(content_str.contains("To: recipient@example.com"));
        assert!(content_str.contains("Subject: Test Subject"));
        assert!(content_str.contains("Date: Mon, 1 Jan 2023 12:00:00 +0000"));
        assert!(content_str.contains("Message-ID: <test@example.com>"));
    }

    #[tokio::test]
    async fn test_message_mime_types() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(b"Content-Type: text/html; charset=UTF-8\r\n\r\n<html><body>Test HTML</body></html>".to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("Content-Type: text/html"));
        assert!(content_str.contains("charset=UTF-8"));
        assert!(content_str.contains("<html>"));
    }

    #[tokio::test]
    async fn test_message_encoding() {
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(b"Content-Type: text/plain; charset=ISO-8859-1\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nTest message with =E4=FC=DF characters".to_vec()));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("charset=ISO-8859-1"));
        assert!(content_str.contains("Content-Transfer-Encoding: quoted-printable"));
        assert!(content_str.contains("=E4=FC=DF"));
    }

    #[tokio::test]
    async fn test_message_size_limits() {
        // Create a 2MB message
        let mut large_message = Vec::with_capacity(2 * 1024 * 1024);
        large_message.extend_from_slice(b"From: test@example.com\r\n\r\n");
        large_message.extend(std::iter::repeat(b'X').take(2 * 1024 * 1024 - large_message.len()));
        
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(large_message));
        let client = ImapClient::new(mock_session);
        
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_message_integrity() {
        let original_message = b"From: test@example.com\r\n\r\nTest message";
        let mock_session = MockImapSession::new()
            .set_fetch_raw_message_result(Ok(original_message.to_vec()));
        let client = ImapClient::new(mock_session);
        
        // Fetch the message
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let fetched_message = result.unwrap();
        
        // Verify the message content is identical
        assert_eq!(fetched_message, original_message);
        
        // Try to fetch again to ensure consistency
        let result = client.fetch_raw_message(1).await;
        assert!(result.is_ok());
        let fetched_again = result.unwrap();
        assert_eq!(fetched_again, original_message);
    }
} 