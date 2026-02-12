// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Standard library imports
use std::{
    pin::Pin,
    future::Future,
    fmt::Debug,
    sync::Arc,
    time::Duration,
};

// Async runtime and utilities
use async_trait::async_trait;
use futures_util::stream::TryStreamExt;
use log::{debug, error, info, warn};

// IMAP types and client
use async_imap::{
    types::{
        Fetch, Flag, Name as AsyncImapName, Mailbox as AsyncImapMailbox,
    },
    Session as AsyncImapSession,
};

// Local types
use crate::imap::{
    types::{Email, FlagOperation, SearchCriteria},
    error::ImapError,
};

// TLS Stream types
use tokio::net::TcpStream as TokioTcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_native_tls::{native_tls, TlsConnector};
use tokio::sync::Mutex as TokioMutex;

// Type aliases
pub type TlsCompatibleStream = tokio_util::compat::Compat<tokio_native_tls::TlsStream<TokioTcpStream>>;
pub type TlsImapSession = async_imap::Session<TlsCompatibleStream>;
pub type ImapClientFactory = fn(TlsCompatibleStream) -> async_imap::Client<TlsCompatibleStream>;

// Define a constant for the delimiter
pub const DEFAULT_MAILBOX_DELIMITER: &str = "/";

/// Trait defining asynchronous IMAP operations
#[async_trait]
pub trait AsyncImapOps: Send + Sync + Debug {
    async fn login(&self, username: &str, password: &str) -> Result<(), ImapError>;
    async fn logout(&self) -> Result<(), ImapError>;
    async fn list_folders(&self) -> Result<Vec<String>, ImapError>;
    async fn list_folders_hierarchical(&self) -> Result<Vec<crate::imap::types::Folder>, ImapError>;
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError>;
    async fn select_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError>;
    async fn search_emails_structured(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError>;
    async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError>;
    async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError>;
    async fn append(&self, folder: &str, content: &[u8], flags: &[String]) -> Result<(), ImapError>;
    async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError>;
    async fn expunge(&self) -> Result<(), ImapError>;
    async fn copy_messages(&self, uids: &[u32], to_folder: &str) -> Result<(), ImapError>;
    async fn move_messages(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), ImapError>;
    async fn mark_as_deleted(&self, uids: &[u32]) -> Result<(), ImapError>;
    async fn delete_messages(&self, uids: &[u32]) -> Result<(), ImapError>;
    async fn undelete_messages(&self, uids: &[u32]) -> Result<(), ImapError>;
    async fn noop(&self) -> Result<(), ImapError>;
}

// Wrapper definition using Arc<Mutex<...>>
#[derive(Debug, Clone)]
pub struct AsyncImapSessionWrapper {
    session: Arc<TokioMutex<TlsImapSession>>,
    current_folder: Arc<TokioMutex<Option<String>>>,
    append_timeout: Duration,
}

impl AsyncImapSessionWrapper {
    pub fn new(session: TlsImapSession) -> Self {
        Self::with_append_timeout(session, Duration::from_secs(35))
    }

    pub fn with_append_timeout(session: TlsImapSession, append_timeout: Duration) -> Self {
        Self {
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
        let tls_builder = native_tls::TlsConnector::builder();
        let tls = tls_builder.build().map_err(|e| ImapError::Tls(e.to_string()))?;
        let tls_connector = TlsConnector::from(tls);

        let addr = format!("{}:{}", server, port);
        let tcp_stream = TokioTcpStream::connect(&addr).await.map_err(|e| ImapError::Connection(e.to_string()))?;

        info!("Setting socket timeouts: read={:?}, write={:?}", append_timeout, append_timeout);

        let std_stream = tcp_stream.into_std().map_err(|e| ImapError::Connection(format!("Failed to convert to std stream: {}", e)))?;
        std_stream.set_read_timeout(Some(append_timeout)).map_err(|e| ImapError::Connection(format!("Failed to set read timeout: {}", e)))?;
        std_stream.set_write_timeout(Some(append_timeout)).map_err(|e| ImapError::Connection(format!("Failed to set write timeout: {}", e)))?;
        let tcp_stream = TokioTcpStream::from_std(std_stream).map_err(|e| ImapError::Connection(format!("Failed to convert back to tokio stream: {}", e)))?;

        let tls_stream = tls_connector.connect(server, tcp_stream).await.map_err(|e| ImapError::Tls(e.to_string()))?;
        let compat_stream = tls_stream.compat();

        let client = async_imap::Client::new(compat_stream);
        let session = client.login(&*username, &*password).await.map_err(|(err, _client)| {
            match err {
                async_imap::error::Error::No(msg) | async_imap::error::Error::Bad(msg) => ImapError::Auth(format!("Login failed: {}", msg)),
                _ => ImapError::Auth(format!("Login failed: {:?}", err)),
            }
        })?;

        Ok(Self::with_append_timeout(session, append_timeout))
    }

    /// Connect using XOAUTH2 authentication (for OAuth2 providers like Microsoft 365 and Gmail)
    pub async fn connect_with_xoauth2(
        server: &str,
        port: u16,
        username: Arc<String>,
        access_token: Arc<String>,
        append_timeout: Duration,
    ) -> Result<Self, ImapError> {
        use crate::imap::xoauth2::XOAuth2Authenticator;

        let tls_builder = native_tls::TlsConnector::builder();
        let tls = tls_builder.build().map_err(|e| ImapError::Tls(e.to_string()))?;
        let tls_connector = TlsConnector::from(tls);

        let addr = format!("{}:{}", server, port);
        let tcp_stream = TokioTcpStream::connect(&addr).await.map_err(|e| ImapError::Connection(e.to_string()))?;

        info!("Setting socket timeouts: read={:?}, write={:?}", append_timeout, append_timeout);

        let std_stream = tcp_stream.into_std().map_err(|e| ImapError::Connection(format!("Failed to convert to std stream: {}", e)))?;
        std_stream.set_read_timeout(Some(append_timeout)).map_err(|e| ImapError::Connection(format!("Failed to set read timeout: {}", e)))?;
        std_stream.set_write_timeout(Some(append_timeout)).map_err(|e| ImapError::Connection(format!("Failed to set write timeout: {}", e)))?;
        let tcp_stream = TokioTcpStream::from_std(std_stream).map_err(|e| ImapError::Connection(format!("Failed to convert back to tokio stream: {}", e)))?;

        let tls_stream = tls_connector.connect(server, tcp_stream).await.map_err(|e| ImapError::Tls(e.to_string()))?;
        let compat_stream = tls_stream.compat();

        let mut client = async_imap::Client::new(compat_stream);

        // Consume the IMAP server greeting before AUTHENTICATE.
        // async-imap's login() handles this internally, but authenticate()
        // expects the greeting to have been read already.
        let _greeting = client.read_response().await;

        // Use XOAUTH2 authentication
        let authenticator = XOAuth2Authenticator::new(&username, &access_token);
        let session = client.authenticate("XOAUTH2", authenticator).await.map_err(|(err, _client)| {
            match err {
                async_imap::error::Error::No(msg) | async_imap::error::Error::Bad(msg) => ImapError::Auth(format!("XOAUTH2 login failed: {}", msg)),
                _ => ImapError::Auth(format!("XOAUTH2 login failed: {:?}", err)),
            }
        })?;

        info!("XOAUTH2 authentication successful for user: {}", username);
        Ok(Self::with_append_timeout(session, append_timeout))
    }

    pub async fn current_folder(&self) -> Option<String> {
        self.current_folder.lock().await.clone()
    }

    pub async fn ensure_folder_selected(&self, folder: &str) -> Result<(), ImapError> {
        let current = self.current_folder().await;
        if current.as_deref() != Some(folder) {
            let mut session_guard = self.session.lock().await;
            session_guard.select(folder).await.map_err(ImapError::from)?;
            drop(session_guard);
            let mut folder_guard = self.current_folder.lock().await;
            *folder_guard = Some(folder.to_string());
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncImapOps for AsyncImapSessionWrapper {
    async fn login(&self, _username: &str, _password: &str) -> Result<(), ImapError> {
        Ok(())
    }

    async fn logout(&self) -> Result<(), ImapError> {
        info!("IMAP logout called - releasing session resources");
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await.map_err(ImapError::from)?;
        info!("IMAP logout completed successfully");
        Ok(())
    }

    async fn list_folders(&self) -> Result<Vec<String>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let mut folders_stream = session_guard.list(None, Some("*")).await.map_err(ImapError::from)?;
        let mut folder_names = Vec::new();
        while let Some(folder_result) = folders_stream.try_next().await.map_err(ImapError::from)? {
            folder_names.push(folder_result.name().to_string());
        }
        Ok(folder_names)
    }

    async fn list_folders_hierarchical(&self) -> Result<Vec<crate::imap::types::Folder>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let mut folders_stream = session_guard.list(None, Some("*")).await.map_err(ImapError::from)?;
        let mut folder_data = Vec::new();
        while let Some(folder_result) = folders_stream.try_next().await.map_err(ImapError::from)? {
            let name = folder_result.name().to_string();
            let delimiter = folder_result.delimiter().map(|d| d.to_string());
            let attributes: Vec<String> = folder_result.attributes().iter().map(|attr| format!("{:?}", attr)).collect();
            folder_data.push((name, delimiter, attributes));
        }
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
        let mut folder_guard = self.current_folder.lock().await;
        *folder_guard = Some(name.to_string());
        Ok(())
    }

    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence_set = session_guard.uid_search(criteria).await.map_err(ImapError::from)?;
        Ok(sequence_set.into_iter().collect())
    }

    async fn search_emails_structured(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError> {
        let criteria_string = criteria.to_string();
        if criteria_string.trim().is_empty() {
            return Err(ImapError::InvalidCriteria("Empty search criteria".to_string()));
        }
        debug!("Executing IMAP search with criteria: {}", criteria_string);
        let mut session_guard = self.session.lock().await;
        let sequence_set = session_guard.uid_search(&criteria_string).await.map_err(|e| {
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
        let mut fetch_stream = session_guard.uid_fetch(&sequence, "(FLAGS ENVELOPE INTERNALDATE BODY.PEEK[])").await.map_err(ImapError::from)?;
        let mut emails = Vec::new();
        while let Some(fetch_result) = fetch_stream.try_next().await.map_err(ImapError::from)? {
            let email = Email::from_fetch(&fetch_result)?;
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
        let mut session_guard = self.session.lock().await;
        session_guard.select(from_folder).await.map_err(ImapError::from)?;
        {
            let mut folder_guard = self.current_folder.lock().await;
            *folder_guard = Some(from_folder.to_string());
        }
        let sequence = uid.to_string();

        let move_result = session_guard.uid_mv(&sequence, to_folder).await;
        if move_result.is_ok() {
            return Ok(());
        }

        debug!("MOVE command failed, falling back to COPY+DELETE");

        session_guard.uid_copy(&sequence, to_folder).await.map_err(|e| ImapError::Other(format!("Failed to copy message: {}", e)))?;
        let store_stream = session_guard.uid_store(&sequence, r#"+FLAGS (\Deleted)"#).await.map_err(|e| ImapError::Other(format!("Failed to mark as deleted: {}", e)))?;
        store_stream.try_collect::<Vec<_>>().await.map_err(|e| ImapError::Other(format!("Failed to process store results: {}", e)))?;

        let expunge_stream = session_guard.expunge().await.map_err(|e| ImapError::Other(format!("Failed to expunge: {}", e)))?;
        expunge_stream.try_collect::<Vec<_>>().await.map_err(|e| ImapError::Other(format!("Failed to process expunge results: {}", e)))?;

        Ok(())
    }

    async fn store_flags(&self, uids: &[u32], operation: FlagOperation, flags: &[String]) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        let flags_str = flags.join(" ");
        let op_str = match operation {
            FlagOperation::Add => format!("+FLAGS ({})", flags_str),
            FlagOperation::Remove => format!("-FLAGS ({})", flags_str),
            FlagOperation::Set => format!("FLAGS ({})", flags_str),
        };
        let stream = session_guard.uid_store(&sequence, &op_str).await?;
        stream.try_collect::<Vec<_>>().await.map(|_| ()).map_err(ImapError::from)
    }

    async fn append(&self, folder: &str, content: &[u8], _flags: &[String]) -> Result<(), ImapError> {
        let session_arc = self.session.clone();
        let folder_str = folder.to_string();
        let folder_for_error = folder_str.clone();
        let content = content.to_vec();
        let append_timeout = self.append_timeout;

        info!("Starting IMAP APPEND to folder '{}' with spawn_blocking (timeout: {:?})", folder_str, append_timeout);

        let blocking_task = tokio::task::spawn_blocking(move || {
            let runtime_handle = tokio::runtime::Handle::current();
            let mut session_guard = runtime_handle.block_on(session_arc.lock());
            debug!("Executing IMAP APPEND in blocking thread for folder '{}'", folder_str);
            runtime_handle.block_on(session_guard.append(folder_str, &content))
        });

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
                error!("APPEND to folder '{}' timed out after {:?}. The blocking thread was terminated.", folder_for_error, append_timeout);
                Err(ImapError::Timeout(format!("APPEND operation timed out after {:?}.", append_timeout)))
            }
        }
    }

    async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uid.to_string();
        let mut fetch_stream = session_guard.uid_fetch(&sequence, "BODY.PEEK[]").await.map_err(ImapError::from)?;
        if let Some(fetch_result) = fetch_stream.try_next().await.map_err(ImapError::from)? {
            fetch_result.body().map(|b| b.to_vec()).ok_or_else(|| ImapError::MissingData("Message body not found".to_string()))
        } else {
            Err(ImapError::MissingData("No fetch result found for UID".to_string()))
        }
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        let stream = session_guard.expunge().await?;
        stream.try_collect::<Vec<_>>().await.map(|_| ()).map_err(ImapError::from)
    }

    async fn copy_messages(&self, uids: &[u32], to_folder: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        session_guard.uid_copy(&sequence, to_folder).await.map_err(|e| ImapError::Other(format!("Failed to copy messages: {}", e)))?;
        Ok(())
    }

    async fn move_messages(&self, uids: &[u32], from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() { return Ok(()); }
        self.ensure_folder_selected(from_folder).await?;
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");

        let move_result = session_guard.uid_mv(&sequence, to_folder).await;
        if move_result.is_ok() {
            debug!("Batch MOVE command succeeded for {} messages", uids.len());
            return Ok(());
        }

        debug!("Batch MOVE command failed, falling back to COPY+DELETE+EXPUNGE.");

        session_guard.uid_copy(&sequence, to_folder).await.map_err(|e| ImapError::Other(format!("Failed to copy messages: {}", e)))?;

        let store_stream = session_guard.uid_store(&sequence, r#"+FLAGS (\Deleted)"#).await.map_err(|e| ImapError::Other(format!("Failed to mark messages as deleted: {}", e)))?;
        store_stream.try_collect::<Vec<_>>().await.map_err(|e| ImapError::Other(format!("Failed to process store results: {}", e)))?;

        drop(session_guard);

        self.expunge().await?;

        info!("Successfully moved {} messages from {} to {} using COPY+DELETE+EXPUNGE", uids.len(), from_folder, to_folder);
        Ok(())
    }

    async fn mark_as_deleted(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() { return Ok(()); }
        debug!("Marking {} messages as deleted", uids.len());
        self.store_flags(uids, FlagOperation::Add, &[String::from(r"\Deleted")]).await?;
        info!("Successfully marked {} messages as deleted", uids.len());
        Ok(())
    }

    async fn delete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() { return Ok(()); }
        debug!("Deleting {} messages (mark as deleted + expunge)", uids.len());
        self.mark_as_deleted(uids).await?;
        self.expunge().await?;
        info!("Successfully deleted {} messages permanently", uids.len());
        Ok(())
    }

    async fn undelete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        if uids.is_empty() { return Ok(()); }
        debug!("Removing \\Deleted flag from {} messages", uids.len());
        self.store_flags(uids, FlagOperation::Remove, &[String::from(r"\Deleted")]).await?;
        info!("Successfully undeleted {} messages", uids.len());
        Ok(())
    }

    async fn noop(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.noop().await.map_err(ImapError::from)?;
        debug!("Successfully sent NOOP keepalive command");
        Ok(())
    }
}
