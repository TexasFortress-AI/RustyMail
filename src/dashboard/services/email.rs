use std::sync::Arc;
use log::{info, error, debug, warn};
use crate::imap::error::ImapError;
use crate::imap::types::Email;
use crate::prelude::CloneableImapSessionFactory;
use crate::connection_pool::ConnectionPool;
use crate::dashboard::services::cache::{CacheService, CachedEmail};
use crate::dashboard::services::account::{AccountService, Account, AccountError};
use crate::dashboard::services::attachment_storage::{self, AttachmentInfo, AttachmentError};
use tokio::sync::Mutex as TokioMutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmailServiceError {
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapError),
    #[error("Connection pool error: {0}")]
    ConnectionError(String),
    #[error("No IMAP connection available")]
    NoConnection,
    #[error("Account error: {0}")]
    AccountError(#[from] AccountError),
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Attachment error: {0}")]
    AttachmentError(#[from] AttachmentError),
    #[error("Cache service not available")]
    CacheServiceNotAvailable,
}

pub struct EmailService {
    imap_factory: CloneableImapSessionFactory,
    connection_pool: Arc<ConnectionPool>,
    cache_service: Option<Arc<CacheService>>,
    account_service: Option<Arc<TokioMutex<AccountService>>>,
}

impl EmailService {
    pub fn new(imap_factory: CloneableImapSessionFactory, connection_pool: Arc<ConnectionPool>) -> Self {
        Self {
            imap_factory,
            connection_pool,
            cache_service: None,
            account_service: None,
        }
    }

    pub fn with_cache(mut self, cache_service: Arc<CacheService>) -> Self {
        self.cache_service = Some(cache_service);
        self
    }

    pub fn with_account_service(mut self, account_service: Arc<TokioMutex<AccountService>>) -> Self {
        self.account_service = Some(account_service);
        self
    }

    /// Get account by ID from AccountService
    async fn get_account(&self, account_id: &str) -> Result<Account, EmailServiceError> {
        let account_service = self.account_service.as_ref()
            .ok_or_else(|| EmailServiceError::AccountNotFound("Account service not available".to_string()))?;

        let account_service = account_service.lock().await;
        let account = account_service.get_account(account_id).await?;
        Ok(account)
    }

    /// List all folders for a specific account
    pub async fn list_folders_for_account(&self, account_id: &str) -> Result<Vec<String>, EmailServiceError> {
        debug!("Listing email folders for account: {}", account_id);

        // Get account credentials
        let account = self.get_account(account_id).await?;

        // Create session with account-specific credentials and record connection status
        let session = match self.imap_factory.create_session_for_account(&account).await {
            Ok(s) => {
                // Record successful IMAP connection
                if let Some(account_service) = &self.account_service {
                    let account_service = account_service.lock().await;
                    if let Err(e) = account_service.update_imap_status(account_id, true, format!("Successfully connected to {}", account.imap_host)).await {
                        warn!("Failed to update IMAP connection status: {}", e);
                    }
                }
                s
            }
            Err(e) => {
                // Record failed IMAP connection
                if let Some(account_service) = &self.account_service {
                    let account_service = account_service.lock().await;
                    if let Err(status_err) = account_service.update_imap_status(account_id, false, e.to_string()).await {
                        warn!("Failed to update IMAP connection status: {}", status_err);
                    }
                }
                return Err(EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)));
            }
        };

        // List folders
        let folders = session.list_folders().await?;

        info!("Listed {} folders for account {}", folders.len(), account_id);
        Ok(folders)
    }

    /// List all folders in the email account (uses default account)
    pub async fn list_folders(&self) -> Result<Vec<String>, EmailServiceError> {
        debug!("Listing email folders (default account)");

        // Get a session from the factory (uses .env credentials for backwards compatibility)
        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // List folders
        let folders = session.list_folders().await?;

        info!("Listed {} folders", folders.len());
        Ok(folders)
    }

    /// Search for emails in a specific folder for a specific account
    pub async fn search_emails_for_account(&self, folder: &str, criteria: &str, account_id: &str) -> Result<Vec<u32>, EmailServiceError> {
        debug!("Searching emails in folder '{}' with criteria: {} for account {}", folder, criteria, account_id);

        // Get account credentials
        let account = self.get_account(account_id).await?;

        // Create session with account-specific credentials
        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)))?;

        // Select the folder first
        session.select_folder(folder).await?;

        // Search for emails
        let uids = session.search_emails(criteria).await?;

        info!("Found {} emails matching criteria for account {}", uids.len(), account_id);
        Ok(uids)
    }

    /// Search for emails in a specific folder (uses default account)
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

    /// Fetch emails by their UIDs for a specific account
    pub async fn fetch_emails_for_account(&self, folder: &str, uids: &[u32], account_id: &str) -> Result<Vec<Email>, EmailServiceError> {
        debug!("Fetching {} emails from folder '{}' for account {}", uids.len(), folder, account_id);

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        // Get account credentials
        let account = self.get_account(account_id).await?;

        let mut emails = Vec::new();
        let mut uids_to_fetch = Vec::new();

        // Use the email address directly for caching
        let account_email = &account.email_address;

        // Check cache first if available
        if let Some(cache) = &self.cache_service {
            for &uid in uids {
                match cache.get_cached_email(folder, uid, account_email).await {
                    Ok(Some(cached_email)) => {
                        debug!("Email {} found in cache", uid);
                        emails.push(self.cached_email_to_email(cached_email));
                    }
                    Ok(None) => {
                        debug!("Email {} not in cache, will fetch from IMAP", uid);
                        uids_to_fetch.push(uid);
                    }
                    Err(e) => {
                        warn!("Cache error for email {}: {}", uid, e);
                        uids_to_fetch.push(uid);
                    }
                }
            }
        } else {
            uids_to_fetch = uids.to_vec();
        }

        // Fetch emails from IMAP
        if !uids_to_fetch.is_empty() {
            // Create session with connection status recording
            let session = match self.imap_factory.create_session_for_account(&account).await {
                Ok(s) => {
                    // Record successful IMAP connection
                    if let Some(account_service) = &self.account_service {
                        let account_service = account_service.lock().await;
                        if let Err(e) = account_service.update_imap_status(account_id, true, format!("Successfully connected to {} for fetch", account.imap_host)).await {
                            warn!("Failed to update IMAP connection status: {}", e);
                        }
                    }
                    s
                }
                Err(e) => {
                    // Record failed IMAP connection
                    if let Some(account_service) = &self.account_service {
                        let account_service = account_service.lock().await;
                        if let Err(status_err) = account_service.update_imap_status(account_id, false, e.to_string()).await {
                            warn!("Failed to update IMAP connection status: {}", status_err);
                        }
                    }
                    return Err(EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)));
                }
            };

            // Select the folder first
            session.select_folder(folder).await?;

            // Fetch the emails
            let fetched_emails = session.fetch_emails(&uids_to_fetch).await?;

            // Cache emails with account_id support
            if let Some(cache) = &self.cache_service {
                for email in &fetched_emails {
                    if let Err(e) = cache.cache_email(folder, email, account_email).await {
                        warn!("Failed to cache email {}: {}", email.uid, e);
                    }
                }
            }

            emails.extend(fetched_emails);
        }

        info!("Fetched {} emails for account {} ({} from cache, {} from IMAP)",
              emails.len(), account_id,
              uids.len() - uids_to_fetch.len(),
              uids_to_fetch.len());
        Ok(emails)
    }

    /// Fetch emails by their UIDs (uses default account)
    /// DEPRECATED: Use fetch_emails_for_account instead for proper multi-account support
    #[allow(dead_code)]
    pub async fn fetch_emails(&self, folder: &str, uids: &[u32]) -> Result<Vec<Email>, EmailServiceError> {
        debug!("Fetching {} emails from folder '{}'", uids.len(), folder);

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        // NOTE: This method doesn't support caching properly because it lacks account_id context
        // Fetching directly from IMAP without cache
        let session = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Select the folder first
        session.select_folder(folder).await?;

        // Fetch the emails
        let emails = session.fetch_emails(uids).await?;

        info!("Fetched {} emails from IMAP (no cache support)", emails.len());
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

    /// Convert CachedEmail to Email
    fn cached_email_to_email(&self, cached: CachedEmail) -> Email {
        use crate::imap::types::{Envelope, Address};

        // Reconstruct envelope from cached data
        let envelope = Some(Envelope {
            date: None, // Date string not stored separately
            subject: cached.subject.clone(),
            from: if let Some(from_str) = &cached.from_address {
                // Parse email address
                let parts: Vec<&str> = from_str.split('@').collect();
                vec![Address {
                    name: cached.from_name.clone(),
                    mailbox: parts.get(0).map(|s| s.to_string()),
                    host: parts.get(1).map(|s| s.to_string()),
                }]
            } else {
                Vec::new()
            },
            to: cached.to_addresses.iter().map(|addr| {
                let parts: Vec<&str> = addr.split('@').collect();
                Address {
                    name: None,
                    mailbox: parts.get(0).map(|s| s.to_string()),
                    host: parts.get(1).map(|s| s.to_string()),
                }
            }).collect(),
            cc: cached.cc_addresses.iter().map(|addr| {
                let parts: Vec<&str> = addr.split('@').collect();
                Address {
                    name: None,
                    mailbox: parts.get(0).map(|s| s.to_string()),
                    host: parts.get(1).map(|s| s.to_string()),
                }
            }).collect(),
            bcc: Vec::new(),
            reply_to: Vec::new(),
            in_reply_to: None,
            message_id: cached.message_id.clone(),
        });

        Email {
            uid: cached.uid,
            flags: cached.flags,
            internal_date: cached.internal_date,
            envelope,
            body: None, // Body loaded on demand
            mime_parts: Vec::new(),
            text_body: cached.body_text,
            html_body: cached.body_html,
            attachments: Vec::new(), // Attachments loaded separately
        }
    }

    /// Atomically move a single email from one folder to another
    pub async fn atomic_move_message(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), EmailServiceError> {
        debug!("Atomically moving email {} from {} to {}", uid, from_folder, to_folder);

        let client = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Use atomic operations - extract the session from the client
        let session = client.session_arc();
        let atomic_ops = crate::imap::atomic::AtomicImapOperations::new((*session).clone());
        atomic_ops.atomic_move(uid, from_folder, to_folder).await?;

        // Note: Cache will be invalidated naturally on next access
        info!("Successfully moved email {} from {} to {}", uid, from_folder, to_folder);
        Ok(())
    }

    /// Atomically move multiple emails from one folder to another
    pub async fn atomic_batch_move(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), EmailServiceError> {
        debug!("Atomically moving {} emails from {} to {}", uids.len(), from_folder, to_folder);

        let client = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        // Use atomic operations - extract the session from the client
        let session = client.session_arc();
        let atomic_ops = crate::imap::atomic::AtomicImapOperations::new((*session).clone());
        atomic_ops.atomic_batch_move(uids, from_folder, to_folder).await?;

        // Note: Cache will be invalidated naturally on next access
        info!("Successfully moved {} emails from {} to {}", uids.len(), from_folder, to_folder);
        Ok(())
    }

    /// Mark email(s) as deleted (sets \Deleted flag)
    pub async fn mark_as_deleted(&self, folder: &str, uids: &[u32]) -> Result<(), EmailServiceError> {
        debug!("Marking {} emails as deleted in {}", uids.len(), folder);

        let client = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        client.select_folder(folder).await?;
        client.mark_as_deleted(uids).await?;

        // Note: Cache will be invalidated naturally on next access
        info!("Successfully marked {} emails as deleted", uids.len());
        Ok(())
    }

    /// Permanently delete messages (mark as deleted and expunge)
    pub async fn delete_messages(&self, folder: &str, uids: &[u32]) -> Result<(), EmailServiceError> {
        debug!("Deleting {} messages in {}", uids.len(), folder);

        // First mark as deleted
        self.mark_as_deleted(folder, uids).await?;

        // Then expunge
        self.expunge(folder).await?;

        info!("Successfully deleted {} messages", uids.len());
        Ok(())
    }

    /// Permanently delete messages for a specific account with attachment cleanup
    /// This method ensures that when emails are deleted, their attachments are also removed
    pub async fn delete_messages_for_account(
        &self,
        folder: &str,
        uids: &[u32],
        account_id: &str,
    ) -> Result<(), EmailServiceError> {
        debug!("Deleting {} messages in {} for account {} with attachment cleanup",
               uids.len(), folder, account_id);

        if uids.is_empty() {
            return Ok(());
        }

        // Get account
        let account = self.get_account(account_id).await?;
        let account_email = &account.email_address;

        // Get database pool for attachment cleanup
        let db_pool = self.cache_service.as_ref()
            .and_then(|cache| cache.db_pool.as_ref());

        // If we have database access, clean up attachments
        if let Some(db_pool) = db_pool {
            // Fetch emails to get their message_ids
            let emails = self.fetch_emails_for_account(folder, uids, account_id).await?;

            // Delete attachments for each email
            for email in &emails {
                let message_id = attachment_storage::ensure_message_id(email, account_email);

                match attachment_storage::delete_attachments_for_email(db_pool, &message_id, account_email).await {
                    Ok(_) => {
                        debug!("Deleted attachments for email UID {} (message_id: {})", email.uid, message_id);
                    }
                    Err(e) => {
                        // Log warning but continue - attachment deletion shouldn't prevent email deletion
                        warn!("Failed to delete attachments for email UID {}: {}", email.uid, e);
                    }
                }
            }
        } else {
            warn!("Database not available for attachment cleanup");
        }

        // Delete emails from IMAP
        let client = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)))?;

        client.select_folder(folder).await?;
        client.mark_as_deleted(uids).await?;
        client.expunge().await?;

        info!("Successfully deleted {} messages with attachments for account {}", uids.len(), account_id);
        Ok(())
    }

    /// Remove \Deleted flag from messages
    pub async fn undelete_messages(&self, folder: &str, uids: &[u32]) -> Result<(), EmailServiceError> {
        debug!("Undeleting {} messages in {}", uids.len(), folder);

        let client = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        client.select_folder(folder).await?;
        client.undelete_messages(uids).await?;

        info!("Successfully undeleted {} messages", uids.len());
        Ok(())
    }

    /// Expunge deleted messages from a folder
    pub async fn expunge(&self, folder: &str) -> Result<(), EmailServiceError> {
        debug!("Expunging deleted messages from {}", folder);

        let client = self.imap_factory.create_session().await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session: {}", e)))?;

        client.select_folder(folder).await?;
        client.expunge().await?;

        info!("Successfully expunged messages from {}", folder);
        Ok(())
    }

    /// Create a new folder for a specific account
    pub async fn create_folder_for_account(&self, name: &str, account_id: &str) -> Result<(), EmailServiceError> {
        debug!("Creating folder '{}' for account {}", name, account_id);

        // Get account credentials
        let account = self.get_account(account_id).await?;

        // Create session with account-specific credentials
        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)))?;

        // Create the folder
        session.create_folder(name).await?;

        info!("Successfully created folder '{}' for account {}", name, account_id);
        Ok(())
    }

    /// Delete a folder for a specific account
    pub async fn delete_folder_for_account(&self, name: &str, account_id: &str) -> Result<(), EmailServiceError> {
        debug!("Deleting folder '{}' for account {}", name, account_id);

        // Get account credentials
        let account = self.get_account(account_id).await?;

        // Create session with account-specific credentials
        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)))?;

        // Delete the folder
        session.delete_folder(name).await?;

        info!("Successfully deleted folder '{}' for account {}", name, account_id);
        Ok(())
    }

    /// Rename a folder for a specific account
    pub async fn rename_folder_for_account(&self, old_name: &str, new_name: &str, account_id: &str) -> Result<(), EmailServiceError> {
        debug!("Renaming folder '{}' to '{}' for account {}", old_name, new_name, account_id);

        // Get account credentials
        let account = self.get_account(account_id).await?;

        // Create session with account-specific credentials
        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| EmailServiceError::ConnectionError(format!("Failed to create session for account {}: {}", account_id, e)))?;

        // Rename the folder
        session.rename_folder(old_name, new_name).await?;

        info!("Successfully renamed folder '{}' to '{}' for account {}", old_name, new_name, account_id);
        Ok(())
    }

    /// Fetch a single email with full body and save its attachments
    /// This is called when the user views an email (lazy loading)
    pub async fn fetch_email_with_attachments(
        &self,
        folder: &str,
        uid: u32,
        account_id: &str,
    ) -> Result<(Email, Vec<AttachmentInfo>), EmailServiceError> {
        debug!("Fetching email {} from folder '{}' with attachments for account {}", uid, folder, account_id);

        // Get account
        let account = self.get_account(account_id).await?;
        let account_email = &account.email_address;

        // Get database pool from cache service
        let db_pool = self.cache_service.as_ref()
            .and_then(|cache| cache.db_pool.as_ref())
            .ok_or(EmailServiceError::CacheServiceNotAvailable)?;

        // Fetch the email with full body
        let emails = self.fetch_emails_for_account(folder, &[uid], account_id).await?;
        let mut email = emails.into_iter().next()
            .ok_or_else(|| EmailServiceError::ConnectionError(format!("Email {} not found", uid)))?;

        // Ensure the email has a message_id (or generate one)
        let message_id = attachment_storage::ensure_message_id(&email, account_email);

        // Save attachments to filesystem and database
        let mut attachment_infos = Vec::new();
        for attachment in &email.attachments {
            match attachment_storage::save_attachment(
                db_pool,
                account_email,
                &message_id,
                attachment,
            ).await {
                Ok(info) => {
                    debug!("Saved attachment: {}", info.filename);
                    attachment_infos.push(info);
                }
                Err(e) => {
                    warn!("Failed to save attachment: {}", e);
                    // Continue processing other attachments even if one fails
                }
            }
        }

        // Update the email's envelope to ensure message_id is set
        if let Some(ref mut envelope) = email.envelope {
            if envelope.message_id.is_none() {
                envelope.message_id = Some(message_id.clone());
            }
        }

        info!("Fetched email {} with {} attachments for account {}", uid, attachment_infos.len(), account_id);
        Ok((email, attachment_infos))
    }
}