// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::fmt::Debug;
use async_trait::async_trait;
use crate::imap::error::ImapError;
use crate::imap::types::{Email, FlagOperation};

/// Mock implementation of AsyncImapOps for testing
#[derive(Debug, Clone)]
pub struct MockImapSession {
    // Configurable results for each operation
    pub list_folders_result: Result<Vec<String>, ImapError>,
    pub create_folder_result: Result<(), ImapError>,
    pub delete_folder_result: Result<(), ImapError>,
    pub rename_folder_result: Result<(), ImapError>,
    pub select_folder_result: Result<(), ImapError>,
    pub search_emails_result: Result<Vec<u32>, ImapError>,
    pub fetch_emails_result: Result<Vec<Email>, ImapError>,
    pub move_email_result: Result<(), ImapError>,
    pub store_flags_result: Result<(), ImapError>,
    pub append_result: Result<(), ImapError>,
    pub fetch_raw_message_result: Result<Vec<u8>, ImapError>,
    pub expunge_result: Result<(), ImapError>,
    pub login_result: Result<(), ImapError>,
    pub logout_result: Result<(), ImapError>,
}

impl MockImapSession {
    /// Create a new mock session with all operations succeeding
    pub fn new() -> Self {
        Self {
            list_folders_result: Ok(vec!["INBOX".to_string()]),
            create_folder_result: Ok(()),
            delete_folder_result: Ok(()),
            rename_folder_result: Ok(()),
            select_folder_result: Ok(()),
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
            expunge_result: Ok(()),
            login_result: Ok(()),
            logout_result: Ok(()),
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
            expunge_result: Err(ImapError::Connection("Connection failed".to_string())),
            login_result: Err(ImapError::Connection("Connection failed".to_string())),
            logout_result: Err(ImapError::Connection("Connection failed".to_string())),
        }
    }

    // Builder methods
    pub fn set_list_folders_result(mut self, result: Result<Vec<String>, ImapError>) -> Self {
        self.list_folders_result = result;
        self
    }

    pub fn set_create_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.create_folder_result = result;
        self
    }

    pub fn set_delete_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.delete_folder_result = result;
        self
    }

    pub fn set_rename_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.rename_folder_result = result;
        self
    }

    pub fn set_select_folder_result(mut self, result: Result<(), ImapError>) -> Self {
        self.select_folder_result = result;
        self
    }

    pub fn set_search_emails_result(mut self, result: Result<Vec<u32>, ImapError>) -> Self {
        self.search_emails_result = result;
        self
    }

    pub fn set_fetch_emails_result(mut self, result: Result<Vec<Email>, ImapError>) -> Self {
        self.fetch_emails_result = result;
        self
    }

    pub fn set_move_email_result(mut self, result: Result<(), ImapError>) -> Self {
        self.move_email_result = result;
        self
    }

    pub fn set_store_flags_result(mut self, result: Result<(), ImapError>) -> Self {
        self.store_flags_result = result;
        self
    }

    pub fn set_append_result(mut self, result: Result<(), ImapError>) -> Self {
        self.append_result = result;
        self
    }

    pub fn set_fetch_raw_message_result(mut self, result: Result<Vec<u8>, ImapError>) -> Self {
        self.fetch_raw_message_result = result;
        self
    }

    pub fn set_expunge_result(mut self, result: Result<(), ImapError>) -> Self {
        self.expunge_result = result;
        self
    }

    pub fn set_login_result(mut self, result: Result<(), ImapError>) -> Self {
        self.login_result = result;
        self
    }

    pub fn set_logout_result(mut self, result: Result<(), ImapError>) -> Self {
        self.logout_result = result;
        self
    }
}

#[async_trait]
impl AsyncImapOps for MockImapSession {
    async fn login(&self, _username: &str, _password: &str) -> Result<(), ImapError> {
        self.login_result.clone()
    }

    async fn logout(&self) -> Result<(), ImapError> {
        self.logout_result.clone()
    }

    async fn list_folders(&self) -> Result<Vec<String>, ImapError> {
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

    async fn select_folder(&self, _name: &str) -> Result<(), ImapError> {
        self.select_folder_result.clone()
    }

    async fn search_emails(&self, _criteria: &str) -> Result<Vec<u32>, ImapError> {
        self.search_emails_result.clone()
    }

    async fn fetch_emails(&self, _uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        self.fetch_emails_result.clone()
    }

    async fn move_email(&self, _uid: u32, _from_folder: &str, _to_folder: &str) -> Result<(), ImapError> {
        self.move_email_result.clone()
    }

    async fn store_flags(&self, _uids: &[u32], _operation: FlagOperation, _flags: &[String]) -> Result<(), ImapError> {
        self.store_flags_result.clone()
    }

    async fn append(&self, _folder: &str, _content: &[u8], _flags: &[String]) -> Result<(), ImapError> {
        self.append_result.clone()
    }

    async fn fetch_raw_message(&self, _uid: u32) -> Result<Vec<u8>, ImapError> {
        self.fetch_raw_message_result.clone()
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        self.expunge_result.clone()
    }
} 