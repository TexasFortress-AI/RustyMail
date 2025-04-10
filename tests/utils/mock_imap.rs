use std::sync::Arc;
use tokio::sync::Mutex;
use crate::imap::error::ImapError;
use crate::imap::types::{Email, Folder, MailboxInfo};
use crate::imap::session::AsyncImapOps;
use crate::imap::types::FlagOperation;

/// Mock implementation of AsyncImapOps for testing
#[derive(Clone)]
pub struct MockImapSession {
    // Configurable results for each operation
    pub list_folders_result: Result<Vec<Folder>, ImapError>,
    pub create_folder_result: Result<(), ImapError>,
    pub delete_folder_result: Result<(), ImapError>,
    pub rename_folder_result: Result<(), ImapError>,
    pub select_folder_result: Result<MailboxInfo, ImapError>,
    pub search_emails_result: Result<Vec<u32>, ImapError>,
    pub fetch_emails_result: Result<Vec<Email>, ImapError>,
    pub move_email_result: Result<(), ImapError>,
    pub store_flags_result: Result<(), ImapError>,
    pub append_result: Result<(), ImapError>,
    pub fetch_raw_message_result: Result<Vec<u8>, ImapError>,
}

impl MockImapSession {
    /// Create a new mock session with all operations succeeding
    pub fn new() -> Self {
        Self {
            list_folders_result: Ok(vec![Folder {
                name: "INBOX".to_string(),
                delimiter: '/',
            }]),
            create_folder_result: Ok(()),
            delete_folder_result: Ok(()),
            rename_folder_result: Ok(()),
            select_folder_result: Ok(MailboxInfo {
                name: "INBOX".to_string(),
                delimiter: '/',
                attributes: vec![],
                exists: 0,
                recent: 0,
                unseen: 0,
            }),
            search_emails_result: Ok(vec![1, 2, 3]),
            fetch_emails_result: Ok(vec![Email {
                uid: 1,
                flags: vec![],
                internal_date: None,
                envelope: None,
                body: None,
            }]),
            move_email_result: Ok(()),
            store_flags_result: Ok(()),
            append_result: Ok(()),
            fetch_raw_message_result: Ok(vec![]),
        }
    }

    /// Create a new mock session that fails all operations
    pub fn new_failing() -> Self {
        Self {
            list_folders_result: Err(ImapError::Connection("Connection failed".to_string())),
            create_folder_result: Err(ImapError::Connection("Connection failed".to_string())),
            delete_folder_result: Err(ImapError::Connection("Connection failed".to_string())),
            rename_folder_result: Err(ImapError::Connection("Connection failed".to_string())),
            select_folder_result: Err(ImapError::Connection("Connection failed".to_string())),
            search_emails_result: Err(ImapError::Connection("Connection failed".to_string())),
            fetch_emails_result: Err(ImapError::Connection("Connection failed".to_string())),
            move_email_result: Err(ImapError::Connection("Connection failed".to_string())),
            store_flags_result: Err(ImapError::Connection("Connection failed".to_string())),
            append_result: Err(ImapError::Connection("Connection failed".to_string())),
            fetch_raw_message_result: Err(ImapError::Connection("Connection failed".to_string())),
        }
    }

    /// Builder method to set list_folders_result
    pub fn set_list_folders_result(mut self, result: Result<Vec<Folder>, ImapError>) -> Self {
        self.list_folders_result = result;
        self
    }

    /// Builder method to set create_folder_result
    pub fn set_create_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.create_folder_result = result;
        self
    }

    /// Builder method to set delete_folder_result
    pub fn set_delete_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.delete_folder_result = result;
        self
    }

    /// Builder method to set rename_folder_result
    pub fn set_rename_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.rename_folder_result = result;
        self
    }

    /// Builder method to set select_folder_result
    pub fn set_select_folder_result(mut self, result: Result<MailboxInfo, ImapError>) -> Self {
        self.select_folder_result = result;
        self
    }

    /// Builder method to set search_emails_result
    pub fn set_search_emails_result(mut self, result: Result<Vec<u32>, ImapError>) -> Self {
        self.search_emails_result = result;
        self
    }

    /// Builder method to set fetch_emails_result
    pub fn set_fetch_emails_result(mut self, result: Result<Vec<Email>, ImapError>) -> Self {
        self.fetch_emails_result = result;
        self
    }

    /// Builder method to set move_email_result
    pub fn set_move_email_result(mut self, result: Result<(), ImapError>) -> Self {
        self.move_email_result = result;
        self
    }

    /// Builder method to set store_flags_result
    pub fn set_store_flags_result(mut self, result: Result<(), ImapError>) -> Self {
        self.store_flags_result = result;
        self
    }

    /// Builder method to set append_result
    pub fn set_append_result(mut self, result: Result<(), ImapError>) -> Self {
        self.append_result = result;
        self
    }

    /// Builder method to set fetch_raw_message_result
    pub fn set_fetch_raw_message_result(mut self, result: Result<Vec<u8>, ImapError>) -> Self {
        self.fetch_raw_message_result = result;
        self
    }
}

#[async_trait::async_trait]
impl AsyncImapOps for MockImapSession {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        self.list_folders_result.clone()
    }

    async fn create_folder(&self, _name: &str) -> Result<(), ImapError> {
        self.create_folder_result.clone()
    }

    async fn delete_folder(&self, _name: &str) -> Result<(), ImapError> {
        self.delete_folder_result.clone()
    }

    async fn rename_folder(&self, _old_name: &str, _new_name: &str) -> Result<(), ImapError> {
        self.rename_folder_result.clone()
    }

    async fn select_folder(&self, _name: &str) -> Result<MailboxInfo, ImapError> {
        self.select_folder_result.clone()
    }

    async fn search_emails(&self, _criteria: &str) -> Result<Vec<u32>, ImapError> {
        self.search_emails_result.clone()
    }

    async fn fetch_emails(&self, _uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        self.fetch_emails_result.clone()
    }

    async fn move_email(&self, _uid: u32, _target_folder: &str) -> Result<(), ImapError> {
        self.move_email_result.clone()
    }

    async fn store_flags(&self, _uid: u32, _operation: FlagOperation, _flags: &str) -> Result<(), ImapError> {
        self.store_flags_result.clone()
    }

    async fn append(&self, _folder: &str, _message: &[u8], _flags: &[&str]) -> Result<(), ImapError> {
        self.append_result.clone()
    }

    async fn fetch_raw_message(&self, _uid: u32) -> Result<Vec<u8>, ImapError> {
        self.fetch_raw_message_result.clone()
    }
} 