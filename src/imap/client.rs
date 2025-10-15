// Standard library imports
use std::{
    fmt::Debug,
    // borrow::Cow, // Unused
    net::ToSocketAddrs,
    sync::Arc,
    time::Duration,
};

// Async runtime and utilities
use tokio_native_tls::TlsConnector as TokioTlsConnector;
// use async_imap::error::Error as AsyncImapNativeError; // Unused
// use async_trait::async_trait; // Unused
// use futures_util::stream::StreamExt; // Not directly used here, but used by async_imap::Client::connect
// use chrono::{DateTime, Utc}; // Unused
use log::{info}; // Keep used logs

// TLS and crypto
// use rustls::{ClientConfig, RootCertStore}; // Unused
use native_tls::TlsConnector;
use tokio::net::TcpStream as TokioTcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;

// IMAP types (imap-types crate) - REMOVED: All imports were unused

// Local types
use crate::imap::{
    error::ImapError,
    session::{AsyncImapOps, AsyncImapSessionWrapper, TlsImapSession},
    // Folder, MailboxInfo, ModifyFlagsPayload, SearchCriteria, // Unused
};

// Async IMAP types (async-imap crate)
use async_imap::{
    // types::{ // Unused types
    //     Fetch as AsyncImapFetch,
    //     Flag as AsyncImapFlag,
    //     Name as AsyncImapName,
    //     // Status as AsyncImapStatus, // Unresolved and Unused
    //     Mailbox as AsyncImapMailbox,
    //     UnsolicitedResponse,
    // },
    // client::Client as AsyncImapClient, // Unused and possibly private
    // Session as AsyncSession, // Keep if needed for connect
    Client as AsyncImapInternalClient, // Renamed to avoid clash
};

/// High-level IMAP client providing a simplified interface for common operations.
#[derive(Debug, Clone)]
pub struct ImapClient<T: AsyncImapOps + Send + Sync + Debug + 'static> {
    session: Arc<T>,
}

impl<T: AsyncImapOps + Send + Sync + Debug + 'static> ImapClient<T> {
    /// Creates a new `ImapClient` wrapping an existing session.
    pub fn new(session: T) -> Self {
        Self { session: Arc::new(session) }
    }

    /// Establishes a new IMAP connection with the given server, port, and credentials
    pub async fn connect(server: &str, port: u16, username: &str, password: &str) -> Result<ImapClient<AsyncImapSessionWrapper>, ImapError> {
        let session = AsyncImapSessionWrapper::connect(server, port, 
            Arc::new(username.to_string()), 
            Arc::new(password.to_string())).await?;
        Ok(ImapClient::new(session))
    }

    /// Provides direct access to the underlying session operations.
    pub fn session(&self) -> &T {
        &self.session
    }

    /// Returns the Arc-wrapped session for sharing across threads/tasks
    pub fn session_arc(&self) -> Arc<T> {
        self.session.clone()
    }

    // Add convenience methods here that delegate to self.session
    pub async fn list_folders(&self) -> Result<Vec<String>, ImapError> {
        self.session.list_folders().await
    }

    pub async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.create_folder(name).await
    }

    pub async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.delete_folder(name).await
    }

    pub async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError> {
        self.session.rename_folder(old_name, new_name).await
    }

    pub async fn select_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.select_folder(name).await
    }

    pub async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        self.session.search_emails(criteria).await
    }

    pub async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<crate::imap::types::Email>, ImapError> {
        self.session.fetch_emails(uids).await
    }

    pub async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        self.session.move_email(uid, from_folder, to_folder).await
    }

    pub async fn store_flags(&self, uids: &[u32], operation: crate::imap::types::FlagOperation, flags: &[String]) -> Result<(), ImapError> {
        self.session.store_flags(uids, operation, flags).await
    }

    pub async fn append(&self, folder: &str, content: &[u8], flags: &[String]) -> Result<(), ImapError> {
        self.session.append(folder, content, flags).await
    }

    pub async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError> {
        self.session.fetch_raw_message(uid).await
    }

    pub async fn expunge(&self) -> Result<(), ImapError> {
        self.session.expunge().await
    }

    pub async fn mark_as_deleted(&self, uids: &[u32]) -> Result<(), ImapError> {
        self.session.mark_as_deleted(uids).await
    }

    pub async fn delete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        self.session.delete_messages(uids).await
    }

    pub async fn undelete_messages(&self, uids: &[u32]) -> Result<(), ImapError> {
        self.session.undelete_messages(uids).await
    }

    pub async fn noop(&self) -> Result<(), ImapError> {
        self.session.noop().await
    }

    pub async fn logout(&self) -> Result<(), ImapError> {
        self.session.logout().await
    }
}

/// Establishes a TLS-encrypted IMAP connection.
pub async fn connect(
    server: &str,
    port: u16,
    username: &str,
    password: &str,
    timeout: Duration,
) -> Result<ImapClient<AsyncImapSessionWrapper>, ImapError> {
    let addr = (server, port)
        .to_socket_addrs()? 
        .next()
        .ok_or_else(|| ImapError::Connection("Invalid server address".to_string()))?;

    info!("Connecting to IMAP server: {}", addr);

    // Establish Tokio TCP connection
    let tcp_stream = tokio::time::timeout(timeout, TokioTcpStream::connect(addr))
        .await
        .map_err(|_| ImapError::Timeout("Connection timed out".to_string()))??; 

    // Setup TLS connector
    let tls_builder = TlsConnector::builder();
    let native_tls_connector = tls_builder.build()
        .map_err(|e| ImapError::Tls(format!("Failed to build TLS connector: {}", e)))?;
    let tls_connector = TokioTlsConnector::from(native_tls_connector);

    // Perform TLS handshake with timeout (tokio_native_tls works with tokio's AsyncRead/Write)
    let tls_stream = tokio::time::timeout(timeout, tls_connector.connect(server, tcp_stream))
        .await
        .map_err(|_| ImapError::Timeout("Operation timed out".to_string()))?
        .map_err(|e| ImapError::Tls(e.to_string()))?;

    info!("TLS connection established");

    // Build IMAP client with the TLS stream wrapped in compat for async-imap
    // The client itself is the unauthenticated session - no need to call connect
    let unauthenticated_session = AsyncImapInternalClient::new(tls_stream.compat());
    
    info!("IMAP session established");

    // Login with timeout (login returns the authenticated session)
    let authenticated_session = tokio::time::timeout(timeout, unauthenticated_session.login(username, password))
        .await
        .map_err(|_| ImapError::Timeout("Login timed out".to_string()))?
        .map_err(|(err, _client)| ImapError::from(err))?;

    info!("IMAP login successful for user: {}", username);

    // Wrap the authenticated session in our mutex wrapper
    let wrapped_session = AsyncImapSessionWrapper::new(authenticated_session);

    // Create our client using the wrapped session
    Ok(ImapClient::new(wrapped_session))
}

