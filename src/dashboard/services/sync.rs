use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use log::{info, error, debug, warn};
use crate::imap::error::ImapError;
use crate::prelude::CloneableImapSessionFactory;
use crate::dashboard::services::cache::{CacheService, SyncStatus};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("IMAP error: {0}")]
    ImapError(#[from] ImapError),
    #[error("Cache error: {0}")]
    CacheError(String),
    #[error("Sync cancelled")]
    Cancelled,
}

pub struct SyncService {
    imap_factory: CloneableImapSessionFactory,
    cache_service: Arc<CacheService>,
    sync_interval: Duration,
}

impl SyncService {
    pub fn new(
        imap_factory: CloneableImapSessionFactory,
        cache_service: Arc<CacheService>,
        sync_interval_seconds: u64,
    ) -> Self {
        Self {
            imap_factory,
            cache_service,
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
                if let Err(e) = self.sync_all_folders().await {
                    error!("Background sync failed: {}", e);
                }
            }
        })
    }

    /// Sync all folders
    pub async fn sync_all_folders(&self) -> Result<(), SyncError> {
        info!("Starting email sync for all folders");

        let session = self.imap_factory.create_session().await
            .map_err(|e| SyncError::ImapError(e))?;

        let folders = session.list_folders().await?;

        for folder in folders {
            if let Err(e) = self.sync_folder(&folder).await {
                warn!("Failed to sync folder {}: {}", folder, e);
                // Continue with other folders even if one fails
            }
        }

        info!("Email sync completed for all folders");
        Ok(())
    }

    /// Sync a specific folder
    pub async fn sync_folder(&self, folder_name: &str) -> Result<(), SyncError> {
        debug!("Syncing folder: {}", folder_name);

        // Update sync status
        if let Err(e) = self.cache_service.update_sync_state(folder_name, 0, SyncStatus::Syncing).await {
            warn!("Failed to update sync state: {}", e);
        }

        let session = self.imap_factory.create_session().await
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
            // First sync - get last N emails
            "ALL".to_string()
        };

        let uids = session.search_emails(&search_criteria).await?;

        if uids.is_empty() {
            debug!("No new emails to sync in folder {}", folder_name);
            if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid_synced, SyncStatus::Idle).await {
                warn!("Failed to update sync state: {}", e);
            }
            return Ok(());
        }

        // Limit the number of emails to sync in one batch
        const BATCH_SIZE: usize = 50;
        let uids_to_sync: Vec<u32> = if uids.len() > BATCH_SIZE {
            // For initial sync or large updates, take only the most recent emails
            uids.into_iter().rev().take(BATCH_SIZE).collect()
        } else {
            uids
        };

        debug!("Syncing {} emails in folder {}", uids_to_sync.len(), folder_name);

        // Fetch and cache emails
        let emails = session.fetch_emails(&uids_to_sync).await?;

        let mut last_uid = last_uid_synced;
        for email in emails {
            if let Err(e) = self.cache_service.cache_email(folder_name, &email).await {
                error!("Failed to cache email {}: {}", email.uid, e);
            } else {
                if email.uid > last_uid {
                    last_uid = email.uid;
                }
            }
        }

        // Update sync state with the highest UID synced
        if let Err(e) = self.cache_service.update_sync_state(folder_name, last_uid, SyncStatus::Idle).await {
            warn!("Failed to update sync state: {}", e);
        }

        info!("Synced {} emails in folder {}", uids_to_sync.len(), folder_name);
        Ok(())
    }

    /// Perform a full sync of a folder (clear cache and re-download)
    pub async fn full_sync_folder(&self, folder_name: &str) -> Result<(), SyncError> {
        info!("Performing full sync of folder: {}", folder_name);

        // Clear the folder cache
        if let Err(e) = self.cache_service.clear_folder_cache(folder_name).await {
            error!("Failed to clear folder cache: {}", e);
        }

        // Perform normal sync
        self.sync_folder(folder_name).await
    }

    /// Handle IMAP IDLE for real-time updates
    pub async fn start_idle_monitoring(&self, folder_name: &str) -> Result<(), SyncError> {
        debug!("Starting IDLE monitoring for folder: {}", folder_name);

        let session = self.imap_factory.create_session().await
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