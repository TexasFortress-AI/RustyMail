// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::connection_status::AccountConnectionStatus;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::RwLock;
use log::{debug, error, info, warn};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectionStatusStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Status not found for account: {0}")]
    NotFound(String),
}

/// Storage for connection status information
#[derive(Debug, Serialize, Deserialize)]
struct ConnectionStatusStorage {
    /// Map of email_address to connection status
    #[serde(default)]
    statuses: HashMap<String, AccountConnectionStatus>,
}

impl Default for ConnectionStatusStorage {
    fn default() -> Self {
        Self {
            statuses: HashMap::new(),
        }
    }
}

/// Store for managing connection status persistence
pub struct ConnectionStatusStore {
    storage_path: PathBuf,
    cache: RwLock<ConnectionStatusStorage>,
}

impl ConnectionStatusStore {
    /// Create a new connection status store
    pub fn new(storage_path: impl AsRef<Path>) -> Self {
        Self {
            storage_path: storage_path.as_ref().to_path_buf(),
            cache: RwLock::new(ConnectionStatusStorage::default()),
        }
    }

    /// Initialize the store by loading from disk
    pub async fn initialize(&self) -> Result<(), ConnectionStatusStoreError> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Load existing data or create new file
        match fs::read_to_string(&self.storage_path).await {
            Ok(contents) => {
                let storage: ConnectionStatusStorage = serde_json::from_str(&contents)?;
                *self.cache.write().await = storage;
                info!(
                    "Loaded {} connection statuses from {}",
                    self.cache.read().await.statuses.len(),
                    self.storage_path.display()
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("Connection status file not found, creating new: {}", self.storage_path.display());
                self.save().await?;
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    /// Save the current state to disk
    async fn save(&self) -> Result<(), ConnectionStatusStoreError> {
        let storage = self.cache.read().await;
        let json = serde_json::to_string_pretty(&*storage)?;
        fs::write(&self.storage_path, json).await?;
        debug!("Saved connection status to {}", self.storage_path.display());
        Ok(())
    }

    /// Get connection status for an account
    pub async fn get_status(
        &self,
        email_address: &str,
    ) -> Result<AccountConnectionStatus, ConnectionStatusStoreError> {
        let storage = self.cache.read().await;
        storage
            .statuses
            .get(email_address)
            .cloned()
            .ok_or_else(|| ConnectionStatusStoreError::NotFound(email_address.to_string()))
    }

    /// Get connection status for an account, or return default if not found
    pub async fn get_status_or_default(&self, email_address: &str) -> AccountConnectionStatus {
        self.get_status(email_address)
            .await
            .unwrap_or_else(|_| AccountConnectionStatus::new(email_address))
    }

    /// Update connection status for an account
    pub async fn update_status(
        &self,
        status: AccountConnectionStatus,
    ) -> Result<(), ConnectionStatusStoreError> {
        {
            let mut storage = self.cache.write().await;
            storage
                .statuses
                .insert(status.email_address.clone(), status);
        }
        self.save().await?;
        Ok(())
    }

    /// Delete connection status for an account
    pub async fn delete_status(&self, email_address: &str) -> Result<(), ConnectionStatusStoreError> {
        {
            let mut storage = self.cache.write().await;
            storage.statuses.remove(email_address);
        }
        self.save().await?;
        Ok(())
    }

    /// Get all connection statuses
    pub async fn list_statuses(&self) -> HashMap<String, AccountConnectionStatus> {
        let storage = self.cache.read().await;
        storage.statuses.clone()
    }

    /// Clear all connection statuses (for testing)
    #[cfg(test)]
    pub async fn clear_all(&self) -> Result<(), ConnectionStatusStoreError> {
        {
            let mut storage = self.cache.write().await;
            storage.statuses.clear();
        }
        self.save().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_connection_status_store() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("connection_status.json");
        let store = ConnectionStatusStore::new(&store_path);

        // Initialize store
        store.initialize().await.unwrap();

        // Create and save a status
        let mut status = AccountConnectionStatus::new("test@example.com");
        status.set_imap_success("Connected successfully");
        store.update_status(status.clone()).await.unwrap();

        // Retrieve status
        let retrieved = store.get_status("test@example.com").await.unwrap();
        assert_eq!(retrieved.email_address, "test@example.com");
        assert!(retrieved.is_imap_healthy());

        // Update status
        let mut updated_status = retrieved.clone();
        updated_status.set_imap_failed("Connection timeout");
        store.update_status(updated_status).await.unwrap();

        // Verify update
        let retrieved_again = store.get_status("test@example.com").await.unwrap();
        assert!(!retrieved_again.is_imap_healthy());

        // Delete status
        store.delete_status("test@example.com").await.unwrap();
        assert!(store.get_status("test@example.com").await.is_err());
    }

    #[tokio::test]
    async fn test_connection_status_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("connection_status.json");

        // Create first store and save data
        {
            let store = ConnectionStatusStore::new(&store_path);
            store.initialize().await.unwrap();

            let mut status = AccountConnectionStatus::new("persistent@example.com");
            status.set_smtp_success("SMTP connected");
            store.update_status(status).await.unwrap();
        }

        // Create second store and verify data persisted
        {
            let store = ConnectionStatusStore::new(&store_path);
            store.initialize().await.unwrap();

            let status = store.get_status("persistent@example.com").await.unwrap();
            assert_eq!(status.email_address, "persistent@example.com");
            assert!(status.is_smtp_healthy());
        }
    }
}
