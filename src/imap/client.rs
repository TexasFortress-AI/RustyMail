// use crate::config::Settings;
use crate::imap::error::ImapError;
use crate::imap::session::{AsyncImapSessionWrapper, ImapSession, StoreOperation};
use crate::imap::types::{Email, Folder, SearchCriteria, MailboxInfo, FlagOperation, Flags, AppendEmailPayload, ExpungeResponse};
use async_imap::{Client as AsyncImapClient, Session as AsyncImapSession};
use async_trait::async_trait;
use log;
use rustls::pki_types::ServerName as PkiServerName;
use rustls::{ClientConfig, RootCertStore};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream as TokioTcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_rustls::{client::TlsStream as TokioTlsStreamClient, TlsConnector};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

// --- Type Aliases ---

// Concrete Tokio types
type BaseTcpStream = TokioTcpStream;
type BaseTlsStream = TokioTlsStreamClient<BaseTcpStream>;

// Compatibility wrapper for async_imap
type CompatStream = Compat<BaseTlsStream>;

// The actual session type returned by async_imap::login
type UnderlyingImapSession = AsyncImapSession<CompatStream>;

/// High-level asynchronous IMAP client providing a simplified interface.
///
/// This client handles the connection, TLS setup, login, and delegates
/// operations to an underlying `ImapSession`.
pub struct ImapClient {
    session: Arc<Mutex<dyn ImapSession>>,
}

#[async_trait]
pub trait ImapClientTrait: Send + Sync {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError>;
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError>;
    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError>;
    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, uids: Vec<u32>, fetch_body: bool) -> Result<Vec<Email>, ImapError>;
    async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError>;
    async fn store_flags(&self, uids: Vec<u32>, operation: StoreOperation, flags: Vec<String>) -> Result<(), ImapError>;
    async fn append(&self, folder: &str, payload: Vec<u8>) -> Result<(), ImapError>;
    async fn expunge(&self) -> Result<(), ImapError>;
    async fn logout(&self) -> Result<(), ImapError>;
}

// --- Internal Connection Logic ---

/// Establishes TCP connection, performs TLS handshake, and configures the stream.
async fn setup_tls_stream(
    host: &str,
    port: u16,
    tls_connector: TlsConnector,
    server_name_for_tls: PkiServerName<'static>,
) -> Result<BaseTlsStream, ImapError> {
    log::debug!("Attempting TCP connection to {}:{}...", host, port);
    let tcp_stream = BaseTcpStream::connect((host, port)).await?;
    log::debug!("TCP connected. Performing TLS handshake...");

    let tls_stream = tls_connector.connect(server_name_for_tls, tcp_stream).await?;
    log::debug!("TLS handshake successful.");
    Ok(tls_stream)
}

/// Performs IMAP login using the compatible stream.
async fn perform_imap_login(
    compat_stream: CompatStream,
    username: &str,
    password: &str,
    timeout_duration: Duration,
) -> Result<UnderlyingImapSession, ImapError> {
    let client = AsyncImapClient::new(compat_stream);
    log::debug!("IMAP client created. Attempting login for user '{}'...", username);

    match timeout(timeout_duration, client.login(username, password)).await {
        Ok(Ok(session)) => {
            log::info!("IMAP login successful for user: {}", username);
            Ok(session)
        }
        Ok(Err((e, _client))) => {
            log::error!("IMAP login failed for user {}: {:?}", username, e);
            Err(ImapError::from(e))
        }
        Err(elapsed_err) => {
            log::error!("IMAP login timed out for user {} after {:?}. Error: {}", username, timeout_duration, elapsed_err);
            Err(ImapError::ConnectionError("Login timed out".to_string()))
        }
    }
}

/// Internal helper to connect, setup TLS, and login, returning the raw session.
async fn connect_and_login_internal(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    timeout_duration: Duration,
) -> Result<UnderlyingImapSession, ImapError> {
    log::info!("Starting internal connection process for {}:{}", host, port);

    // --- Server Name Setup ---
    let host_owned = host.to_string();
    let server_name_static: PkiServerName<'static> = PkiServerName::try_from(host_owned)
        .map_err(|_| ImapError::ConnectionError(format!("Invalid server name format: {}", host)))?;

    // --- TLS Configuration ---
    let mut root_cert_store = RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs()?;
    let (added, ignored) = root_cert_store.add_parsable_certificates(certs);
    log::debug!("Loaded {} native certs, ignored {}.", added, ignored);
    if added == 0 && ignored > 0 {
        log::warn!("No valid native certs found, TLS connection might fail.");
    }
    if root_cert_store.is_empty() {
        log::warn!("Root certificate store is empty after loading native certs.");
    }

    let config = ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let tls_connector = TlsConnector::from(Arc::new(config));

    // --- Connect, TLS Handshake ---
    let tls_stream = setup_tls_stream(host, port, tls_connector, server_name_static).await?;

    // --- Login ---
    let compat_stream = tls_stream.compat();
    perform_imap_login(compat_stream, username, password, timeout_duration).await
}

// --- Public ImapClient Implementation ---

impl ImapClient {
    /// Establishes a connection to the IMAP server, logs in, and returns a new `ImapClient`.
    ///
    /// This is the primary way to create an `ImapClient`.
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Result<Self, ImapError> {
        log::info!("Public connect called for user '{}' at {}:{}", username, host, port);
        let login_timeout = Duration::from_secs(30); // Example timeout

        // Call internal logic to get the raw underlying session
        let underlying_session =
            connect_and_login_internal(host, port, username, password, login_timeout).await?;

        // Wrap the raw session with our domain-specific logic/trait implementation
        let wrapped_session = AsyncImapSessionWrapper::new(underlying_session);

        // Prepare the trait object for the client struct
        let session_arc_mutex: Arc<Mutex<dyn ImapSession>> = Arc::new(Mutex::new(wrapped_session));

        Ok(Self {
            session: session_arc_mutex,
        })
    }

    /// Creates a new `ImapClient` instance directly from a pre-existing session trait object.
    /// Useful for testing or scenarios where the session is managed externally.
    pub fn new_with_session(session: Arc<Mutex<dyn ImapSession>>) -> Self {
        Self { session }
    }

    // --- Delegated IMAP Operations ---

    pub async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        self.session.lock().await.list_folders().await
    }

    pub async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.lock().await.create_folder(name).await
    }

    pub async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.lock().await.delete_folder(name).await
    }

    pub async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        self.session.lock().await.rename_folder(from, to).await
    }

    pub async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError> {
        self.session.lock().await.select_folder(name).await
    }

    pub async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        self.session.lock().await.search_emails(criteria).await
    }

    pub async fn fetch_emails(&self, uids: Vec<u32>, fetch_body: bool) -> Result<Vec<Email>, ImapError> {
        if uids.is_empty() {
            return Ok(Vec::new()); // Handle empty UIDs directly
        }
        self.session.lock().await.fetch_emails(uids, fetch_body).await
    }

    pub async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }
        self.session.lock().await.move_email(source_folder, uids, destination_folder).await
    }

    /// Modifies flags for specified emails.
    pub async fn store_flags(&self, uids: Vec<u32>, operation: FlagOperation, flags: Flags) -> Result<(), ImapError> {
        let store_op = match operation {
            FlagOperation::Add => StoreOperation::Add,
            FlagOperation::Remove => StoreOperation::Remove,
            FlagOperation::Set => StoreOperation::Set,
        };
        let flag_strings = flags.items;
        self.session.lock().await.store_flags(uids, store_op, flag_strings).await
    }

    /// Appends an email to the specified folder.
    pub async fn append(&self, folder: &str, payload: AppendEmailPayload) -> Result<Option<u32>, ImapError> {
        // Convert AppendEmailPayload to Vec<u8>
        let bytes = payload.content.into_bytes();
        self.session.lock().await.append(folder, bytes).await.map(|_| None)
    }

    /// Expunges emails marked for deletion in the currently selected folder.
    pub async fn expunge(&self) -> Result<ExpungeResponse, ImapError> {
        self.session.lock().await.expunge().await.map(|_| ExpungeResponse {
            message: "Expunge successful.".to_string(),
        })
    }

    pub async fn fetch_raw_message(&self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let mut session = self.session.lock().await;
        session.fetch_raw_message(uid).await
    }

    /// Logs out from the IMAP server.
    /// Note: This consumes the client to prevent further operations after logout.
    /// The underlying session's logout implementation should handle cleanup.
    pub async fn logout(self) -> Result<(), ImapError> {
        let session_guard = self.session.lock().await;
        session_guard.logout().await
    }
}

#[async_trait]
impl ImapClientTrait for ImapClient {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        self.session.lock().await.list_folders().await
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.lock().await.create_folder(name).await
    }

    async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.lock().await.delete_folder(name).await
    }

    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        self.session.lock().await.rename_folder(from, to).await
    }

    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError> {
        self.session.lock().await.select_folder(name).await
    }

    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        self.session.lock().await.search_emails(criteria).await
    }

    async fn fetch_emails(&self, uids: Vec<u32>, fetch_body: bool) -> Result<Vec<Email>, ImapError> {
        self.session.lock().await.fetch_emails(uids, fetch_body).await
    }

    async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        self.session.lock().await.move_email(source_folder, uids, destination_folder).await
    }

    async fn store_flags(&self, uids: Vec<u32>, operation: StoreOperation, flags: Vec<String>) -> Result<(), ImapError> {
        self.session.lock().await.store_flags(uids, operation, flags).await
    }

    async fn append(&self, folder: &str, payload: Vec<u8>) -> Result<(), ImapError> {
        self.session.lock().await.append(folder, payload).await
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        self.session.lock().await.expunge().await
    }

    async fn logout(&self) -> Result<(), ImapError> {
        self.session.lock().await.logout().await
    }
}
