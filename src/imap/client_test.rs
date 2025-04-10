#[cfg(test)]
mod tests {
    use crate::{
        api::sse::SseState,
        config::Settings,
        imap::{
            client::ImapClient,
            error::ImapError,
            session::AsyncImapOps,
            types::{Email, Envelope, FlagOperation, MailboxInfo, Address},
        },
        prelude::setup_test_logger,
    };
    use async_trait::async_trait;
    use mockall::{
        mock,
        predicate::eq
    };
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Define the mock directly in the test module
    mock! {
        #[derive(Debug)]
        pub AsyncImapOps {
            async fn login(&self, username: &str, password: &str) -> Result<(), ImapError>;
            async fn logout(&self) -> Result<(), ImapError>;
            async fn list_folders(&self) -> Result<Vec<String>, ImapError>;
            async fn create_folder(&self, folder_name: &str) -> Result<(), ImapError>;
            async fn delete_folder(&self, folder_name: &str) -> Result<(), ImapError>;
            async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError>;
            async fn select_folder(&self, folder_name: &str) -> Result<MailboxInfo, ImapError>;
            async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError>;
            async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError>;
            async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError>;
            async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError>;
            async fn expunge(&self) -> Result<(), ImapError>;
            async fn append(&self, folder_name: &str, body: &[u8], flags: &[String]) -> Result<(), ImapError>;
            async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError>;
        }
    }

    #[async_trait]
    impl AsyncImapOps for MockAsyncImapOps {
        async fn login(&self, username: &str, password: &str) -> Result<(), ImapError> {
            self.login(username, password).await
        }
        
        async fn logout(&self) -> Result<(), ImapError> {
            self.logout().await
        }

        async fn list_folders(&self) -> Result<Vec<String>, ImapError> {
            self.list_folders().await
        }

        async fn create_folder(&self, folder_name: &str) -> Result<(), ImapError> {
            self.create_folder(folder_name).await
        }

        async fn delete_folder(&self, folder_name: &str) -> Result<(), ImapError> {
            self.delete_folder(folder_name).await
        }

        async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError> {
            self.rename_folder(old_name, new_name).await
        }

        async fn select_folder(&self, folder_name: &str) -> Result<MailboxInfo, ImapError> {
            self.select_folder(folder_name).await
        }

        async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError> {
            self.search_emails(criteria).await
        }

        async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError> {
            self.fetch_emails(uids).await
        }

        async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
            self.move_email(uid, from_folder, to_folder).await
        }

        async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError> {
            self.store_flags(uids, operation, flags).await
        }

        async fn expunge(&self) -> Result<(), ImapError> {
            self.expunge().await
        }

        async fn append(&self, folder_name: &str, body: &[u8], flags: &[String]) -> Result<(), ImapError> {
            self.append(folder_name, body, flags).await
        }

        async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError> {
            self.fetch_raw_message(uid).await
        }
    }

    // Helper function to set up the client with the mock session
    fn setup_client(mock_session: MockAsyncImapOps) -> ImapClient<MockAsyncImapOps> {
        ImapClient::new(mock_session)
    }

    // Helper function to create a default mock with success responses
    fn create_default_mock() -> MockAsyncImapOps {
        let mut mock = MockAsyncImapOps::new();
        
        mock.expect_login()
            .returning(|_, _| Ok(()));
        
        mock.expect_logout()
            .returning(|| Ok(()));
        
        mock.expect_list_folders()
            .returning(|| Ok(vec!["INBOX".to_string()]));
        
        mock.expect_create_folder()
            .returning(|_| Ok(()));
        
        mock.expect_delete_folder()
            .returning(|_| Ok(()));
        
        mock.expect_select_folder()
            .returning(|_| Ok(MailboxInfo {
                flags: vec!["\\Seen".to_string()],
                exists: 1,
                recent: 0,
                unseen: Some(0),
                permanent_flags: vec!["\\Seen".to_string()],
                uid_next: Some(2),
                uid_validity: Some(1),
            }));
        
        mock.expect_fetch_emails()
            .returning(|_uids: &[u32]| Ok(vec![Email {
                uid: 1,
                flags: vec!["\\Seen".to_string()],
                internal_date: None,
                envelope: None,
                body: None,
            }]));
        
        mock.expect_store_flags()
            .returning(|_uids: &[u32], _op: FlagOperation, _flags: &[String]| Ok(()));
        
        mock.expect_move_email()
            .returning(|_, _, _| Ok(()));
        
        mock.expect_expunge()
            .returning(|| Ok(()));
        
        mock.expect_append()
            .returning(|_, _, _| Ok(()));
        
        mock.expect_fetch_raw_message()
            .returning(|_| Ok(vec![]));
            
        mock
    }

    #[tokio::test]
    async fn test_list_folders_success() {
        let client = setup_client(create_default_mock());
        let folders = client.list_folders().await.unwrap();
        assert_eq!(folders, vec!["INBOX".to_string()]);
    }

    #[tokio::test]
    async fn test_create_folder_success() {
        let client = setup_client(create_default_mock());
        let result = client.create_folder("Sent").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_emails_success() {
        let client = setup_client(create_default_mock());
        let emails = client.fetch_emails(&[1]).await.unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].uid, 1);
        assert_eq!(emails[0].flags, vec!["\\Seen".to_string()]);
    }

    #[tokio::test]
    async fn test_fetch_emails_error() {
        let mut mock = create_default_mock();
        mock.expect_fetch_emails()
            .returning(|_uids: &[u32]| Err(ImapError::Fetch("Simulated fetch error".to_string())));
        let client = setup_client(mock);
        let result = client.fetch_emails(&[1]).await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Fetch(msg)) => assert_eq!(msg, "Simulated fetch error"),
            _ => panic!("Expected Fetch error"),
        }
    }

    #[tokio::test]
    async fn test_store_flags_add() {
        let mut mock = create_default_mock();
        mock.expect_store_flags()
            .returning(|uids: &[u32], op: FlagOperation, flags: &[String]| {
                assert_eq!(uids, &[1]);
                assert_eq!(op, FlagOperation::Add);
                assert_eq!(flags, &["\\Flagged".to_string()]);
                Ok(())
            });
        let client = setup_client(mock);
        let flags_vec = vec!["\\Flagged".to_string()];
        let result = client.store_flags(&[1], FlagOperation::Add, &flags_vec).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_flags_remove() {
        let mut mock = create_default_mock();
        mock.expect_store_flags()
            .returning(|uids: &[u32], op: FlagOperation, flags: &[String]| {
                assert_eq!(uids, &[1]);
                assert_eq!(op, FlagOperation::Remove);
                assert_eq!(flags, &["\\Seen".to_string()]);
                Ok(())
            });
        let client = setup_client(mock);
        let flags_vec = vec!["\\Seen".to_string()];
        let result = client.store_flags(&[1], FlagOperation::Remove, &flags_vec).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_flags_set() {
        let mut mock = create_default_mock();
        mock.expect_store_flags()
            .returning(|uids: &[u32], op: FlagOperation, flags: &[String]| {
                assert_eq!(uids, &[1]);
                assert_eq!(op, FlagOperation::Set);
                assert_eq!(flags, &[r"\\Answered".to_string()]);
                Ok(())
            });
        let client = setup_client(mock);
        let flags_vec = vec![r"\\Answered".to_string()];
        let result = client.store_flags(&[1], FlagOperation::Set, &flags_vec).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_folders_error() {
        let mut mock = create_default_mock();
        mock.expect_list_folders()
            .returning(|| Err(ImapError::Operation("List failed".to_string())));
        let client = setup_client(mock);
        let result = client.list_folders().await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Operation(msg)) => assert_eq!(msg, "List failed"),
            _ => panic!("Expected Operation error"),
        }
    }

    #[tokio::test]
    async fn test_create_folder_error() {
        let mut mock = create_default_mock();
        mock.expect_create_folder()
            .returning(|_| Err(ImapError::Operation("Create failed".to_string())));
        let client = setup_client(mock);
        let result = client.create_folder("FailFolder").await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Operation(msg)) => assert_eq!(msg, "Create failed"),
            _ => panic!("Expected Operation error"),
        }
    }

    fn create_test_email(uid: u32, subject: &str) -> Email {
        Email {
            uid,
            flags: vec!["\\Seen".to_string()],
            internal_date: None,
            envelope: Some(Envelope {
                subject: Some(subject.to_string()),
                from: vec![Address{name: Some("from".to_string()), mailbox: Some("a".to_string()), host: Some("b".to_string())}],
                to: vec![], cc: vec![], bcc: vec![], reply_to: vec![], date: None, in_reply_to: None, message_id: None
            }),
            body: None,
        }
    }

    #[tokio::test]
    async fn test_fetch_emails_success_multiple() {
        let mut mock = create_default_mock();
        let emails = vec![
            create_test_email(1, "Subject 1"),
            create_test_email(2, "Subject 2")
        ];
        mock.expect_fetch_emails()
            .returning(move |_uids: &[u32]| Ok(emails.clone()));
        let client = setup_client(mock);
        let fetched_emails = client.fetch_emails(&[1, 2]).await.unwrap();
        assert_eq!(fetched_emails.len(), 2);
        assert_eq!(fetched_emails[0].uid, 1);
        assert_eq!(fetched_emails[1].uid, 2);
    }

    #[tokio::test]
    async fn test_store_flags_success() {
        let client = setup_client(create_default_mock());
        let flags_vec = vec!["\\Flagged".to_string()];
        let result = client.store_flags(&[1], FlagOperation::Add, &flags_vec).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_flags_error() {
        let mut mock = create_default_mock();
        mock.expect_store_flags()
            .returning(|_, _, _| Err(ImapError::Operation("Store failed".to_string())));
        let client = setup_client(mock);
        let flags_vec = vec!["\\Seen".to_string()];
        let result = client.store_flags(&[1], FlagOperation::Set, &flags_vec).await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Operation(msg)) => assert_eq!(msg, "Store failed"),
            _ => panic!("Expected Operation error"),
        }
    }
    
    #[tokio::test]
    async fn test_delete_folder_success() {
        let client = setup_client(create_default_mock());
        assert!(client.delete_folder("ToDelete").await.is_ok());
    }

     #[tokio::test]
    async fn test_login_success() {
        let client = setup_client(create_default_mock());
        assert!(client.login("user", "pass").await.is_ok());
    }

     #[tokio::test]
    async fn test_logout_success() {
        let client = setup_client(create_default_mock());
        assert!(client.logout().await.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_emails_with_details() {
        let mut mock = create_default_mock();
        let emails = vec![Email {
            uid: 1,
            flags: vec!["\\Seen".to_string()],
            envelope: Some(Envelope {
                subject: Some("Test Subject".to_string()),
                from: vec![Address{name: Some("from".to_string()), mailbox: Some("a".to_string()), host: Some("b".to_string())}],
                to: vec![], cc: vec![], bcc: vec![], reply_to: vec![], date: None, in_reply_to: None, message_id: None,
            }),
            internal_date: None,
            body: None,
        }];

        let emails_clone = emails.clone();
        mock.expect_fetch_emails()
            .with(eq(&[1_u32] as &[u32]))
            .times(1)
            .returning(move |_uids: &[u32]| Ok(emails_clone.clone()));

        let client = setup_client(mock);
        let fetched_emails = client.fetch_emails(&[1]).await.unwrap();

        assert_eq!(fetched_emails.len(), 1);
        assert_eq!(fetched_emails[0].uid, 1);
        assert!(fetched_emails[0].envelope.is_some());
        let env = fetched_emails[0].envelope.as_ref().unwrap();
        assert_eq!(env.subject.as_deref(), Some("Test Subject"));
        assert_eq!(env.from.len(), 1);
        assert_eq!(env.from[0].name.as_deref(), Some("from"));
    }

    #[tokio::test]
    async fn test_move_email_success() {
        let mut mock = create_default_mock();
        mock.expect_move_email()
            .with(eq(1), eq("INBOX"), eq("Archive"))
            .times(1)
            .returning(|_, _, _| Ok(()));

        let client = setup_client(mock);
        let result = client.move_email(1, "INBOX", "Archive").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_move_email_error() {
        let mut mock = create_default_mock();
        mock.expect_move_email()
            .returning(|_, _, _| Err(ImapError::InvalidMailbox("Folder does not exist".to_string())));
        let client = setup_client(mock);
        let result = client.move_email(1, "INBOX", "NonexistentFolder").await;
        assert!(result.is_err());
        match result {
            Err(ImapError::InvalidMailbox(msg)) => assert_eq!(msg, "Folder does not exist"),
            _ => panic!("Expected InvalidMailbox error"),
        }
    }

    #[tokio::test]
    async fn test_append_email_success() {
        let mut mock = create_default_mock();
        let content = b"From: test@example.com\r\nSubject: Test\r\n\r\nTest body";
        let flags = vec!["\\Seen".to_string(), "\\Flagged".to_string()];
        
        mock.expect_append()
            .with(eq("INBOX"), eq(content.as_ref()), eq(flags.clone()))
            .times(1)
            .returning(|_, _, _| Ok(()));

        let client = setup_client(mock);
        let result = client.append("INBOX", content, &flags).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_append_email_error() {
        let mut mock = create_default_mock();
        mock.expect_append()
            .returning(|_, _, _| Err(ImapError::Flag("Invalid flag specified".to_string())));
        let client = setup_client(mock);
        let result = client.append("INBOX", b"Invalid email content", &["\\Invalid".to_string()]).await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Flag(msg)) => assert_eq!(msg, "Invalid flag specified"),
            _ => panic!("Expected Flag error"),
        }
    }

    #[tokio::test]
    async fn test_fetch_emails_empty_result() {
        let mut mock = create_default_mock();
        mock.expect_fetch_emails()
            .with(eq(&[1_u32] as &[u32]))
            .times(1)
            .returning(|_| Ok(vec![]));

        let client = setup_client(mock);
        let emails = client.fetch_emails(&[1]).await.unwrap();
        assert!(emails.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_emails_multiple_flags() {
        let mut mock = create_default_mock();
        let email = Email {
            uid: 1,
            flags: vec!["\\Seen".to_string(), "\\Flagged".to_string(), "\\Answered".to_string()],
            internal_date: None,
            envelope: Some(Envelope {
                subject: Some("Test".to_string()),
                from: vec![Address{name: Some("Test".to_string()), mailbox: Some("test".to_string()), host: Some("example.com".to_string())}],
                to: vec![], cc: vec![], bcc: vec![], reply_to: vec![], date: None, in_reply_to: None, message_id: None
            }),
            body: None,
        };
        
        let email_clone = email.clone();
        mock.expect_fetch_emails()
            .with(eq(&[1_u32] as &[u32]))
            .times(1)
            .returning(move |_| Ok(vec![email_clone.clone()]));

        let client = setup_client(mock);
        let emails = client.fetch_emails(&[1]).await.unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].flags.len(), 3);
        assert!(emails[0].flags.contains(&"\\Seen".to_string()));
        assert!(emails[0].flags.contains(&"\\Flagged".to_string()));
        assert!(emails[0].flags.contains(&"\\Answered".to_string()));
    }

    #[tokio::test]
    async fn test_store_flags_multiple_uids() {
        let mut mock = create_default_mock();
        let uids = vec![1, 2, 3];
        let flags = vec!["\\Seen".to_string()];
        
        let flags_clone = flags.clone();
        mock.expect_store_flags()
            .with(eq(&uids), eq(FlagOperation::Add), eq(&flags_clone as &[String]))
            .times(1)
            .returning(|_, _, _| Ok(()));

        let client = setup_client(mock);
        let result = client.store_flags(&uids, FlagOperation::Add, &flags).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_expunge_success() {
        let mut mock = create_default_mock();
        mock.expect_expunge()
            .times(1)
            .returning(|| Ok(()));

        let client = setup_client(mock);
        let result = client.expunge().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_expunge_error() {
        let mut mock = create_default_mock();
        mock.expect_expunge()
            .returning(|| Err(ImapError::Command("Failed to expunge".to_string())));

        let client = setup_client(mock);
        let result = client.expunge().await;
        assert!(result.is_err());
        match result {
            Err(ImapError::Command(msg)) => assert_eq!(msg, "Failed to expunge"),
            _ => panic!("Expected Command error"),
        }
    }

    #[tokio::test]
    async fn test_fetch_raw_message_success() {
        let mut mock = create_default_mock();
        let raw_content = b"From: test@example.com\r\nSubject: Test\r\n\r\nTest body".to_vec();
        
        mock.expect_fetch_raw_message()
            .with(eq(1))
            .times(1)
            .returning(move |_| Ok(raw_content.clone()));

        let client = setup_client(mock);
        let content = client.fetch_raw_message(1).await.unwrap();
        assert_eq!(content, b"From: test@example.com\r\nSubject: Test\r\n\r\nTest body");
    }

    #[tokio::test]
    async fn test_fetch_raw_message_error() {
        let mut mock = create_default_mock();
        mock.expect_fetch_raw_message()
            .returning(|_| Err(ImapError::MissingData("Message not found".to_string())));
        let client = setup_client(mock);
        let result = client.fetch_raw_message(999).await;
        assert!(result.is_err());
        match result {
            Err(ImapError::MissingData(msg)) => assert_eq!(msg, "Message not found"),
            _ => panic!("Expected MissingData error"),
        }
    }
} 