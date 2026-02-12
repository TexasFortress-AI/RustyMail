// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tokio::sync::Mutex as TokioMutex;
use log::{info, error, debug, warn};
use crate::imap::error::ImapError;
use crate::prelude::CloneableImapSessionFactory;
use crate::dashboard::services::cache::{CacheService, SyncStatus};
use crate::dashboard::services::account::AccountService;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapError),
    #[error("Cache error: {0}")]
    CacheError(String),
    #[error("Sync cancelled")]
    Cancelled,
    #[error("Account error: {0}")]
    AccountError(String),
}

pub struct SyncService {
    imap_factory: CloneableImapSessionFactory,
    cache_service: Arc<CacheService>,
    account_service: Arc<TokioMutex<AccountService>>,
    sync_interval: Duration,
}

impl SyncService {
    pub fn new(
        imap_factory: CloneableImapSessionFactory,
        cache_service: Arc<CacheService>,
        account_service: Arc<TokioMutex<AccountService>>,
        sync_interval_seconds: u64,
    ) -> Self {
        Self {
            imap_factory,
            cache_service,
            account_service,
            sync_interval: Duration::from_secs(sync_interval_seconds),
        }
    }

    /// Start the background sync task
    pub fn start_background_sync(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = time::interval(self.sync_interval);
            interval.tick().await; // Skip the first immediate tick

            loop {
                interval.tick().await;

                // Get all accounts for background sync
                let account_service = self.account_service.lock().await;
                match account_service.list_accounts().await {
                    Ok(accounts) => {
                        let account_emails: Vec<String> = accounts.iter()
                            .map(|a| a.email_address.clone())
                            .collect();
                        drop(account_service); // Release lock before sync

                        if account_emails.is_empty() {
                            debug!("No accounts configured, skipping background sync");
                            continue;
                        }

                        // Sync all accounts
                        for account_email in account_emails {
                            if let Err(e) = self.sync_all_folders(&account_email).await {
                                error!("Background sync failed for account {}: {}", account_email, e);
                            }
                        }
                    }
                    Err(e) => {
                        drop(account_service);
                        error!("Failed to list accounts for background sync: {}", e);
                    }
                }
            }
        })
    }

    /// Sync all folders for a specific account
    pub async fn sync_all_folders(&self, account_id: &str) -> Result<(), SyncError> {
        info!("Starting email sync for all folders for account: {}", account_id);

        // Get account credentials
        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service); // Release lock before creating session

        // Try to create session and record connection status
        let session = match self.imap_factory.create_session_for_account(&account).await {
            Ok(s) => {
                // Record successful IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(e) = account_service.update_imap_status(account_id, true, format!("Successfully connected to {}", account.imap_host)).await {
                    warn!("Failed to update IMAP connection status: {}", e);
                }
                drop(account_service);
                s
            }
            Err(e) => {
                // Record failed IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(status_err) = account_service.update_imap_status(account_id, false, e.to_string()).await {
                    warn!("Failed to update IMAP connection status: {}", status_err);
                }
                drop(account_service);
                return Err(SyncError::ImapError(e));
            }
        };

        let folders = session.list_folders().await?;

        // IMPORTANT: Reuse the same session for all folders to prevent memory leak
        // Previously, each folder created its own session with separate BytePools
        for folder in folders {
            if let Err(e) = self.sync_folder_with_session(account_id, &folder, &session).await {
                warn!("Failed to sync folder {} for account {}: {}", folder, account_id, e);
                // Continue with other folders even if one fails
            }
        }

        // IMPORTANT: Explicitly logout to ensure the session and its BytePool are freed
        if let Err(e) = session.logout().await {
            warn!("Failed to logout IMAP session: {}", e);
        }

        info!("Email sync completed for all folders for account: {}", account_id);
        Ok(())
    }

    /// Sync a specific folder for a specific account
    pub async fn sync_folder(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        self.sync_folder_with_limit(account_id, folder_name, None).await
    }

    /// Sync a specific folder with a provided session (to prevent creating multiple sessions)
    async fn sync_folder_with_session(&self, account_id: &str, folder_name: &str, session: &crate::imap::client::ImapClient<crate::imap::session::AsyncImapSessionWrapper>) -> Result<(), SyncError> {
        self.sync_folder_with_session_and_limit(account_id, folder_name, session, None).await
    }

    /// Sync a specific folder with optional limit for a specific account
    pub async fn sync_folder_with_limit(&self, account_id: &str, folder_name: &str, limit: Option<usize>) -> Result<(), SyncError> {
        debug!("Syncing folder: {} for account: {} (limit: {:?})", folder_name, account_id, limit);

        // Get account credentials first (need account_email for sync state)
        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service); // Release lock before creating session

        // Use the email address directly as the account ID
        let account_email = &account.email_address;

        // Update sync status
        if let Err(e) = self.cache_service.update_sync_state(folder_name, 0, SyncStatus::Syncing, account_email).await {
            warn!("Failed to update sync state: {}", e);
        }

        // Try to create session and record connection status
        let session = match self.imap_factory.create_session_for_account(&account).await {
            Ok(s) => {
                // Record successful IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(e) = account_service.update_imap_status(account_id, true, format!("Successfully connected to {} for sync", account.imap_host)).await {
                    warn!("Failed to update IMAP connection status: {}", e);
                }
                drop(account_service);
                s
            }
            Err(e) => {
                // Record failed IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(status_err) = account_service.update_imap_status(account_id, false, e.to_string()).await {
                    warn!("Failed to update IMAP connection status: {}", status_err);
                }
                drop(account_service);
                return Err(SyncError::ImapError(e));
            }
        };

        // Select the folder
        session.select_folder(folder_name).await?;

        // Ensure folder exists in database with correct account_id BEFORE caching emails
        // This prevents FOREIGN KEY constraint failures
        if let Err(e) = self.cache_service.get_or_create_folder_for_account(folder_name, account_email).await {
            error!("Failed to create folder {} for account {}: {}", folder_name, account_email, e);
            return Err(SyncError::CacheError(format!("Failed to create folder: {}", e)));
        }

        // Get the sync state from cache
        let sync_state = self.cache_service.get_sync_state(folder_name, account_email).await
            .map_err(|e| SyncError::CacheError(e.to_string()))?;

        let last_uid_synced = sync_state.and_then(|s| s.last_uid_synced).unwrap_or(0);

        // Search for new emails since last sync
        let search_criteria = if last_uid_synced > 0 {
            format!("UID {}:*", last_uid_synced + 1)
        } else {
            // First sync - get ALL emails
            "ALL".to_string()
        };

        let mut uids = session.search_emails(&search_criteria).await?;

        if uids.is_empty() {
            debug!("No new emails to sync in folder {}", folder_name);
            if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid_synced, SyncStatus::Idle, account_email).await {
                warn!("Failed to update sync state: {}", e);
            }
            return Ok(());
        }

        // Apply limit if specified
        let uids_to_sync: Vec<u32> = if let Some(batch_size) = limit {
            if uids.len() > batch_size {
                // For limited sync, take only the most recent emails
                uids.sort_unstable();
                uids.into_iter().rev().take(batch_size).collect()
            } else {
                uids
            }
        } else {
            // No limit - sync all emails
            uids
        };

        info!("Syncing {} emails in folder {}", uids_to_sync.len(), folder_name);

        // Process in batches to avoid memory issues
        const FETCH_BATCH_SIZE: usize = 100;
        let mut last_uid = last_uid_synced;

        for chunk in uids_to_sync.chunks(FETCH_BATCH_SIZE) {
            debug!("Fetching batch of {} emails", chunk.len());

            // Fetch and cache emails
            let emails = session.fetch_emails(chunk).await?;

            // Log memory size of fetched emails for debugging
            let total_size: usize = emails.iter()
                .map(|e| {
                    e.body.as_ref().map_or(0, |b| b.len()) +
                    e.text_body.as_ref().map_or(0, |s| s.len()) +
                    e.html_body.as_ref().map_or(0, |s| s.len()) +
                    e.mime_parts.iter().map(|p| p.body.len()).sum::<usize>() +
                    e.attachments.iter().map(|a| a.body.len()).sum::<usize>()
                })
                .sum();
            debug!("Fetched {} emails with total memory footprint: {} MB",
                   emails.len(), total_size as f64 / 1024.0 / 1024.0);

            // Track which UIDs were actually fetched
            let fetched_uids: Vec<u32> = emails.iter().map(|e| e.uid).collect();
            let missing_uids: Vec<u32> = chunk.iter()
                .filter(|uid| !fetched_uids.contains(uid))
                .copied()
                .collect();

            // Retry missing UIDs individually if there are any
            if !missing_uids.is_empty() {
                warn!("Retrying {} missing UIDs individually: {:?}", missing_uids.len(), missing_uids);
                for uid in missing_uids {
                    match session.fetch_emails(&[uid]).await {
                        Ok(retry_emails) => {
                            for email in retry_emails {
                                if let Err(e) = self.cache_service.cache_email(folder_name, &email, account_email).await {
                                    error!("Failed to cache retried email {}: {}", email.uid, e);
                                } else {
                                    debug!("Successfully fetched and cached previously missing UID: {}", uid);
                                    if email.uid > last_uid {
                                        last_uid = email.uid;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch UID {} even after retry: {}", uid, e);
                        }
                    }
                }
            }

            // Process emails by reference first
            for email in &emails {
                if let Err(e) = self.cache_service.cache_email(folder_name, email, account_email).await {
                    error!("Failed to cache email {}: {}", email.uid, e);
                } else {
                    if email.uid > last_uid {
                        last_uid = email.uid;
                    }
                }
            }

            // EXPLICITLY DROP emails vector to ensure memory is freed
            // This should trigger Drop for all Email structs and their Vec<u8> fields
            debug!("Dropping email batch - should free {} MB", total_size as f64 / 1024.0 / 1024.0);
            drop(emails);
        }

        // Update sync state with the highest UID synced
        if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid, SyncStatus::Idle, account_email).await {
            warn!("Failed to update sync state: {}", e);
        }

        // IMPORTANT: Explicitly logout to ensure the session and its BytePool are freed
        // This method creates its own session, so we must logout
        if let Err(e) = session.logout().await {
            warn!("Failed to logout IMAP session: {}", e);
        }

        info!("Successfully synced {} emails in folder {}", uids_to_sync.len(), folder_name);
        Ok(())
    }

    /// Sync a specific folder with a provided session and optional limit
    /// This is used internally to reuse the same IMAP session across folders
    async fn sync_folder_with_session_and_limit(&self, account_id: &str, folder_name: &str, session: &crate::imap::client::ImapClient<crate::imap::session::AsyncImapSessionWrapper>, limit: Option<usize>) -> Result<(), SyncError> {
        debug!("Syncing folder: {} for account: {} with shared session (limit: {:?})", folder_name, account_id, limit);

        // Get account credentials first (need account_email for sync state)
        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service); // Release lock

        // Use the email address directly as the account ID
        let account_email = &account.email_address;

        // Update sync status
        if let Err(e) = self.cache_service.update_sync_state(folder_name, 0, SyncStatus::Syncing, account_email).await {
            warn!("Failed to update sync state: {}", e);
        }

        // Select the folder
        session.select_folder(folder_name).await?;

        // Ensure folder exists in database with correct account_id BEFORE caching emails
        if let Err(e) = self.cache_service.get_or_create_folder_for_account(folder_name, account_email).await {
            error!("Failed to create folder {} for account {}: {}", folder_name, account_email, e);
            return Err(SyncError::CacheError(format!("Failed to create folder: {}", e)));
        }

        // Get the sync state from cache
        let sync_state = self.cache_service.get_sync_state(folder_name, account_email).await
            .map_err(|e| SyncError::CacheError(e.to_string()))?;

        let last_uid_synced = sync_state.and_then(|s| s.last_uid_synced).unwrap_or(0);

        // Search for new emails since last sync
        let search_criteria = if last_uid_synced > 0 {
            format!("UID {}:*", last_uid_synced + 1)
        } else {
            // First sync - get ALL emails
            "ALL".to_string()
        };

        let mut uids = session.search_emails(&search_criteria).await?;

        if uids.is_empty() {
            debug!("No new emails to sync in folder {}", folder_name);
            if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid_synced, SyncStatus::Idle, account_email).await {
                warn!("Failed to update sync state: {}", e);
            }
            return Ok(());
        }

        // Apply limit if specified
        let uids_to_sync: Vec<u32> = if let Some(batch_size) = limit {
            if uids.len() > batch_size {
                // For limited sync, take only the most recent emails
                uids.sort_unstable();
                uids.into_iter().rev().take(batch_size).collect()
            } else {
                uids
            }
        } else {
            // No limit - sync all emails
            uids
        };

        info!("Syncing {} emails in folder {}", uids_to_sync.len(), folder_name);

        // Process in batches to avoid memory issues
        const FETCH_BATCH_SIZE: usize = 100;
        let mut last_uid = last_uid_synced;

        for chunk in uids_to_sync.chunks(FETCH_BATCH_SIZE) {
            debug!("Fetching batch of {} emails", chunk.len());

            // Fetch and cache emails
            let emails = session.fetch_emails(chunk).await?;

            // Log memory size of fetched emails for debugging
            let total_size: usize = emails.iter()
                .map(|e| {
                    e.body.as_ref().map_or(0, |b| b.len()) +
                    e.text_body.as_ref().map_or(0, |s| s.len()) +
                    e.html_body.as_ref().map_or(0, |s| s.len()) +
                    e.mime_parts.iter().map(|p| p.body.len()).sum::<usize>() +
                    e.attachments.iter().map(|a| a.body.len()).sum::<usize>()
                })
                .sum();
            debug!("Fetched {} emails with total memory footprint: {} MB",
                   emails.len(), total_size as f64 / 1024.0 / 1024.0);

            // Track which UIDs were actually fetched
            let fetched_uids: Vec<u32> = emails.iter().map(|e| e.uid).collect();
            let missing_uids: Vec<u32> = chunk.iter()
                .filter(|uid| !fetched_uids.contains(uid))
                .copied()
                .collect();

            // Retry missing UIDs individually if there are any
            if !missing_uids.is_empty() {
                warn!("Retrying {} missing UIDs individually: {:?}", missing_uids.len(), missing_uids);
                for uid in missing_uids {
                    match session.fetch_emails(&[uid]).await {
                        Ok(retry_emails) => {
                            for email in retry_emails {
                                if let Err(e) = self.cache_service.cache_email(folder_name, &email, account_email).await {
                                    error!("Failed to cache retried email {}: {}", email.uid, e);
                                } else {
                                    debug!("Successfully fetched and cached previously missing UID: {}", uid);
                                    if email.uid > last_uid {
                                        last_uid = email.uid;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch UID {} even after retry: {}", uid, e);
                        }
                    }
                }
            }

            // Process emails by reference first
            for email in &emails {
                if let Err(e) = self.cache_service.cache_email(folder_name, email, account_email).await {
                    error!("Failed to cache email {}: {}", email.uid, e);
                } else {
                    if email.uid > last_uid {
                        last_uid = email.uid;
                    }
                }
            }

            // EXPLICITLY DROP emails vector to ensure memory is freed
            // This should trigger Drop for all Email structs and their Vec<u8> fields
            debug!("Dropping email batch - should free {} MB", total_size as f64 / 1024.0 / 1024.0);
            drop(emails);
        }

        // Update sync state with the highest UID synced
        if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid, SyncStatus::Idle, account_email).await {
            warn!("Failed to update sync state: {}", e);
        }

        info!("Successfully synced {} emails in folder {}", uids_to_sync.len(), folder_name);
        Ok(())
    }

    /// Perform a full sync of a folder (clear cache and re-download) for a specific account
    /// Resync only FLAGS from the server for all cached emails in a folder.
    /// This is lightweight (no body download) and fixes stale read/unread state.
    pub async fn sync_flags_for_folder(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        info!("Resyncing flags for folder: {} account: {}", folder_name, account_id);

        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service);

        let account_email = &account.email_address;

        // Get all cached UIDs for this folder
        let cached_uids = self.cache_service.get_cached_uids(folder_name, account_email).await
            .map_err(|e| SyncError::CacheError(e.to_string()))?;

        if cached_uids.is_empty() {
            debug!("No cached emails to resync flags for in {}", folder_name);
            return Ok(());
        }

        // Create IMAP session
        let session = self.imap_factory.create_session_for_account(&account).await?;
        session.select_folder(folder_name).await?;

        // Fetch flags in batches of 500 (FLAGS-only is very lightweight)
        const FLAG_BATCH_SIZE: usize = 500;
        let mut updated = 0;

        for chunk in cached_uids.chunks(FLAG_BATCH_SIZE) {
            let flag_results = session.fetch_flags(chunk).await?;
            for (uid, flags) in flag_results {
                if let Err(e) = self.cache_service.update_email_flags(folder_name, uid, &flags, account_email).await {
                    warn!("Failed to update flags for UID {}: {}", uid, e);
                } else {
                    updated += 1;
                }
            }
        }

        info!("Flag resync complete: updated {}/{} emails in {}", updated, cached_uids.len(), folder_name);
        Ok(())
    }

    pub async fn full_sync_folder(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        info!("Performing full sync of folder: {} for account: {}", folder_name, account_id);

        // Clear the folder cache
        if let Err(e) = self.cache_service.clear_folder_cache(folder_name, account_id).await {
            error!("Failed to clear folder cache: {}", e);
        }

        // Perform full sync without limit
        self.sync_folder_with_limit(account_id, folder_name, None).await
    }

    /// Handle IMAP IDLE for real-time updates for a specific account
    pub async fn start_idle_monitoring(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        debug!("Starting IDLE monitoring for folder: {} for account: {}", folder_name, account_id);

        // Get account credentials
        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service); // Release lock before creating session

        // Try to create session and record connection status
        let session = match self.imap_factory.create_session_for_account(&account).await {
            Ok(s) => {
                // Record successful IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(e) = account_service.update_imap_status(account_id, true, format!("Successfully connected to {} for IDLE", account.imap_host)).await {
                    warn!("Failed to update IMAP connection status: {}", e);
                }
                drop(account_service);
                s
            }
            Err(e) => {
                // Record failed IMAP connection
                let account_service = self.account_service.lock().await;
                if let Err(status_err) = account_service.update_imap_status(account_id, false, e.to_string()).await {
                    warn!("Failed to update IMAP connection status: {}", status_err);
                }
                drop(account_service);
                return Err(SyncError::ImapError(e));
            }
        };

        // Select the folder
        session.select_folder(folder_name).await?;

        // Note: IMAP IDLE implementation would go here
        // This requires keeping a persistent connection and handling IDLE responses
        // For now, we'll rely on periodic sync

        warn!("IDLE monitoring not yet implemented, using periodic sync");

        // IMPORTANT: Explicitly logout since we're not actually using IDLE
        if let Err(e) = session.logout().await {
            warn!("Failed to logout IMAP session: {}", e);
        }

        Ok(())
    }
}