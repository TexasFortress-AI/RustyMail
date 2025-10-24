// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Standard library imports
use std::{
    pin::Pin,
    future::Future,
    fmt::Debug,
    borrow::Cow,
    sync::Arc,
    time::Duration,
};

// Async runtime and utilities
use async_trait::async_trait;
use futures_util::stream::TryStreamExt;
use futures_util::future::BoxFuture;
use log::{debug, error, info, warn};

// IMAP types and client
use async_imap::{
    // Client struct is likely private or needs specific feature flags
    // client::Client as AsyncImapClient,
    types::{
        Fetch, Flag as AsyncImapFlag, Name as AsyncImapName, Mailbox as AsyncImapMailbox,
        UnsolicitedResponse,
        // Remove unresolved imports
        // Address as AsyncImapAddress, 
        // Envelope as AsyncImapEnvelope, 
        Flag,
    },
    Session as AsyncImapSession,
};

// imap_types imports removed - all were unused

// Local types
use crate::imap::client::ImapClient; // Correct path for ImapClient
use crate::imap::{
    types::{Email, FlagOperation, SearchCriteria, Address, Envelope as ImapEnvelope},
    error::ImapError,
};

// TLS Stream types
use tokio::net::TcpStream as TokioTcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_native_tls;
use tokio_native_tls::native_tls;
use tokio_native_tls::TlsConnector;
use tokio::sync::Mutex as TokioMutex;

// Type aliases
pub type TlsCompatibleStream = tokio_util::compat::Compat<tokio_native_tls::TlsStream<TokioTcpStream>>;
pub type TlsImapSession = async_imap::Session<TlsCompatibleStream>;
pub type ImapSessionFactory = Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<TlsImapSession, ImapError>> + Send>> + Send + Sync>;

// Define a constant for the delimiter
pub const DEFAULT_MAILBOX_DELIMITER: &str = "/";

/// Trait defining asynchronous IMAP operations
#[async_trait]
pub trait AsyncImapOps: Send + Sync + Debug {
    /// Logs in the user with the given credentials
    async fn login(&self, username: &str, password: &str) -> Result<(), ImapError>;

    /// Logs out the current session
    async fn logout(&self) -> Result<(), ImapError>;

    /// Lists all folders in the mailbox (returns flat list of folder names for backward compatibility)
    async fn list_folders(&self) -> Result<Vec<String>, ImapError>;

    /// Lists all folders with hierarchical structure and metadata
    async fn list_folders_hierarchical(&self) -> Result<Vec<crate::imap::types::Folder>, ImapError>;
    
    /// Creates a new folder with the given name
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Deletes an existing folder
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Renames a folder from old_name to new_name
    async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError>;
    
    /// Selects a folder for subsequent operations
    async fn select_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Searches for emails matching the given criteria (string-based for backward compatibility)
    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError>;

    /// Searches for emails using structured search criteria
    async fn search_emails_structured(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError>;
    
    /// Fetches emails with the given UIDs
    async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError>;
    
    /// Moves an email from one folder to another
    async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError>;
    
    /// Stores flags for the given email UIDs
    async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError>;
    
    /// Appends an email to the specified folder
    async fn append(&self, folder: &str, content: &[u8], flags: &[String]) -> Result<(), ImapError>;
    
    /// Fetches the raw message content for a given UID
    async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError>;

    /// Permanently removes messages marked with the \Deleted flag
    async fn expunge(&self) -> Result<(), ImapError>;

    /// Copy messages to another folder (for atomic operations)
    async fn copy_messages(&self, uids: &[u32], to_folder: &str) -> Result<(), ImapError>;

    /// Batch move messages atomically from one folder to another
    async fn move_messages(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), ImapError>;

    /// Mark messages as deleted (sets \Deleted flag)
    async fn mark_as_deleted(&self, uids: &[u32]) -> Result<(), ImapError>;

    /// Delete messages (mark as deleted and expunge)
    async fn delete_messages(&self, uids: &[u32]) -> Result<(), ImapError>;

    /// Undelete messages (removes \Deleted flag)
    async fn undelete_messages(&self, uids: &[u32]) -> Result<(), ImapError>;

    /// Send NOOP command (keeps connection alive and checks for updates)
    async fn noop(&self) -> Result<(), ImapError>;
}

// Wrapper definition using Arc<Mutex<...>>
#[derive(Debug, Clone)] // Add Clone
pub struct AsyncImapSessionWrapper {
    // Wrap the session in Arc<Mutex> for interior mutability
    session: Arc<TokioMutex<TlsImapSession>>,
    // Track currently selected folder for atomic operations
    current_folder: Arc<TokioMutex<Option<String>>>,
    // Timeout for APPEND operations (configurable to handle slow servers)
    append_timeout: Duration,
}

impl AsyncImapSessionWrapper {
    pub fn new(session: TlsImapSession) -> Self {
        Self::with_append_timeout(session, Duration::from_secs(35))
    }

    pub fn with_append_timeout(session: TlsImapSession, append_timeout: Duration) -> Self {
        Self {
            // Create the Arc<Mutex<>> here
            session: Arc::new(TokioMutex::new(session)),
            current_folder: Arc::new(TokioMutex::new(None)),
            append_timeout,
        }
    }

    pub async fn connect(
        server: &str,
        port: u16,
        username: Arc<String>,
        password: Arc<String>,
        append_timeout: Duration,
    ) -> Result<Self, ImapError> {
        // Create TLS connector
        let tls_builder = native_tls::TlsConnector::builder();
        let tls = tls_builder
            .build()
            .map_err(|e| ImapError::Tls(e.to_string()))?;
        let tls_connector = TlsConnector::from(tls);

        // Connect to server via TCP
        let addr = format!("{}:{}", server, port);
        let tcp_stream = TokioTcpStream::connect(&addr)
            .await
            .map_err(|e| ImapError::Connection(e.to_string()))?;

        // Set socket-level timeouts to ensure blocking I/O operations timeout
        // This is CRITICAL for IMAP APPEND operations which may block indefinitely
        info!("Setting socket timeouts: read={:?}, write={:?}", append_timeout, append_timeout);

        // Convert to std::net::TcpStream to set SO_RCVTIMEO and SO_SNDTIMEO
        let std_stream = tcp_stream.into_std()
            .map_err(|e| ImapError::Connection(format!("Failed to convert to std stream: {}", e)))?;

        std_stream.set_read_timeout(Some(append_timeout))
            .map_err(|e| ImapError::Connection(format!("Failed to set read timeout: {}", e)))?;
        std_stream.set_write_timeout(Some(append_timeout))
            .map_err(|e| ImapError::Connection(format!("Failed to set write timeout: {}", e)))?;

        // Convert back to tokio::net::TcpStream
        let tcp_stream = TokioTcpStream::from_std(std_stream)
            .map_err(|e| ImapError::Connection(format!("Failed to convert back to tokio stream: {}", e)))?;

        // Perform TLS handshake
        let tls_stream = tls_connector
            .connect(server, tcp_stream)
            .await
            .map_err(|e| ImapError::Tls(e.to_string()))?;

        // Convert to compatible stream type
        let compat_stream = tls_stream.compat();

        // Create IMAP client and login
        let client = async_imap::Client::new(compat_stream);
        let session = client
            .login(&*username, &*password)
            .await
            .map_err(|(err, _client)| {
                // Handle the error part of the Result - login returns (Error, Client) on failure
                match err {
                    async_imap::error::Error::No(msg) | async_imap::error::Error::Bad(msg) => {
                        ImapError::Auth(format!("Login failed: {}", msg))
                    }
                    _ => ImapError::Auth(format!("Login failed: {:?}", err))
                }
            })?;

        Ok(Self::with_append_timeout(session, append_timeout))
    }

    /// Get the currently selected folder
    pub async fn current_folder(&self) -> Option<String> {
        let folder_guard = self.current_folder.lock().await;
        folder_guard.clone()
    }

    /// Ensure a specific folder is selected (optimization to avoid redundant SELECTs)
    pub async fn ensure_folder_selected(&self, folder: &str) -> Result<(), ImapError> {
        let current = self.current_folder().await;

        if current.as_deref() != Some(folder) {
            // Need to select the folder
            let mut session_guard = self.session.lock().await;
            session_guard.select(folder).await.map_err(ImapError::from)?;
            drop(session_guard);

            // Update tracked state
            let mut folder_guard = self.current_folder.lock().await;
            *folder_guard = Some(folder.to_string());
        }

        Ok(())
    }
}

#[async_trait]
impl AsyncImapOps for AsyncImapSessionWrapper {
    // Acquire lock in each method before calling the inner session method
    async fn login(&self, _username: &str, _password: &str) -> Result<(), ImapError> {
        // Login is already done during connect, so this is a no-op
        // The session is already authenticated
        Ok(())
    }

    async fn logout(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await.map_err(ImapError::from)?;
        Ok(())
    }

    async fn list_folders(&self) -> Result<Vec<String>, ImapError> {
        let mut session_guard = self.session.lock().await;
        // Use the IMAP LIST command to get all folders
        let mut folders_stream = session_guard
            .list(None, Some("*"))
            .await
            .map_err(ImapError::from)?;

        let mut folder_names = Vec::new();
        while let Some(folder_result) = folders_stream.try_next().await.map_err(ImapError::from)? {
            folder_names.push(folder_result.name().to_string());
        }

        Ok(folder_names)
    }

    async fn list_folders_hierarchical(&self) -> Result<Vec<crate::imap::types::Folder>, ImapError> {
        let mut session_guard = self.session.lock().await;

        // Use the IMAP LIST command to get all folders with detailed information
        let mut folders_stream = session_guard
            .list(None, Some("*"))
            .await
            .map_err(ImapError::from)?;

        let mut folder_data = Vec::new();

        while let Some(folder_result) = folders_stream.try_next().await.map_err(ImapError::from)? {
            let name = folder_result.name().to_string();

            // Extract delimiter - async-imap Name struct should have delimiter info
            let delimiter = folder_result.delimiter().map(|d| d.to_string());

            // Extract attributes - convert flags to string attributes
            let attributes: Vec<String> = folder_result.attributes()
                .iter()
                .map(|attr| format!("{:?}", attr)) // Convert attribute enum to string
                .collect();

            folder_data.push((name, delimiter, attributes));
        }

        // Build hierarchical structure
        let hierarchy = crate::imap::types::Folder::build_hierarchy(folder_data);
        Ok(hierarchy)
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.create(name).await.map_err(ImapError::from)
    }

    async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.delete(name).await.map_err(ImapError::from)
    }

    async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.rename(old_name, new_name).await.map_err(ImapError::from)
    }

    async fn select_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.select(name).await.map(|_| ()).map_err(ImapError::from)?;

        // Update tracked folder state
        let mut folder_guard = self.current_folder.lock().await;
        *folder_guard = Some(name.to_string());
        Ok(())
    }

    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        let mut session_guard = self.session.lock().await;
        // Use UID SEARCH to get UIDs, not message sequence numbers
        // This is critical because fetch_emails uses UID FETCH
        let sequence_set = session_guard.uid_search(criteria)
            .await
            .map_err(ImapError::from)?;
        Ok(sequence_set.into_iter().collect())
    }

    async fn search_emails_structured(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError> {
        // Convert structured criteria to IMAP search string
        let criteria_string = criteria.to_string();

        // Validate criteria string before sending to server
        if criteria_string.trim().is_empty() {
            return Err(ImapError::InvalidCriteria("Empty search criteria".to_string()));
        }

        debug!("Executing IMAP search with criteria: {}", criteria_string);

        let mut session_guard = self.session.lock().await;

        // Execute the search on the server using UID SEARCH
        // This is critical because fetch_emails uses UID FETCH
        let sequence_set = session_guard.uid_search(&criteria_string)
            .await
            .map_err(|e| {
                error!("IMAP UID search failed for criteria '{}': {}", criteria_string, e);
                ImapError::InvalidCriteria(format!("Search failed: {}", e))
            })?;

        let results: Vec<u32> = sequence_set.into_iter().collect();

        info!("IMAP search returned {} results for criteria: {}", results.len(), criteria_string);
        Ok(results)
    }

    async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        debug!("Fetching {} UIDs: {:?}", uids.len(), uids);
        let mut fetch_stream = session_guard.uid_fetch(&sequence, "(FLAGS ENVELOPE INTERNALDATE BODY[])")
            .await
            .map_err(ImapError::from)?;

        let mut emails = Vec::new();
        while let Some(fetch_result) = fetch_stream.try_next().await.map_err(ImapError::from)? {
            let email = Email::from(fetch_result);
            debug!("Fetched email UID: {}", email.uid);
            emails.push(email);
        }
        debug!("Fetch complete: requested {} UIDs, received {} emails", uids.len(), emails.len());
        if emails.len() != uids.len() {
            warn!("UID mismatch: requested {}, received {}. Missing UIDs: {:?}",
                  uids.len(), emails.len(),
                  uids.iter().filter(|uid| !emails.iter().any(|e| e.uid == **uid)).collect::<Vec<_>>());
        }
        Ok(emails)
    }

    async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        // Atomic move operation following IMAP best practices
        // Sequence: SELECT source → COPY to dest → STORE \Deleted → EXPUNGE

        let mut session_guard = self.session.lock().await;

        // Step 1: Select source folder
        session_guard.select(from_folder).await.map_err(ImapError::from)?;

        // Update tracked folder state
        {
            let mut folder_guard = self.current_folder.lock().await;
            *folder_guard = Some(from_folder.to_string());
        }

        let sequence = uid.to_string();

        // Step 2: Try MOVE command first (RFC 6851) - more efficient if supported
        match session_guard.uid_mv(&sequence, to_folder).await {
            Ok(_) => {
                // MOVE succeeded - atomic operation complete
                return Ok(());
            }
            Err(e) => {
                // MOVE not supported or failed, fallback to COPY+DELETE+EXPUNGE
                debug!("MOVE command failed, falling back to COPY+DELETE: {:?}", e);
            }
        }

        // Fallback: Traditional atomic move sequence
        // Step 3: Copy message to destination
        session_guard.uid_copy(&sequence, to_folder)
            .await
            .map_err(|e| ImapError::Other(format!("Failed to copy message: {}", e)))?;

        // Step 4: Mark original as deleted
        let mut store_stream = session_guard.uid_store(&sequence, "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| ImapError::Other(format!("Failed to mark as deleted: {}", e)))?;

        // Consume the store stream
        let _store_results: Vec<_> = store_stream
            .try_collect()
            .await
            .map_err(|e| ImapError::Other(format!("Failed to process store results: {}", e)))?;

        // Step 5: Expunge to remove deleted messages
        let mut expunge_stream = session_guard.expunge()
            .await
            .map_err(|e| ImapError::Other(format!("Failed to expunge: {}", e)))?;

        // Consume the expunge stream
        let _expunge_results: Vec<_> = expunge_stream
            .try_collect()
            .await
            .map_err(|e| ImapError::Other(format!("Failed to process expunge results: {}", e)))?;

        Ok(())
    }

    async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");

        // Build the store command with flags
        let flags_str = flags.join(" ");
        let op_str = match operation {
            FlagOperation::Add => format!("+FLAGS ({})", flags_str),
            FlagOperation::Remove => format!("-FLAGS ({})", flags_str),
            FlagOperation::Set => format!("FLAGS ({})", flags_str),
        };

        // uid_store takes sequence and the full command
        let mut stream = session_guard.uid_store(&sequence, &op_str).await?;
        let result = stream.try_collect::<Vec<_>>().await;
        drop(session_guard); // Explicitly drop the guard after consuming the stream
        result.map(|_| ()).map_err(ImapError::from)
    }

    async fn append(&self, folder: &str, content: &[u8], flags: &[String]) -> Result<(), ImapError> {
        // Convert String flags to async_imap Flag types
        let imap_flags: Vec<Flag> = flags
            .iter()
            .filter_map(|f| {
                match f.as_str() {
                    "\\Seen" => Some(Flag::Seen),
                    "\\Answered" => Some(Flag::Answered),
                    "\\Flagged" => Some(Flag::Flagged),
                    "\\Deleted" => Some(Flag::Deleted),
                    "\\Draft" => Some(Flag::Draft),
                    _ => None, // Skip unknown flags
                }
            })
            .collect();

        // Clone the session Arc for move into spawn_blocking
        let session_arc = self.session.clone();
        let folder_str = folder.to_string();
        let folder_for_error = folder_str.clone(); // Clone for error messages
        let content = content.to_vec();
        let append_timeout = self.append_timeout;

        info!("Starting IMAP APPEND to folder '{}' with spawn_blocking (timeout: {:?})", folder_str, append_timeout);

        // Use spawn_blocking to run the IMAP APPEND in a dedicated blocking thread
        // This allows us to timeout even when the underlying I/O is blocking
        let blocking_task = tokio::task::spawn_blocking(move || {
            // Block on getting the mutex lock - this will happen in the blocking thread pool
            let runtime_handle = tokio::runtime::Handle::current();
            let mut session_guard = runtime_handle.block_on(session_arc.lock());

            // Perform the blocking IMAP APPEND operation
            debug!("Executing IMAP APPEND in blocking thread for folder '{}'", folder_str);
            runtime_handle.block_on(session_guard.append(&folder_str, &content))
        });

        // Apply timeout to the entire spawn_blocking task
        match tokio::time::timeout(append_timeout, blocking_task).await {
            Ok(Ok(Ok(()))) => {
                info!("APPEND to folder '{}' completed successfully", folder_for_error);
                Ok(())
            }
            Ok(Ok(Err(e))) => {
                error!("APPEND to folder '{}' failed: {}", folder_for_error, e);
                Err(ImapError::from(e))
            }
            Ok(Err(join_err)) => {
                error!("APPEND spawn_blocking task panicked: {}", join_err);
                Err(ImapError::Other(format!("APPEND task panicked: {}", join_err)))
            }
            Err(_elapsed) => {
                error!("APPEND to folder '{}' timed out after {:?}", folder_for_error, append_timeout);
                Err(ImapError::Timeout(format!(
                    "APPEND operation timed out after {:?}. The blocking thread was terminated. Server may be slow due to security scanning.",
                    append_timeout
                )))
            }
        }
    }

    async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uid.to_string();
        // uid_fetch needs &mut
        let mut fetch_stream = session_guard.uid_fetch(&sequence, "BODY[]")
            .await
            .map_err(ImapError::from)?;
            
        if let Some(fetch_result) = fetch_stream.try_next().await.map_err(ImapError::from)? {
            fetch_result.body()
                .map(|b| b.to_vec())
                .ok_or_else(|| ImapError::MissingData("Message body not found".to_string()))
        } else {
            Err(ImapError::MissingData("No fetch result found for UID".to_string()))
        }
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        // expunge needs &mut
        let mut stream = session_guard.expunge().await?;
        let result = stream.try_collect::<Vec<_>>().await;
        drop(session_guard); // Explicitly drop the guard after consuming the stream
        result.map(|_| ()).map_err(ImapError::from)
    }

    async fn copy_messages(&self, uids: &[u32], to_folder: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");

        // Use uid_copy to copy messages to destination folder
        session_guard.uid_copy(&sequence, to_folder)
            .await
            .map_err(|e| ImapError::Other(format!("Failed to copy messages: {}", e)))?;

        Ok(())
    }

    async fn move_messages(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }

        // Ensure the source folder is selected
        self.ensure_folder_selected(from_folder).await?;

        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");

        // Try MOVE command first (RFC 6851) - more efficient for batch operations
        match session_guard.uid_mv(&sequence, to_folder).await {
            Ok(_) => {
                debug!("Batch MOVE command succeeded for {} messages", uids.len());
                return Ok(());
            }
            Err(e) => {
                debug!("Batch MOVE command failed, falling back to COPY+DELETE+EXPUNGE: {:?}", e);
            }
        }

        // Fallback: COPY+DELETE+EXPUNGE sequence for batch
        // Step 1: Copy all messages to destination
        session_guard.uid_copy(&sequence, to_folder)
            .await
            .map_err(|e| ImapError::Other(format!("Failed to copy messages: {}", e)))?;

        // Step 2: Mark all as deleted
        let mut store_stream = session_guard.uid_store(&sequence, "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| ImapError::Other(format!("Failed to mark messages as deleted: {}", e)))?;

        // Consume the store stream
        let _store_results: Vec<_> = store_stream
            .try_collect()
            .await
            .map_err(|e| ImapError::Other(format!("Failed to process store results: {}", e)))?;

        drop(session_guard); // Release lock before expunge

        // Step 3: Expunge to remove deleted messages
        self.expunge().await?;

        info!("Successfully moved {} messages from {} to {} using COPY+DELETE+EXPUNGE",
              uids.len(), from_folder, to_folder);
        Ok(())
    }

    async fn mark_as_deleted(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }

        debug!("Marking {} messages as deleted", uids.len());

        // Use the store_flags method to add the \Deleted flag
        self.store_flags(uids, FlagOperation::Add, &[String::from("\\Deleted")]).await?;

        info!("Successfully marked {} messages as deleted", uids.len());
        Ok(())
    }

    async fn delete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }

        debug!("Deleting {} messages (mark as deleted + expunge)", uids.len());

        // Step 1: Mark messages as deleted
        self.mark_as_deleted(uids).await?;

        // Step 2: Expunge to permanently remove deleted messages
        self.expunge().await?;

        info!("Successfully deleted {} messages permanently", uids.len());
        Ok(())
    }

    async fn undelete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }

        debug!("Removing \\Deleted flag from {} messages", uids.len());

        // Use the store_flags method to remove the \Deleted flag
        self.store_flags(uids, FlagOperation::Remove, &[String::from("\\Deleted")]).await?;

        info!("Successfully undeleted {} messages", uids.len());
        Ok(())
    }

    async fn noop(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;

        // Send NOOP command to keep connection alive and check for updates
        session_guard.noop()
            .await
            .map_err(ImapError::from)?;

        debug!("Successfully sent NOOP keepalive command");
        Ok(())
    }
}

/// Type alias for a factory function that creates IMAP clients
pub type ImapClientFactory = Box<dyn Fn() -> BoxFuture<'static, Result<ImapClient<AsyncImapSessionWrapper>, ImapError>> + Send + Sync>;

/// Creates a factory function for IMAP clients
/// 
/// # Arguments
/// * `hostname` - IMAP server hostname
/// * `port` - IMAP server port 
/// * `username` - IMAP account username
/// * `password` - IMAP account password
///
/// # Returns
/// A boxed factory function that creates new IMAP client instances
pub fn create_imap_factory(
    host: String,
    port: u16,
    username: String,
    password: String,
    timeout: Duration,
) -> Result<ImapSessionFactory, ImapError> {
    // Create TLS connector with default configuration
    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| ImapError::Tls(e.to_string()))?;
    let tls = TlsConnector::from(tls);
    let tls = Arc::new(tls);

    // Store connection parameters in Arc for sharing
    let host = Arc::new(host);
    let username = Arc::new(username);
    let password = Arc::new(password);

    Ok(Box::new(move || {
        let host = host.clone();
        let username = username.clone();
        let password = password.clone();
        let tls = tls.clone();

        Box::pin(async move {
            // Connect to server via TCP
            let tcp_stream = TokioTcpStream::connect((host.as_str(), port))
                .await
                .map_err(|e| ImapError::Connection(e.to_string()))?;

            // Perform TLS handshake
            let tls_stream = tls
                .connect(host.as_str(), tcp_stream)
                .await
                .map_err(|e| ImapError::Tls(e.to_string()))?;

            // Convert to compatible stream type
            let compat_stream = tls_stream.compat();

            // Create IMAP client and login
            let client = async_imap::Client::new(compat_stream);
            let session = client
                .login(&*username, &*password)
                .await
                .map_err(|(err, _client)| ImapError::Auth(err.to_string()))?;

            Ok(session)
        })
    }))
}

// Note: Removed dead AsyncImapSession implementation
// All IMAP functionality is now in AsyncImapSessionWrapper
