#[cfg(test)]
mod tests {
    use crate::imap::client::ImapClient;
    use crate::imap::error::ImapError;
    use crate::imap::session::ImapSession; // Use the trait
    use crate::imap::types::{Email, Folder, SearchCriteria, MailboxInfo};
    use async_trait::async_trait;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::sync::Mutex;

    // --- Mock IMAP Session ---

    // Structure to track which methods were called on the mock
    #[derive(Debug, Default)]
    struct MockCallTracker {
        list_folders_called: AtomicBool,
        create_folder_called: AtomicBool,
        delete_folder_called: AtomicBool,
        rename_folder_called: AtomicBool,
        select_folder_called: AtomicBool,
        search_emails_called: AtomicBool,
        fetch_emails_called: AtomicBool,
        move_email_called: AtomicBool,
        logout_called: AtomicBool,
        store_flags_called: AtomicBool,
        append_called: AtomicBool,
        expunge_called: AtomicBool,
    }

    // The Mock Session implementation
    #[derive(Debug, Clone)]
    struct MockImapSession {
        tracker: Arc<MockCallTracker>,
        list_folders_result: Result<Vec<Folder>, ImapError>,
        select_folder_result: Result<MailboxInfo, ImapError>,
        search_emails_result: Result<Vec<u32>, ImapError>,
        fetch_emails_result: Result<Vec<Email>, ImapError>,
        create_result: Result<(), ImapError>,
        delete_result: Result<(), ImapError>,
        rename_result: Result<(), ImapError>,
        move_result: Result<(), ImapError>,
        logout_result: Result<(), ImapError>,
        store_flags_result: Result<(), ImapError>,
        append_result: Result<(), ImapError>,
    }

    impl MockImapSession {
        // Helper to create a mock with default Ok results
        fn default_ok() -> Self {
            Self {
                tracker: Arc::new(MockCallTracker::default()),
                list_folders_result: Ok(vec![
                    Folder {
                        name: "INBOX".to_string(),
                        delimiter: Some("/".to_string()),
                    },
                    Folder {
                        name: "Sent".to_string(),
                        delimiter: Some("/".to_string()),
                    },
                ]),
                select_folder_result: Ok(MailboxInfo {
                    flags: vec!["\\Seen".to_string()],
                    exists: 10,
                    recent: 1,
                    unseen: Some(5),
                    permanent_flags: vec!["\\".to_string()],
                    uid_next: Some(101),
                    uid_validity: Some(12345),
                }),
                search_emails_result: Ok(vec![1, 2, 3]),
                fetch_emails_result: Ok(vec![Email {
                    uid: 1,
                    flags: vec![],
                    size: Some(100),
                    envelope: None,
                    body: None,
                }]),
                create_result: Ok(()),
                delete_result: Ok(()),
                rename_result: Ok(()),
                move_result: Ok(()),
                logout_result: Ok(()),
                store_flags_result: Ok(()),
                append_result: Ok(()),
            }
        }

        // Helper to set a specific method to fail
        fn set_fail(mut self, method: &str) -> Self {
             let err = ImapError::Operation(format!("Mock {} failed", method));
             match method {
                 "list_folders" => self.list_folders_result = Err(err),
                 "select_folder" => self.select_folder_result = Err(err),
                 "search_emails" => self.search_emails_result = Err(err),
                 "fetch_emails" => self.fetch_emails_result = Err(err),
                 "create_folder" => self.create_result = Err(err),
                 "delete_folder" => self.delete_result = Err(err),
                 "rename_folder" => self.rename_result = Err(err),
                 "move_email" => self.move_result = Err(err),
                 "logout" => self.logout_result = Err(err),
                 _ => panic!("Unknown method to fail: {}", method),
             }
             self
        }

        fn set_fetch_emails(mut self, result: Result<Vec<Email>, ImapError>) -> Self {
            self.fetch_emails_result = result;
            self
        }
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
            self.tracker
                .list_folders_called
                .store(true, Ordering::SeqCst);
            self.list_folders_result.clone() // Clone to return owned value
        }

        async fn create_folder(&self, _name: &str) -> Result<(), ImapError> {
            self.tracker
                .create_folder_called
                .store(true, Ordering::SeqCst);
            self.create_result.clone()
        }

        async fn delete_folder(&self, _name: &str) -> Result<(), ImapError> {
            self.tracker
                .delete_folder_called
                .store(true, Ordering::SeqCst);
            self.delete_result.clone()
        }

        async fn rename_folder(&self, _from: &str, _to: &str) -> Result<(), ImapError> {
            self.tracker
                .rename_folder_called
                .store(true, Ordering::SeqCst);
            self.rename_result.clone()
        }

        async fn select_folder(&self, _name: &str) -> Result<MailboxInfo, ImapError> {
            self.tracker
                .select_folder_called
                .store(true, Ordering::SeqCst);
            self.select_folder_result.clone()
        }

        async fn search_emails(&self, _criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
            self.tracker
                .search_emails_called
                .store(true, Ordering::SeqCst);
            self.search_emails_result.clone()
        }

        async fn fetch_emails(&self, _uids: Vec<u32>, _fetch_body: bool) -> Result<Vec<Email>, ImapError> {
            self.tracker
                .fetch_emails_called
                .store(true, Ordering::SeqCst);
            if _uids.is_empty() { 
                return Ok(Vec::new()); 
            }
            self.fetch_emails_result.clone()
        }

        async fn fetch_raw_message(&mut self, _uid: u32) -> Result<Vec<u8>, ImapError> {
            // Basic mock implementation
            Ok(Vec::new()) 
        }

        async fn move_email(
            &self,
            _source_folder: &str,
            _uids: Vec<u32>,
            _destination_folder: &str,
        ) -> Result<(), ImapError> {
            self.tracker.move_email_called.store(true, Ordering::SeqCst);
            // Handle empty UID case as client does
            if _uids.is_empty() { return Ok(()); }
            self.move_result.clone()
        }

        async fn store_flags(&self, _uids: Vec<u32>, _operation: crate::imap::session::StoreOperation, _flags: Vec<String>) -> Result<(), ImapError> {
            self.tracker.store_flags_called.store(true, Ordering::SeqCst);
            // Handle empty UID case if necessary
            if _uids.is_empty() { return Ok(()); }
            self.store_flags_result.clone()
        }

        async fn append(&self, _folder: &str, _payload: Vec<u8>) -> Result<(), ImapError> {
            self.tracker.append_called.store(true, Ordering::SeqCst);
            self.append_result.clone()
        }

        async fn expunge(&self) -> Result<(), ImapError> {
            self.tracker.expunge_called.store(true, Ordering::SeqCst);
            Ok(()) // Always return Ok since the result field is removed
        }

        async fn logout(&self) -> Result<(), ImapError> {
            self.tracker.logout_called.store(true, Ordering::SeqCst);
            self.logout_result.clone()
        }
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_list_folders_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.list_folders().await;
        assert!(result.is_ok());
        let folders = result.unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "INBOX");
        assert!(tracker.list_folders_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_list_folders_error() {
        let mock_session = MockImapSession::default_ok().set_fail("list_folders");
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.list_folders().await;
        assert!(result.is_err());
        assert!(tracker.list_folders_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_create_folder_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.create_folder("NewFolder").await;
        assert!(result.is_ok());
        assert!(tracker.create_folder_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_delete_folder_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.delete_folder("ToDelete").await;
        assert!(result.is_ok());
        assert!(tracker.delete_folder_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_rename_folder_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.rename_folder("OldName", "NewName").await;
        assert!(result.is_ok());
        assert!(tracker.rename_folder_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_select_folder_delegation() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.select_folder("INBOX").await;
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.exists, 10);
        assert!(tracker.select_folder_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_search_emails_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.search_emails(SearchCriteria::All).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
        assert!(tracker.search_emails_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_fetch_emails_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.fetch_emails(vec![1], true).await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].uid, 1);
        assert!(tracker.fetch_emails_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_fetch_emails_empty_uids() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.fetch_emails(vec![], true).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
        assert!(!tracker.fetch_emails_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_move_email_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.move_email("INBOX", vec![1, 2], "Archive").await;
        assert!(result.is_ok());
        assert!(tracker.move_email_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_move_email_empty_uids() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.move_email("INBOX", vec![], "Archive").await;
        assert!(result.is_ok());
         // Ensure the mock move was NOT called
        assert!(!tracker.move_email_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_logout_success() {
        let mock_session = MockImapSession::default_ok();
        let tracker = mock_session.tracker.clone();
        // Use Arc::new(Mutex::new()) directly as new_with_session expects it
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.logout().await; // Consumes client
        assert!(result.is_ok());
        assert!(tracker.logout_called.load(Ordering::SeqCst));
    }

     #[tokio::test]
    async fn test_logout_error() {
         let mock_session = MockImapSession::default_ok().set_fail("logout");
        let tracker = mock_session.tracker.clone();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));

        let result = client.logout().await;
        assert!(result.is_err());
        assert!(tracker.logout_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_client_fetch_emails_success() {
        let mock_session = MockImapSession::default_ok()
            .set_fetch_emails(Ok(vec![Email { uid: 1, flags: vec!["\\Seen".into()], size: Some(100), envelope: None, body: Some(b"body".to_vec()) }]));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        let result = client.fetch_emails(vec![1], true).await;
        assert!(result.is_ok());
        let emails = result.unwrap();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].uid, 1);
        assert_eq!(emails[0].body, Some(b"body".to_vec()));
    }

    #[tokio::test]
    async fn test_client_fetch_emails_error() {
        let mock_session = MockImapSession::default_ok()
            .set_fetch_emails(Err(ImapError::Command("Failed fetch".to_string())));
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        let result = client.fetch_emails(vec![1], true).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_fetch_emails_empty_input() {
        let mock_session = MockImapSession::default_ok();
        let client = ImapClient::new_with_session(Arc::new(Mutex::new(mock_session)));
        let result = client.fetch_emails(vec![], true).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
} 