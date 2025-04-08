use async_trait::async_trait;
use crate::imap::client::ImapClientTrait;
use crate::prelude::*;
// Imports needed by MockImapClient struct/impl outside tests
use crate::imap::types::{MailboxInfo, FlagOperation, Flags};
use crate::imap::session::StoreOperation;

#[cfg(test)]
mod tests {
    // Add all necessary type imports inside the test module
    use crate::imap::types::{Folder, MailboxInfo, Email, SearchCriteria};
    use crate::imap::session::{ImapSession, StoreOperation};
    // Keep other existing imports within tests
    use crate::imap::error::ImapError;
    use async_trait::async_trait;
    use std::sync::{ atomic::{AtomicBool, Ordering}, Arc };

    // --- Mock IMAP Session ---
    #[derive(Debug, Default)]
    struct MockCallTracker {
        // Restore tracker fields
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

    #[derive(Debug)]
    pub struct MockImapSession {
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
        expunge_result: Result<(), ImapError>, // Ensure correct type
        fetch_raw_result: Result<Vec<u8>, ImapError>,
    }

    impl Default for MockImapSession {
        fn default() -> Self {
            // Ensure initialization uses correct types and values
            Self {
                tracker: Arc::new(MockCallTracker::default()),
                list_folders_result: Ok(vec![ Folder { name: "INBOX".to_string(), delimiter: Some("/".to_string()) }, Folder { name: "Sent".to_string(), delimiter: Some("/".to_string()) }, ]),
                select_folder_result: Ok(MailboxInfo { flags: vec!["\\Seen".to_string()], exists: 10, recent: 1, unseen: Some(5), permanent_flags: vec!["\\".to_string()], uid_next: Some(101), uid_validity: Some(12345), }),
                search_emails_result: Ok(vec![1, 2, 3]),
                fetch_emails_result: Ok(vec![Email { uid: 1, flags: vec![], size: Some(100), envelope: None, body: None, }]),
                create_result: Ok(()),
                delete_result: Ok(()),
                rename_result: Ok(()),
                move_result: Ok(()),
                logout_result: Ok(()),
                store_flags_result: Ok(()),
                append_result: Ok(()),
                expunge_result: Ok(()), // Correct default value
                fetch_raw_result: Ok(Vec::new()),
             }
        }
    }
    
    impl MockImapSession {
        // Restore helper methods
    }

    #[async_trait]
    impl ImapSession for MockImapSession {
        async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
            self.tracker.list_folders_called.store(true, Ordering::SeqCst);
            match &self.list_folders_result {
                Ok(v) => Ok(v.clone()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn create_folder(&self, _name: &str) -> Result<(), ImapError> {
            self.tracker.create_folder_called.store(true, Ordering::SeqCst);
            match &self.create_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn delete_folder(&self, _name: &str) -> Result<(), ImapError> {
             self.tracker.delete_folder_called.store(true, Ordering::SeqCst);
             match &self.delete_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn rename_folder(&self, _from: &str, _to: &str) -> Result<(), ImapError> {
             self.tracker.rename_folder_called.store(true, Ordering::SeqCst);
             match &self.rename_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn select_folder(&self, _name: &str) -> Result<MailboxInfo, ImapError> {
            self.tracker.select_folder_called.store(true, Ordering::SeqCst);
            match &self.select_folder_result {
                Ok(v) => Ok(v.clone()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn search_emails(&self, _criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
            self.tracker.search_emails_called.store(true, Ordering::SeqCst);
            match &self.search_emails_result {
                Ok(v) => Ok(v.clone()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn fetch_emails(&self, _uids: Vec<u32>, _fetch_body: bool) -> Result<Vec<Email>, ImapError> {
            self.tracker.fetch_emails_called.store(true, Ordering::SeqCst);
            match &self.fetch_emails_result {
                Ok(v) => Ok(v.clone()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn fetch_raw_message(&mut self, _uid: u32) -> Result<Vec<u8>, ImapError> {
            match &self.fetch_raw_result {
                Ok(v) => Ok(v.clone()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn move_email( &self, _source_folder: &str, _uids: Vec<u32>, _destination_folder: &str, ) -> Result<(), ImapError> {
            self.tracker.move_email_called.store(true, Ordering::SeqCst);
            match &self.move_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn store_flags(&self, _uids: Vec<u32>, _operation: StoreOperation, _flags: Vec<String>) -> Result<(), ImapError> {
            self.tracker.store_flags_called.store(true, Ordering::SeqCst);
            match &self.store_flags_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn append(&self, _folder: &str, _payload: Vec<u8>) -> Result<(), ImapError> {
            self.tracker.append_called.store(true, Ordering::SeqCst);
             match &self.append_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn expunge(&self) -> Result<(), ImapError> {
            self.tracker.expunge_called.store(true, Ordering::SeqCst);
            match &self.expunge_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
        async fn logout(&self) -> Result<(), ImapError> {
            self.tracker.logout_called.store(true, Ordering::SeqCst);
            match &self.logout_result {
                Ok(_) => Ok(()),
                Err(_) => Err(ImapError::OperationFailed("Mock configured to fail".to_string()))
            }
        }
    }

    // --- Test Cases ---
    // ... tests using MockImapSession ...
}

// --- MockImapClient Definition --- 
#[derive(Debug, Default)]
pub struct MockImapClient {
    fail_flags: std::collections::HashSet<String>,
}

impl MockImapClient {
    pub fn set_fail(mut self, method_name: &str) -> Self {
        self.fail_flags.insert(method_name.to_string());
        self
    }

    fn should_fail(&self, method_name: &str) -> bool {
        self.fail_flags.contains(method_name)
    }

    fn mock_error(&self, method_name: &str) -> ImapError {
        ImapError::OperationFailed(format!("Mock {} failed as configured", method_name))
    }
}

#[async_trait]
impl ImapClientTrait for MockImapClient {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        if self.should_fail("list_folders") { Err(self.mock_error("list_folders")) }
        else { Ok(vec![ Folder { name: "INBOX".into(), delimiter: Some("/".into()) } ]) }
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        if self.should_fail("create_folder") { Err(self.mock_error(&format!("create_folder_{}", name))) }
        else { Ok(()) }
    }

    async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        if self.should_fail("delete_folder") { Err(self.mock_error(&format!("delete_folder_{}", name))) }
        else { Ok(()) }
    }

    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        if self.should_fail("rename_folder") { Err(self.mock_error(&format!("rename_folder_{}_{}", from, to))) }
        else { Ok(()) }
    }
    
    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError> {
        if self.should_fail("select_folder") { Err(self.mock_error(&format!("select_folder_{}", name))) }
        else { Ok(MailboxInfo { flags: vec![], exists: 1, recent: 0, unseen: Some(0), permanent_flags: vec![], uid_next: Some(1), uid_validity: Some(1) }) }
    }

    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        if self.should_fail("search_emails") { Err(self.mock_error(&format!("search_emails_{:?}", criteria))) }
        else { Ok(vec![1, 2]) } // Default UIDs
    }

    async fn fetch_emails(&self, uids: Vec<u32>, fetch_body: bool) -> Result<Vec<Email>, ImapError> {
        if self.should_fail("fetch_emails") { Err(self.mock_error(&format!("fetch_emails_{:?}_{}", uids, fetch_body))) }
        else { Ok(uids.into_iter().map(|uid| Email { uid, flags: vec![], size: Some(100), envelope: None, body: if fetch_body { Some(vec![]) } else { None } }).collect()) }
    }

    async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        if self.should_fail("move_email") { Err(self.mock_error(&format!("move_email_{}_{:?}_{}", source_folder, uids, destination_folder))) }
        else { Ok(()) }
    }

    async fn store_flags(&self, uids: Vec<u32>, operation: StoreOperation, flags: Vec<String>) -> Result<(), ImapError> {
        let mock_op = match operation { 
            StoreOperation::Add => FlagOperation::Add,
            StoreOperation::Remove => FlagOperation::Remove,
            StoreOperation::Set => FlagOperation::Set,
        };
        let mock_flags = Flags { items: flags }; 
        if self.should_fail("store_flags") { Err(self.mock_error(&format!("store_flags_{:?}_{:?}_{:?}", uids, mock_op, mock_flags))) }
        else { Ok(()) }
    }

    async fn append(&self, folder: &str, payload: Vec<u8>) -> Result<(), ImapError> {
        if self.should_fail("append") { Err(self.mock_error(&format!("append_{}_{:?}", folder, payload.len()))) }
        else { Ok(()) }
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        if self.should_fail("expunge") { Err(self.mock_error("expunge")) }
        else { Ok(()) }
    }

    async fn logout(&self) -> Result<(), ImapError> {
        if self.should_fail("logout") { Err(self.mock_error("logout")) }
        else { Ok(()) }
    }
} 