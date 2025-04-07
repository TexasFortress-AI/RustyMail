use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::TryStreamExt;
use tokio_util::compat::Compat;
use tokio_rustls::client::TlsStream as TokioTlsStream;
use tokio::net::TcpStream as TokioTcpStream;

use async_imap::{Session as AsyncImapSession, types::{Flag as AsyncImapFlag, Name, Fetch, Mailbox as AsyncMailbox}};
// Remove unused imap_types imports
// use imap_types::mailbox::Mailbox;
// use imap_types::response::Data;
// use imap_types::status::Status;
// use imap_types::ToStatic;

use crate::imap::types::{Email, Folder, SearchCriteria};
use crate::imap::error::ImapError;

// Type alias for the stream compatible with futures_util::io traits
// Remove unused type alias
// type CompatStream = Compat<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>;

// Update the main session type alias to use the compatible stream
// Make it pub(crate) so client.rs can use it
pub type TlsImapSession = AsyncImapSession<Compat<TokioTlsStream<TokioTcpStream>>>;

/// Wrapper around `async_imap::Session` that implements our `ImapSession` trait.
pub struct AsyncImapSessionWrapper {
    // Store the session that uses the compatible stream
    session: Arc<Mutex<TlsImapSession>>, 
}

impl AsyncImapSessionWrapper {
    // The constructor now takes the async_imap::Session directly
    // Make it pub(crate) so client.rs can use it
    pub(crate) fn new(session: TlsImapSession) -> Self { 
        Self { session: Arc::new(Mutex::new(session)) }
    }
}

// Helper to convert our SearchCriteria into the format needed by async-imap
fn format_search_criteria(criteria: &SearchCriteria) -> Result<String, ImapError> {
    match criteria {
        SearchCriteria::All => Ok("ALL".to_string()),
        SearchCriteria::Unseen => Ok("UNSEEN".to_string()),
        SearchCriteria::From(s) => Ok(format!("FROM \"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))),
        SearchCriteria::Subject(s) => Ok(format!("SUBJECT \"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))),
        SearchCriteria::Body(s) => Ok(format!("BODY \"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))),
        SearchCriteria::To(s) => Ok(format!("TO \"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))),
        SearchCriteria::Uid(uid_set) => Ok(format!("UID {}", uid_set)),
        SearchCriteria::Since(date_str) => {
            Ok(format!("SINCE {}", date_str))
        },
        SearchCriteria::And(_) => Err(ImapError::Command("Complex search criteria (And) not yet supported".to_string())),
        SearchCriteria::Or(_) => Err(ImapError::Command("Complex search criteria (Or) not yet supported".to_string())),
        SearchCriteria::Not(_) => Err(ImapError::Command("Complex search criteria (Not) not yet supported".to_string())),
    }
}

// Helper to convert async_imap::types::Flag to our String representation
fn convert_async_flag_to_string(flag: &AsyncImapFlag) -> String {
    match flag {
        AsyncImapFlag::Seen => "\\Seen".to_string(),
        AsyncImapFlag::Answered => "\\Answered".to_string(),
        AsyncImapFlag::Flagged => "\\Flagged".to_string(),
        AsyncImapFlag::Deleted => "\\Deleted".to_string(),
        AsyncImapFlag::Draft => "\\Draft".to_string(),
        AsyncImapFlag::Recent => "\\Recent".to_string(),
        AsyncImapFlag::MayCreate => "\\MayCreate".to_string(),
        AsyncImapFlag::Custom(s) => s.to_string(),
    }
}

#[async_trait]
pub trait ImapSession: Send + Sync {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError>;
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError>;
    async fn select_folder(&self, name: &str) -> Result<AsyncMailbox, ImapError>;
    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError>;
    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError>;
    async fn logout(self: Arc<Self>) -> Result<(), ImapError>;
}

#[async_trait]
impl ImapSession for AsyncImapSessionWrapper {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        let mut session = self.session.lock().await;
        let names_stream = session.list(Some(""), Some("*")).await?;
        // Collect the Name objects from the stream
        let names: Vec<Name> = names_stream.try_collect().await?;
        
        Ok(names.into_iter().map(|name| {
            Folder {
                name: name.name().to_string(), 
                delimiter: name.delimiter().map(|d| d.to_string()),
            }
        }).collect())
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.create(name).await?; // Directly await the future
        Ok(())
    }

    async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.delete(name).await?; 
        Ok(())
    }

    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.rename(from, to).await?; 
        Ok(())
    }

    async fn select_folder(&self, name: &str) -> Result<AsyncMailbox, ImapError> {
        let mut session_guard = self.session.lock().await;
        let session = &mut *session_guard;
        let async_mailbox = session.select(name).await?;
        Ok(async_mailbox)
    }

    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        let search_string = format_search_criteria(&criteria)?;
        let mut session_guard = self.session.lock().await;
        let session = &mut *session_guard;
        let sequence_set = session.search(search_string).await?;
        let uids: Vec<u32> = sequence_set.into_iter().collect();
        Ok(uids)
    }

    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let sequence_set = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        let query = "FLAGS RFC822.SIZE ENVELOPE";

        let mut session_guard = self.session.lock().await;
        let session = &mut *session_guard;
        let message_stream = session.fetch(sequence_set, query).await?;
        let messages: Vec<Fetch> = message_stream.try_collect().await?;

        Ok(messages.into_iter().map(|fetch| {
            Email {
                uid: fetch.uid.unwrap_or(0),
                flags: fetch.flags().map(|f| convert_async_flag_to_string(&f)).collect(),
                size: fetch.size,
                envelope: None, // TODO: Implement conversion
            }
        }).collect())
    }

    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(()); // Nothing to move
        }
        let mut session = self.session.lock().await;
        let seq_set_str = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        session.uid_mv(seq_set_str, destination_folder).await?;
        Ok(())
    }

    async fn logout(self: Arc<Self>) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // ... any test code ...
} 