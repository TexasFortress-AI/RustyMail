use std::sync::Arc;
use log::{info, error, debug};
use crate::imap::error::ImapError;
use crate::imap::types::Email;
use crate::prelude::CloneableImapSessionFactory;
use crate::connection_pool::ConnectionPool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmailServiceError {
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapError),
    #[error("Connection pool error: {0}")]
    ConnectionError(String),
    #[error("No IMAP connection available")]
    NoConnection,
}

pub struct EmailService {
    imap_factory: CloneableImapSessionFactory,
    connection_pool: Arc<ConnectionPool>,
}

impl EmailService {
    pub fn new(imap_factory: CloneableImapSessionFactory, connection_pool: Arc<ConnectionPool>) -> Self {
        Self {
            imap_factory,
            connection_pool,
        }
    }

    /// List all folders in the email account
    pub async fn list_folders(&self) -> Result<Vec<String>, EmailServiceError> {
        debug!("Listing email folders");

        // Get a session from the factory
        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // List folders
        let folders = session.list_folders().await?;

        info!("Listed {} folders", folders.len());
        Ok(folders)
    }

    /// Search for emails in a specific folder
    pub async fn search_emails(&self, folder: &str, criteria: &str) -> Result<Vec<u32>, EmailServiceError> {
        debug!("Searching emails in folder '{}' with criteria: {}", folder, criteria);

        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Select the folder first
        session.select_folder(folder).await?;

        // Search for emails
        let uids = session.search_emails(criteria).await?;

        info!("Found {} emails matching criteria", uids.len());
        Ok(uids)
    }

    /// Fetch emails by their UIDs
    pub async fn fetch_emails(&self, folder: &str, uids: &[u32]) -> Result<Vec<Email>, EmailServiceError> {
        debug!("Fetching {} emails from folder '{}'", uids.len(), folder);

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Select the folder first
        session.select_folder(folder).await?;

        // Fetch the emails
        let emails = session.fetch_emails(uids).await?;

        info!("Fetched {} emails", emails.len());
        Ok(emails)
    }

    /// Get recent emails from inbox
    pub async fn get_recent_inbox_emails(&self, limit: usize) -> Result<Vec<Email>, EmailServiceError> {
        debug!("Getting recent {} emails from INBOX", limit);

        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Select INBOX
        session.select_folder("INBOX").await?;

        // Search for all emails (or use a more specific criteria)
        let all_uids = session.search_emails("ALL").await?;

        if all_uids.is_empty() {
            info!("No emails found in INBOX");
            return Ok(Vec::new());
        }

        // Get the most recent UIDs (last N from the list)
        let recent_uids: Vec<u32> = all_uids.iter()
            .rev()
            .take(limit)
            .copied()
            .collect();

        // Fetch the emails
        let emails = session.fetch_emails(&recent_uids).await?;

        info!("Fetched {} recent emails from INBOX", emails.len());
        Ok(emails)
    }

    /// Get unread emails from inbox
    pub async fn get_unread_emails(&self) -> Result<Vec<Email>, EmailServiceError> {
        debug!("Getting unread emails from INBOX");

        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Select INBOX
        session.select_folder("INBOX").await?;

        // Search for unread emails
        let unread_uids = session.search_emails("UNSEEN").await?;

        if unread_uids.is_empty() {
            info!("No unread emails found");
            return Ok(Vec::new());
        }

        // Fetch the emails
        let emails = session.fetch_emails(&unread_uids).await?;

        info!("Fetched {} unread emails", emails.len());
        Ok(emails)
    }
}