use crate::imap::error::ImapError;
use crate::imap::session::{AsyncImapSessionWrapper, ImapSession, TlsImapSession};
use crate::imap::types::{Email, Folder, SearchCriteria, OwnedMailbox};
use async_imap::Client as AsyncImapClientProto;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio::net::TcpStream as TokioTcpStream;
use tokio_rustls::TlsConnector;
use rustls::pki_types::{IpAddr as PkiIpAddr, ServerName as PkiServerName};
use tokio_rustls::rustls::{Certificate, ClientConfig, RootCertStore, ServerName as RustlsServerName};
use rustls_native_certs;
use std::sync::Arc;
use std::time::Duration;

/// High-level asynchronous IMAP client.
/// Wraps an ImapSession to provide a simpler interface.
/// Note: ImapSession implementation will change significantly.
#[derive(Clone)]
pub struct ImapClient {
    // Use the ImapSession trait object - implementation will use async-imap
    session: Arc<dyn ImapSession>,
}

// Helper function for the actual connection and login logic with timeout
async fn connect_and_login(
    server: &str,
    port: u16,
    username: &str,
    password: &str,
    connect_timeout: Duration,
) -> Result<TlsImapSession, ImapError> { // Return the async-imap Session
    log::info!("Connecting to IMAP server: {}:{}", server, port);
    
    // --- TLS Setup --- 
    let pki_server_name = PkiServerName::try_from(server)
        .map_err(|_| ImapError::Connection(format!("Invalid server name format: {}", server)))?;
    let rustls_server_name = match pki_server_name {
        PkiServerName::DnsName(dns) => RustlsServerName::try_from(dns.as_ref())
            .map_err(|e| ImapError::Connection(format!("Failed to convert DNS name: {}", e)))?,
        // Manually match PkiIpAddr and construct std::net::IpAddr
        PkiServerName::IpAddress(pki_ip) => {
            let std_ip = match pki_ip {
                PkiIpAddr::V4(pki_v4_addr) => std::net::IpAddr::V4(std::net::Ipv4Addr::from(pki_v4_addr)),
                PkiIpAddr::V6(pki_v6_addr) => std::net::IpAddr::V6(std::net::Ipv6Addr::from(pki_v6_addr)),
            };
            RustlsServerName::IpAddress(std_ip)
        }
        _ => return Err(ImapError::Connection("Server name must be a DNS name or IP address".into())),
    };
    
    let mut root_store = RootCertStore::empty();
    match rustls_native_certs::load_native_certs() {
        Ok(certs) => {
            for cert in certs {
                // Use Certificate from tokio_rustls::rustls
                if root_store.add(&Certificate(cert.clone().to_vec())).is_err() {
                    log::warn!("Failed to add native certificate to root store");
                }
            }
        }
        Err(e) => {
            log::error!("Could not load native certificates: {}", e);
            return Err(ImapError::Connection("Failed to load native root certificates".to_string()));
        }
    }

    if root_store.is_empty() {
        log::warn!("No native root certificates loaded, TLS connection might fail verification");
    }

    // Re-add with_safe_defaults() and pass root_store directly
    let config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let addr = format!("{}:{}", server, port);

    // --- TCP Connect with Timeout --- 
    log::debug!("Attempting TCP connection to {}...", addr);
    let tcp_stream = tokio::time::timeout(connect_timeout, TokioTcpStream::connect(&addr))
        .await
        .map_err(|_| ImapError::Connection(format!("TCP connection timeout ({:?}) to {}", connect_timeout, addr)))?
        .map_err(|e| ImapError::Connection(format!("TCP connect error to {}: {}", addr, e)))?;
    log::debug!("TCP connection successful.");

    // --- TLS Handshake with Timeout --- 
    log::debug!("Performing TLS handshake with {}...", server);
    let tls_stream = tokio::time::timeout(connect_timeout, connector.connect(rustls_server_name.clone(), tcp_stream))
        .await
        .map_err(|_| ImapError::Connection(format!("TLS handshake timeout ({:?}) with {}", connect_timeout, addr)))?
        .map_err(|e| ImapError::Connection(format!("TLS handshake error with {}: {}", addr, e)))?;
    log::debug!("TLS handshake successful.");

    // --- IMAP Client Creation and Login --- 
    let compat_tls_stream = tls_stream.compat();
    log::debug!("Creating IMAP client...");
    let client = AsyncImapClientProto::new(compat_tls_stream);
    log::info!("Logging in as user: {}", username);
    let session = client.login(username, password)
        .await
        .map_err(|(e, _client)| {
            log::error!("IMAP login failed for user {}: {}", username, e);
            ImapError::Authentication(format!("Login failed: {}", e))
        })?;
    log::info!("IMAP login successful for user {}.", username);

    Ok(session) // Return the established async-imap session
}

impl ImapClient {
    /// Creates a new ImapClient with a provided session implementation.
    /// The signature might change depending on the final session implementation.
    pub fn new(session: Arc<dyn ImapSession>) -> Self {
        ImapClient { session }
    }

    /// Establishes a connection to the IMAP server and logs in.
    /// Returns an Arc<dyn ImapSession> which needs to be used to create ImapClient.
    /// This decouples connection/login from the client struct itself.
    pub async fn connect(
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        timeout: Option<Duration>,
    ) -> Result<Arc<dyn ImapSession>, ImapError> {
        let connect_timeout = timeout.unwrap_or_else(|| Duration::from_secs(10));
        
        // Call the helper to get the logged-in async-imap session
        let async_session = connect_and_login(server, port, username, password, connect_timeout).await?;
        
        // Wrap the session
        let session_wrapper = Arc::new(AsyncImapSessionWrapper::new(async_session));
        Ok(session_wrapper)
    }

    // Pass-through methods to the ImapSession trait object
    pub async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        self.session.list_folders().await
    }

    pub async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.create_folder(name).await
    }

    pub async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        self.session.delete_folder(name).await
    }

    pub async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        self.session.rename_folder(from, to).await
    }

    pub async fn select_folder(&self, name: &str) -> Result<OwnedMailbox, ImapError> {
        self.session.select_folder(name).await
    }

    pub async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        self.session.search_emails(criteria).await
    }

    pub async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> {
        self.session.fetch_emails(uids).await
    }

    pub async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        self.session.move_email(uids, destination_folder).await
    }

    /* // TODO: Fix append implementation in ImapSession trait first
    pub async fn append(&self, folder: &str, body: &[u8], flags: Option<Vec<&str>>) -> Result<(), ImapError> {
        self.session.append(folder, body, flags).await
    }
    */

    pub async fn logout(self) -> Result<(), ImapError> {
        log::info!("Logging out...");
        // Clone the Arc before moving it into logout
        let session_arc = self.session.clone(); 
        session_arc.logout().await?; // Call logout on the Arc<dyn ImapSession>
        Ok(())
    }
}

// Remove old map_address function, it will be handled within session.rs refactoring
// Remove old certificate verification mod, imap crate handles this via rustls config
