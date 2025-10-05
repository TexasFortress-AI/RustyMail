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

                // Get default account for background sync
                let account_service = self.account_service.lock().await;
                match account_service.get_default_account().await {
                    Ok(Some(account)) => {
                        let account_id = account.id.clone();
                        drop(account_service); // Release lock before sync

                        if let Err(e) = self.sync_all_folders(&account_id).await {
                            error!("Background sync failed for account {}: {}", account_id, e);
                        }
                    }
                    Ok(None) => {
                        drop(account_service);
                        debug!("No default account configured, skipping background sync");
                    }
                    Err(e) => {
                        drop(account_service);
                        error!("Failed to get default account for background sync: {}", e);
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

        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| SyncError::ImapError(e))?;

        let folders = session.list_folders().await?;

        for folder in folders {
            if let Err(e) = self.sync_folder(account_id, &folder).await {
                warn!("Failed to sync folder {} for account {}: {}", folder, account_id, e);
                // Continue with other folders even if one fails
            }
        }

        info!("Email sync completed for all folders for account: {}", account_id);
        Ok(())
    }

    /// Sync a specific folder for a specific account
    pub async fn sync_folder(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        self.sync_folder_with_limit(account_id, folder_name, None).await
    }

    /// Sync a specific folder with optional limit for a specific account
    pub async fn sync_folder_with_limit(&self, account_id: &str, folder_name: &str, limit: Option<usize>) -> Result<(), SyncError> {
        debug!("Syncing folder: {} for account: {} (limit: {:?})", folder_name, account_id, limit);

        // Update sync status
        if let Err(e) = self.cache_service.update_sync_state(folder_name, 0, SyncStatus::Syncing).await {
            warn!("Failed to update sync state: {}", e);
        }

        // Get account credentials
        let account_service = self.account_service.lock().await;
        let account = account_service.get_account(account_id).await
            .map_err(|e| SyncError::AccountError(format!("Failed to get account: {}", e)))?;
        drop(account_service); // Release lock before creating session

        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| SyncError::ImapError(e))?;

        // Select the folder
        session.select_folder(folder_name).await?;

        // Get the sync state from cache
        let sync_state = self.cache_service.get_sync_state(folder_name).await
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
            if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid_synced, SyncStatus::Idle).await {
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
                                if let Err(e) = self.cache_service.cache_email(folder_name, &email).await {
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

            for email in emails {
                if let Err(e) = self.cache_service.cache_email(folder_name, &email).await {
                    error!("Failed to cache email {}: {}", email.uid, e);
                } else {
                    if email.uid > last_uid {
                        last_uid = email.uid;
                    }
                }
            }
        }

        // Update sync state with the highest UID synced
        if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid, SyncStatus::Idle).await {
            warn!("Failed to update sync state: {}", e);
        }

        info!("Successfully synced {} emails in folder {}", uids_to_sync.len(), folder_name);
        Ok(())
    }

    /// Perform a full sync of a folder (clear cache and re-download) for a specific account
    pub async fn full_sync_folder(&self, account_id: &str, folder_name: &str) -> Result<(), SyncError> {
        info!("Performing full sync of folder: {} for account: {}", folder_name, account_id);

        // Clear the folder cache
        if let Err(e) = self.cache_service.clear_folder_cache(folder_name).await {
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

        let session = self.imap_factory.create_session_for_account(&account).await
            .map_err(|e| SyncError::ImapError(e))?;

        // Select the folder
        session.select_folder(folder_name).await?;

        // Note: IMAP IDLE implementation would go here
        // This requires keeping a persistent connection and handling IDLE responses
        // For now, we'll rely on periodic sync

        warn!("IDLE monitoring not yet implemented, using periodic sync");
        Ok(())
    }
}