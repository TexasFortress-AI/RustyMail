use async_trait::async_trait;
use imap::Session;
use parking_lot::Mutex as ParkingLotMutex;
use std::sync::{Arc, Mutex as StdMutex};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Serialize)]
pub struct ZeroCopy<T> {
    pub inner: T,
}

impl<T> ZeroCopy<T> {
    pub fn from(inner: T) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Error, Clone)]
pub enum ImapClientError {
    #[error("IMAP error: {0}")]
    ImapError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Folder error: {0}")]
    FolderError(String),
    #[error("Email error: {0}")]
    EmailError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<imap::Error> for ImapClientError {
    fn from(err: imap::Error) -> Self {
        match err {
            imap::Error::No(msg) => ImapClientError::FolderError(msg.to_string()),
            imap::Error::Bad(msg) => ImapClientError::EmailError(msg.to_string()),
            imap::Error::Append => ImapClientError::EmailError("Failed to append message".into()),
            imap::Error::Parse(e) => ImapClientError::ImapError(e.to_string()),
            imap::Error::Validate(e) => ImapClientError::ImapError(e.to_string()),
            imap::Error::Io(io_err) => ImapClientError::IoError(io_err.to_string()),
            _ => ImapClientError::InternalError(format!("IMAP Error: {}", err)),
        }
    }
}

impl From<native_tls::Error> for ImapClientError {
    fn from(err: native_tls::Error) -> Self {
        ImapClientError::TlsError(err.to_string())
    }
}

impl From<std::io::Error> for ImapClientError {
    fn from(err: std::io::Error) -> Self {
        ImapClientError::IoError(err.to_string())
    }
}

pub type ActualImapSession = Session<native_tls::TlsStream<std::net::TcpStream>>;

#[async_trait]
pub trait ImapSessionTrait: Send + Sync {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError>;
    async fn create(&self, name: &str) -> Result<(), ImapClientError>;
    async fn delete(&self, name: &str) -> Result<(), ImapClientError>;
    async fn select(&self, name: &str) -> Result<(), ImapClientError>;
    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapClientError>;
    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError>;
    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError>;
    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapClientError>;
    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapClientError>;
    async fn logout(&self) -> Result<(), ImapClientError>;
}

#[async_trait]
impl ImapSessionTrait for ParkingLotMutex<ActualImapSession> {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock();
        session.list(None, Some("*"))
            .map(|folders| ZeroCopy::from(folders.iter().map(|f| f.name().to_string()).collect()))
            .map_err(Into::into)
    }

    async fn create(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.create(name).map_err(Into::into)
    }

    async fn delete(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.delete(name).map_err(Into::into)
    }

    async fn select(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.select(name).map(|_| ()).map_err(Into::into)
    }

    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapClientError> {
        let mut session = self.lock();
        session.search(query)
            .map(|set| set.into_iter().collect())
            .map_err(Into::into)
    }

    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock();
        session.fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock();
        session.uid_fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.uid_mv(sequence, mailbox).map_err(Into::into)
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.rename(from, to).map_err(Into::into)
    }

    async fn logout(&self) -> Result<(), ImapClientError> {
        let mut session = self.lock();
        session.logout().map_err(Into::into)
    }
}

#[async_trait]
impl ImapSessionTrait for StdMutex<ActualImapSession> {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.list(None, Some("*"))
            .map(|folders| ZeroCopy::from(folders.iter().map(|f| f.name().to_string()).collect()))
            .map_err(Into::into)
    }

    async fn create(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.create(name).map_err(Into::into)
    }

    async fn delete(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.delete(name).map_err(Into::into)
    }

    async fn select(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.select(name).map(|_| ()).map_err(Into::into)
    }

    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.search(query)
            .map(|set| set.into_iter().collect())
            .map_err(Into::into)
    }

    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.uid_fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.uid_mv(sequence, mailbox).map_err(Into::into)
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.rename(from, to).map_err(Into::into)
    }

    async fn logout(&self) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.logout().map_err(Into::into)
    }
}

#[async_trait]
impl ImapSessionTrait for Arc<StdMutex<ActualImapSession>> {
    async fn list(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.list(None, Some("*"))
            .map(|folders| ZeroCopy::from(folders.iter().map(|f| f.name().to_string()).collect()))
            .map_err(Into::into)
    }

    async fn create(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.create(name).map_err(Into::into)
    }

    async fn delete(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.delete(name).map_err(Into::into)
    }

    async fn select(&self, name: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.select(name).map(|_| ()).map_err(Into::into)
    }

    async fn search(&self, query: &str) -> Result<Vec<u32>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.search(query)
            .map(|set| set.into_iter().collect())
            .map_err(Into::into)
    }

    async fn fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_fetch(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        let mut session = self.lock().unwrap();
        session.uid_fetch(sequence, "RFC822")
            .map(|messages| ZeroCopy::from(messages.iter()
                .map(|m| String::from_utf8_lossy(m.body().unwrap_or_default()).to_string())
                .collect()))
            .map_err(Into::into)
    }

    async fn uid_move(&self, sequence: &str, mailbox: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.uid_mv(sequence, mailbox).map_err(Into::into)
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.rename(from, to).map_err(Into::into)
    }

    async fn logout(&self) -> Result<(), ImapClientError> {
        let mut session = self.lock().unwrap();
        session.logout().map_err(Into::into)
    }
}

pub struct ImapClient<S: ImapSessionTrait> {
    session: S,
}

impl<S: ImapSessionTrait> ImapClient<S> {
    pub fn new(session: S) -> Self {
        Self { session }
    }

    pub async fn list_folders(&self) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.session.list().await
    }

    pub async fn create_folder(&self, name: &str) -> Result<(), ImapClientError> {
        self.session.create(name).await
    }

    pub async fn delete_folder(&self, name: &str) -> Result<(), ImapClientError> {
        self.session.delete(name).await
    }

    pub async fn select_folder(&self, name: &str) -> Result<(), ImapClientError> {
        self.session.select(name).await
    }

    pub async fn search_emails(&self, query: &str) -> Result<Vec<u32>, ImapClientError> {
        self.session.search(query).await
    }

    pub async fn fetch_email(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.session.fetch(sequence).await
    }

    pub async fn fetch_email_by_uid(&self, sequence: &str) -> Result<ZeroCopy<Vec<String>>, ImapClientError> {
        self.session.uid_fetch(sequence).await
    }

    pub async fn move_email(&self, sequence: &str, mailbox: &str) -> Result<(), ImapClientError> {
        self.session.uid_move(sequence, mailbox).await
    }

    pub async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapClientError> {
        self.session.rename(from, to).await
    }

    pub async fn logout(&self) -> Result<(), ImapClientError> {
        self.session.logout().await
    }
} 