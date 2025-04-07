#[cfg(test)]
mod tests {
    use crate::imap::ImapClient;
    use crate::imap::error::ImapError;
    use crate::imap::types::{Email, Folder, SearchCriteria, OwnedMailbox};
    use crate::imap::session::ImapSession;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockImapSession {
        folders: Arc<Mutex<Vec<Folder>>>,
        emails: Arc<Mutex<Vec<Email>>>,
        should_fail: bool,
        list_folders_error: Option<ImapError>,
    }

    // Manual Default implementation
    impl Default for MockImapSession {
        fn default() -> Self {
            Self {
                folders: Arc::new(Mutex::new(Vec::new())),
                emails: Arc::new(Mutex::new(Vec::new())),
                should_fail: false,
                list_folders_error: None,
            }
        }
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
            if self.should_fail {
                return Err(ImapError::Connection("Simulated connection error".into()));
            }
            Ok(self.folders.lock().await.clone())
        }

        async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
            if self.should_fail {
                return Err(ImapError::Mailbox("Simulated folder creation error".into()));
            }
            let mut folders = self.folders.lock().await;
            if folders.iter().any(|f| f.name == name) {
                return Err(ImapError::Mailbox("Folder already exists".into()));
            }
            folders.push(Folder {
                name: name.to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec![],
            });
            Ok(())
        }

        async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
            if self.should_fail {
                return Err(ImapError::Mailbox("Simulated folder deletion error".into()));
            }
            let mut folders = self.folders.lock().await;
            if let Some(pos) = folders.iter().position(|f| f.name == name) {
                folders.remove(pos);
                Ok(())
            } else {
                Err(ImapError::Mailbox("Folder not found".into()))
            }
        }

        async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
            if self.should_fail { return Err(ImapError::Mailbox("Simulated rename error".into())); }
            let mut folders = self.folders.lock().await;
            if let Some(folder) = folders.iter_mut().find(|f| f.name == from) {
                folder.name = to.to_string();
                Ok(())
            } else {
                Err(ImapError::Mailbox("Source folder not found".into()))
            }
        }

        async fn select_folder(&self, _name: &str) -> Result<OwnedMailbox<'static>, ImapError> { 
             Ok(imap_types::mailbox::Mailbox::Inbox) 
        }

        async fn search_emails(&self, _criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> { Ok(vec![1,2,3]) }
        
        async fn fetch_emails(&self, _uids: Vec<u32>) -> Result<Vec<Email>, ImapError> { Ok(vec![]) }
        
        async fn move_email(&self, _uids: Vec<u32>, _dest: &str) -> Result<(), ImapError> { Ok(()) }
        
        async fn logout(self: Arc<Self>) -> Result<(), ImapError> { Ok(()) }
    }

    // Helper to create ImapClient with the mock session
    fn create_test_client(session: MockImapSession) -> ImapClient {
        // Wrap the session in Arc for ImapClient::new
        ImapClient::new(Arc::new(session))
    }

    #[tokio::test]
    async fn test_list_folders_success() {
        let mock_session = MockImapSession::default();
        mock_session.folders.lock().await.push(Folder {
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec![],
        });
        let client = create_test_client(mock_session);
        let result = client.list_folders().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_create_folder_success() {
        let mock_session = MockImapSession::default();
        let client = create_test_client(mock_session);
        let result = client.create_folder("Sent").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_folder_duplicate() {
        let mock_session = MockImapSession::default();
        mock_session.folders.lock().await.push(Folder {
            name: "Sent".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec![],
        });
        let client = create_test_client(mock_session);
        let result = client.create_folder("Sent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImapError::Mailbox(_)));
    }

    #[tokio::test]
    async fn test_delete_folder_success() {
        let mock_session = MockImapSession::default();
        mock_session.folders.lock().await.push(Folder {
            name: "Trash".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec![],
        });
        let client = create_test_client(mock_session);
        let result = client.delete_folder("Trash").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_delete_folder_not_found() {
        let mock_session = MockImapSession::default();
        let client = create_test_client(mock_session);
        let result = client.delete_folder("NonExistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImapError::Mailbox(_)));
    }

    #[tokio::test]
    async fn test_list_folders_error() {
        let mut mock_session = MockImapSession::default();
        mock_session.should_fail = true;
        let client = create_test_client(mock_session);
        let result = client.list_folders().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImapError::Connection(_)));
    }

    // Add more tests for other ImapClient methods (rename, select, search, fetch, move, logout)
    // covering both success and error cases.
} 