// Standard library imports
use std::sync::Arc;
use std::time::Duration;

// External crate imports
use async_imap::{Client as AsyncImapClient, Session as AsyncImapSession};
use async_imap::error::Error as AsyncImapNativeError;
use async_imap::State as AsyncImapState;
use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use log::info;
use rustls::pki_types::ServerName as PkiServerName;
use rustls::{ClientConfig, RootCertStore};
use tokio::net::TcpStream as TokioTcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_rustls::{client::TlsStream as TokioTlsStreamClient, TlsConnector};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

// IMAP types
use imap_types::{
    command::CommandBody,
    fetch::Attribute as FetchAttribute,
    flag::Flag as ImapFlag,
    response::{Capability, Status as ResponseStatus},
    sequence::SequenceSet,
    status::StatusDataItem,
};

// Internal imports
use crate::imap::error::ImapError;
use crate::imap::session::{AsyncImapOps, AsyncImapSessionWrapper, ImapSession, StoreOperation};
use crate::imap::types::{
    AppendEmailPayload, Email, ExpungeResponse, FlagOperation, Flags,
    Folder, MailboxInfo, ModifyFlagsPayload, SearchCriteria,
};

// Type Aliases
type BaseTcpStream = TokioTcpStream;
type BaseTlsStream = TokioTlsStreamClient<BaseTcpStream>;
type CompatStream = Compat<BaseTlsStream>;
type UnderlyingImapSession = AsyncImapSession<CompatStream>;

/// High-level asynchronous IMAP client providing a simplified interface.
///
/// This client handles the connection, TLS setup, login, and delegates
/// operations to an underlying `ImapSession`.
pub struct ImapClient {
    session: Arc<Mutex<dyn AsyncImapOps + Send + Sync>>,
}

impl ImapClient {
    pub fn new(session: Arc<Mutex<dyn AsyncImapOps + Send + Sync>>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl AsyncImapOps for ImapClient {
    async fn login(&mut self, username: &str, password: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.login(username, password).await
    }

    async fn logout(&mut self) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.logout().await
    }

    async fn list_folders(&mut self) -> Result<Vec<Folder>, ImapError> {
        let mut session = self.session.lock().await;
        session.list_folders().await
    }

    async fn create_folder(&mut self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.create_folder(name).await
    }

    async fn delete_folder(&mut self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.delete_folder(name).await
    }

    async fn rename_folder(&mut self, old_name: &str, new_name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.rename_folder(old_name, new_name).await
    }

    async fn select_folder(&mut self, name: &str) -> Result<MailboxInfo, ImapError> {
        let mut session = self.session.lock().await;
        session.select_folder(name).await
    }

    async fn search_emails(&mut self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        let mut session = self.session.lock().await;
        session.search_emails(criteria).await
    }

    async fn fetch_emails(&mut self, uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        let mut session = self.session.lock().await;
        session.fetch_emails(uids).await
    }

    async fn fetch_raw_message(&mut self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let mut session = self.session.lock().await;
        session.fetch_raw_message(uid).await
    }

    async fn move_email(&mut self, uid: u32, target_folder: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.move_email(uid, target_folder).await
    }

    async fn store_flags(&mut self, uid: u32, flags: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.store_flags(uid, flags).await
    }

    async fn append(&mut self, folder: &str, content: &[u8], flags: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.append(folder, content, flags).await
    }

    async fn expunge(&mut self) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.expunge().await
    }
}

// Public API methods that convert between domain types and IMAP types
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
        info!("Public connect called for user '{}' at {}:{}", username, host, port);
        let login_timeout = Duration::from_secs(30); // Example timeout

        // Call internal logic to get the raw underlying session
        let underlying_session =
            connect_and_login_internal(host, port, username, password, login_timeout).await?;

        // Wrap the raw session with our domain-specific logic/trait implementation
        let session_wrapper = AsyncImapSessionWrapper::new(underlying_session);

        // Prepare the trait object for the client struct
        let session_arc_mutex: Arc<Mutex<dyn AsyncImapOps + Send + Sync>> = Arc::new(Mutex::new(session_wrapper));

        Ok(Self {
            session: session_arc_mutex,
        })
    }

    pub async fn store_flags_with_operation(&self, uids: Vec<u32>, operation: FlagOperation, flags: Flags) -> Result<(), ImapError> {
        let store_op = match operation {
            FlagOperation::Add => StoreOperation::Add,
            FlagOperation::Remove => StoreOperation::Remove,
            FlagOperation::Set => StoreOperation::Set,
        };
        self.store_flags(&uids, store_op, &flags.items).await
    }

    pub async fn append_email(&self, folder: &str, payload: AppendEmailPayload) -> Result<(), ImapError> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(&payload.content)
             .map_err(|e| ImapError::Encoding(format!("Invalid base64 content: {}", e)))?;
        self.append(folder, &bytes, None, None).await
    }

    pub async fn expunge_folder(&self) -> Result<ExpungeResponse, ImapError> {
        self.expunge().await?;
        Ok(ExpungeResponse {
            message: "Expunge successful.".to_string(),
        })
    }

    pub fn current_folder(&self) -> Option<String> {
        let guard = self.session.try_lock().ok()?;
        guard.current_folder()
    }
}

// --- Internal Connection Logic ---

/// Establishes TCP connection, performs TLS handshake, and configures the stream.
async fn setup_tls_stream(
    host: &str,
    port: u16,
    tls_connector: TlsConnector,
    server_name_for_tls: PkiServerName<'static>,
) -> Result<BaseTlsStream, ImapError> {
    info!("Attempting TCP connection to {}:{}...", host, port);
    let tcp_stream = BaseTcpStream::connect((host, port)).await?;
    info!("TCP connected. Performing TLS handshake...");

    let tls_stream = tls_connector.connect(server_name_for_tls, tcp_stream).await?;
    info!("TLS handshake successful.");
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
    info!("IMAP client created. Attempting login for user '{}'...", username);

    match timeout(timeout_duration, client.login(username, password)).await {
        Ok(Ok(session)) => {
            info!("IMAP login successful for user: {}", username);
            Ok(session)
        }
        Ok(Err((e, _client))) => {
            info!("IMAP login failed for user {}: {:?}", username, e);
            Err(ImapError::from(e))
        }
        Err(elapsed_err) => {
            info!("IMAP login timed out for user {} after {:?}. Error: {}", username, timeout_duration, elapsed_err);
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
    info!("Starting internal connection process for {}:{}", host, port);

    // --- Server Name Setup ---
    let host_owned = host.to_string();
    let server_name_static: PkiServerName<'static> = PkiServerName::try_from(host_owned)
        .map_err(|_| ImapError::ConnectionError(format!("Invalid server name format: {}", host)))?;

    // --- TLS Configuration ---
    let mut root_cert_store = RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs()?;
    let (added, ignored) = root_cert_store.add_parsable_certificates(certs);
    info!("Loaded {} native certs, ignored {}.", added, ignored);
    if added == 0 && ignored > 0 {
        info!("No valid native certs found, TLS connection might fail.");
    }
    if root_cert_store.is_empty() {
        info!("Root certificate store is empty after loading native certs.");
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

