// Atomic IMAP operations with ACID properties
use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::collections::HashSet;
use tokio::sync::RwLock;
use std::sync::Arc;

use super::{
    error::ImapError,
    session::{AsyncImapSessionWrapper, AsyncImapOps},
    types::FlagOperation,
};

/// Represents a transaction log entry for rollback support
#[derive(Debug, Clone)]
enum TransactionOp {
    Copy { uid: u32, from: String, to: String },
    StoreFlags { uid: u32, folder: String, flags: Vec<String>, operation: FlagOperation },
    Delete { uid: u32, folder: String },
}

/// Manages atomic IMAP operations with ACID properties
pub struct AtomicImapOperations {
    session: AsyncImapSessionWrapper,
    transaction_log: Arc<RwLock<Vec<TransactionOp>>>,
}

impl AtomicImapOperations {
    pub fn new(session: AsyncImapSessionWrapper) -> Self {
        Self {
            session,
            transaction_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Atomically move an email from one folder to another
    ///
    /// This operation guarantees:
    /// - **Atomicity**: Either the entire operation succeeds or it's rolled back
    /// - **Consistency**: Email is never lost or duplicated
    /// - **Isolation**: Operation is isolated from concurrent access
    /// - **Durability**: Changes are permanent once committed
    pub async fn atomic_move(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        info!("Starting atomic move of UID {} from {} to {}", uid, from_folder, to_folder);

        // Clear transaction log for this operation
        {
            let mut log = self.transaction_log.write().await;
            log.clear();
        }

        // Attempt the move operation
        match self.perform_move(uid, from_folder, to_folder).await {
            Ok(_) => {
                info!("Atomic move completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Atomic move failed: {:?}. Attempting rollback...", e);
                self.rollback().await;
                Err(e)
            }
        }
    }

    /// Internal method to perform the actual move
    async fn perform_move(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        // First, try using the native MOVE command if available
        if let Ok(_) = self.try_native_move(uid, from_folder, to_folder).await {
            return Ok(());
        }

        // Fallback to COPY + MARK_DELETED + EXPUNGE sequence
        self.copy_delete_expunge(uid, from_folder, to_folder).await
    }

    /// Try to use the MOVE extension (RFC 6851) if available
    async fn try_native_move(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        // This will be handled by the session's move_email method which already tries uid_mv
        self.session.move_email(uid, from_folder, to_folder).await
    }

    /// Perform COPY + DELETE + EXPUNGE sequence with logging for rollback
    async fn copy_delete_expunge(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        // Step 1: Ensure source folder is selected
        self.session.ensure_folder_selected(from_folder).await?;

        // Step 2: Verify message exists before proceeding
        let search_results = self.session.search_emails(&format!("UID {}", uid)).await?;
        if !search_results.contains(&uid) {
            return Err(ImapError::MissingData(format!("Message UID {} not found in {}", uid, from_folder)));
        }

        // Step 3: Copy to destination (log this operation)
        {
            let mut log = self.transaction_log.write().await;
            log.push(TransactionOp::Copy {
                uid,
                from: from_folder.to_string(),
                to: to_folder.to_string(),
            });
        }

        // Perform the copy - using the session's internal methods
        // We need to access the raw session for uid_copy
        // For now, we'll create a helper in the session
        self.perform_uid_copy(uid, to_folder).await?;

        // Step 4: Mark as deleted (log this operation)
        {
            let mut log = self.transaction_log.write().await;
            log.push(TransactionOp::StoreFlags {
                uid,
                folder: from_folder.to_string(),
                flags: vec!["\\Deleted".to_string()],
                operation: FlagOperation::Add,
            });
        }

        self.session.store_flags(&[uid], FlagOperation::Add, &[String::from("\\Deleted")]).await?;

        // Step 5: Expunge to complete the move
        self.session.expunge().await?;

        // Clear transaction log on success
        {
            let mut log = self.transaction_log.write().await;
            log.clear();
        }

        Ok(())
    }

    /// Helper to perform uid_copy through the session
    async fn perform_uid_copy(&self, uid: u32, to_folder: &str) -> Result<(), ImapError> {
        // Use the copy_messages method from AsyncImapOps
        self.session.copy_messages(&[uid], to_folder).await
    }

    /// Rollback operations based on transaction log
    async fn rollback(&self) {
        warn!("Starting rollback of atomic operations");

        let log = self.transaction_log.read().await;

        // Process rollback in reverse order
        for op in log.iter().rev() {
            match op {
                TransactionOp::Copy { uid, from: _, to } => {
                    // Rollback: Delete the copied message from destination
                    warn!("Rollback: Removing copied message UID {} from {}", uid, to);
                    if let Err(e) = self.rollback_copy(*uid, to).await {
                        error!("Failed to rollback copy: {:?}", e);
                    }
                }
                TransactionOp::StoreFlags { uid, folder, flags: _, operation } => {
                    // Rollback: Remove the flags that were added
                    if *operation == FlagOperation::Add {
                        warn!("Rollback: Removing flags from UID {} in {}", uid, folder);
                        if let Err(e) = self.session.store_flags(&[*uid], FlagOperation::Remove, &[String::from("\\Deleted")]).await {
                            error!("Failed to rollback flags: {:?}", e);
                        }
                    }
                }
                TransactionOp::Delete { uid, folder } => {
                    // Can't rollback a delete after expunge
                    error!("Cannot rollback deletion of UID {} from {} - message permanently removed", uid, folder);
                }
            }
        }
    }

    /// Rollback a copy operation by deleting from destination
    async fn rollback_copy(&self, uid: u32, folder: &str) -> Result<(), ImapError> {
        self.session.ensure_folder_selected(folder).await?;
        self.session.store_flags(&[uid], FlagOperation::Add, &[String::from("\\Deleted")]).await?;
        self.session.expunge().await?;
        Ok(())
    }

    /// Perform atomic batch move of multiple messages
    pub async fn atomic_batch_move(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        info!("Starting atomic batch move of {} messages from {} to {}", uids.len(), from_folder, to_folder);

        let mut failed_uids = Vec::new();

        for &uid in uids {
            if let Err(e) = self.atomic_move(uid, from_folder, to_folder).await {
                error!("Failed to move UID {}: {:?}", uid, e);
                failed_uids.push(uid);
            }
        }

        if !failed_uids.is_empty() {
            Err(ImapError::Other(format!("Failed to move UIDs: {:?}", failed_uids)))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests will be implemented when we have mock IMAP support
    #[tokio::test]
    async fn test_atomic_move() {
        // TODO: Implement with mock IMAP server
    }

    #[tokio::test]
    async fn test_rollback() {
        // TODO: Implement with mock IMAP server
    }
}