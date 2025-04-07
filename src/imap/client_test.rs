#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::error::ImapError;
    use crate::imap::types::{Email, Folder, SearchCriteria};
    use async_trait::async_trait;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    // Mock ImapSession implementation for testing
    #[derive(Clone, Default)]
    struct MockImapSession {
        folders: Arc<Mutex<Vec<Folder>>>,
        emails: Arc<Mutex<Vec<Email>>>,
        // Add fields to simulate errors or specific states if needed
        should_fail: bool,
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
            if self.should_fail {
                return Err(ImapError::Connection("Simulated connection error".into()));
            }
            Ok(self.folders.lock().unwrap().clone())
        }

        async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
            if self.should_fail {
                return Err(ImapError::Folder("Simulated folder creation error".into()));
            }
            let mut folders = self.folders.lock().unwrap();
            if folders.iter().any(|f| f.name == name) {
                return Err(ImapError::Folder("Folder already exists".into()));
            }
            folders.push(Folder {
                name: name.to_string(),
                delimiter: Some("/".to_string()),
                attributes: HashSet::new(),
            });
            Ok(())
        }

        async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
            if self.should_fail {
                return Err(ImapError::Folder("Simulated folder deletion error".into()));
            }
            let mut folders = self.folders.lock().unwrap();
            if let Some(pos) = folders.iter().position(|f| f.name == name) {
                folders.remove(pos);
                Ok(())
            } else {
                Err(ImapError::Folder("Folder not found".into()))
            }
        }

        async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
            if self.should_fail { return Err(ImapError::Folder("Simulated rename error".into())); }
            let mut folders = self.folders.lock().unwrap();
            if let Some(folder) = folders.iter_mut().find(|f| f.name == from) {
                folder.name = to.to_string();
                Ok(())
            } else {
                Err(ImapError::Folder("Source folder not found".into()))
            }
        }

        async fn select_folder(&self, name: &str) -> Result<(), ImapError> {
            if self.should_fail { return Err(ImapError::Folder("Simulated select error".into())); }
            let folders = self.folders.lock().unwrap();
            if folders.iter().any(|f| f.name == name) {
                Ok(())
            } else {
                Err(ImapError::Folder("Folder not found".into()))
            }
        }

        async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
            if self.should_fail { return Err(ImapError::Email("Simulated search error".into())); }
            // Basic mock search - just return all UIDs
            Ok(self.emails.lock().unwrap().iter().map(|e| e.uid).collect())
        }

        async fn fetch_emails(&self, sequence_set: &str) -> Result<Vec<Email>, ImapError> {
            if self.should_fail { return Err(ImapError::Email("Simulated fetch error".into())); }
            // Basic mock fetch - return all emails matching the sequence (simplified)
            let uids: Vec<u32> = sequence_set.split(',').filter_map(|s| s.parse().ok()).collect();
            let emails = self.emails.lock().unwrap();
            Ok(emails.iter().filter(|e| uids.contains(&e.uid)).cloned().collect())
        }

        async fn move_email(&self, sequence_set: &str, destination_folder: &str) -> Result<(), ImapError> {
            if self.should_fail { return Err(ImapError::Email("Simulated move error".into())); }
            // Mock move - just check destination folder exists
            self.select_folder(destination_folder).await
        }

        async fn logout(&self) -> Result<(), ImapError> {
            if self.should_fail { return Err(ImapError::Connection("Simulated logout error".into())); }
            Ok(())
        }
    }

    fn create_test_client(session: MockImapSession) -> ImapClient {
        ImapClient::new(Box::new(session))
    }

    #[tokio::test]
    async fn test_list_folders_success() {
        let mut mock_session = MockImapSession::default();
        mock_session.folders.lock().unwrap().push(Folder {
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            attributes: HashSet::new(),
        });
        let client = create_test_client(mock_session);
        let result = client.list_folders().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_create_folder_success() {
        let mock_session = MockImapSession::default();
        let client = create_test_client(mock_session.clone());
        let result = client.create_folder("Sent").await;
        assert!(result.is_ok());
        assert_eq!(mock_session.folders.lock().unwrap().len(), 1);
        assert_eq!(mock_session.folders.lock().unwrap()[0].name, "Sent");
    }
    
    #[tokio::test]
    async fn test_create_folder_duplicate() {
        let mut mock_session = MockImapSession::default();
        mock_session.folders.lock().unwrap().push(Folder {
            name: "Sent".to_string(),
            delimiter: Some("/".to_string()),
            attributes: HashSet::new(),
        });
        let client = create_test_client(mock_session);
        let result = client.create_folder("Sent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImapError::Folder(_)));
    }

    #[tokio::test]
    async fn test_delete_folder_success() {
        let mut mock_session = MockImapSession::default();
        mock_session.folders.lock().unwrap().push(Folder {
            name: "Trash".to_string(),
            delimiter: Some("/".to_string()),
            attributes: HashSet::new(),
        });
        let client = create_test_client(mock_session.clone());
        let result = client.delete_folder("Trash").await;
        assert!(result.is_ok());
        assert!(mock_session.folders.lock().unwrap().is_empty());
    }
    
    #[tokio::test]
    async fn test_delete_folder_not_found() {
        let mock_session = MockImapSession::default();
        let client = create_test_client(mock_session);
        let result = client.delete_folder("NonExistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ImapError::Folder(_)));
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