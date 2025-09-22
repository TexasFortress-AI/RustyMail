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

    /// Lists all folders in the mailbox
    async fn list_folders(&self) -> Result<Vec<String>, ImapError>;
    
    /// Creates a new folder with the given name
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Deletes an existing folder
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Renames a folder from old_name to new_name
    async fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<(), ImapError>;
    
    /// Selects a folder for subsequent operations
    async fn select_folder(&self, name: &str) -> Result<(), ImapError>;
    
    /// Searches for emails matching the given criteria
    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError>;
    
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
}

// Wrapper definition using Arc<Mutex<...>>
#[derive(Debug, Clone)] // Add Clone
pub struct AsyncImapSessionWrapper {
    // Wrap the session in Arc<Mutex> for interior mutability
    session: Arc<TokioMutex<TlsImapSession>>,
}

impl AsyncImapSessionWrapper {
    pub fn new(session: TlsImapSession) -> Self {
        Self {
            // Create the Arc<Mutex<>> here
            session: Arc::new(TokioMutex::new(session)),
        }
    }

    pub async fn connect(
        server: &str,
        port: u16,
        username: Arc<String>,
        password: Arc<String>
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

        Ok(Self::new(session))
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
        session_guard.select(name).await.map(|_| ()).map_err(ImapError::from)
    }

    async fn search_emails(&self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence_set = session_guard.search(criteria)
            .await
            .map_err(ImapError::from)?;
        Ok(sequence_set.into_iter().collect())
    }

    async fn fetch_emails(&self, uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let sequence = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        let mut fetch_stream = session_guard.uid_fetch(&sequence, "(FLAGS ENVELOPE INTERNALDATE BODY[])")
            .await
            .map_err(ImapError::from)?;
        
        let mut emails = Vec::new();
        while let Some(fetch_result) = fetch_stream.try_next().await.map_err(ImapError::from)? {
            emails.push(Email::from(fetch_result)); 
        }
        Ok(emails)
    }

    async fn move_email(&self, uid: u32, from_folder: &str, to_folder: &str) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        // Select might modify session state, needs &mut
        session_guard.select(from_folder).await.map_err(ImapError::from)?;
        
        let sequence = uid.to_string();
        // uid_mv also likely needs &mut
        session_guard.uid_mv(&sequence, to_folder).await.map_err(ImapError::from)
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
        let mut session_guard = self.session.lock().await;
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

        // The append method takes the mailbox name and content
        session_guard.append(folder, content)
            .await
            .map_err(ImapError::from)?;
        Ok(())
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
