use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::TryStreamExt;
use tokio_util::compat::Compat;
use tokio_rustls::client::TlsStream as TokioTlsStream;
use tokio::net::TcpStream as TokioTcpStream;
use std::collections::HashSet; // Needed for search conversion

use async_imap::{
    Session as AsyncImapSession, 
    types::{Flag as AsyncImapFlag, Name, Fetch, Mailbox as AsyncMailbox},
};

use crate::imap::types::{Email, Folder, MailboxInfo, SearchCriteria};
use crate::imap::error::ImapError;

// Type alias for the stream compatible with futures_util::io traits
pub type TlsImapSession = AsyncImapSession<Compat<TokioTlsStream<TokioTcpStream>>>;

// 1. Define the trait abstracting async_imap::Session operations
#[async_trait]
pub trait AsyncImapOps: Send + Sync {
    async fn list(&mut self, reference_name: Option<&str>, mailbox_pattern: Option<&str>)
        -> Result<Vec<Name>, async_imap::error::Error>;
    
    async fn create(&mut self, mailbox_name: &str) -> Result<(), async_imap::error::Error>;
    
    async fn delete(&mut self, mailbox_name: &str) -> Result<(), async_imap::error::Error>;
    
    async fn rename(&mut self, current_mailbox_name: &str, new_mailbox_name: &str) -> Result<(), async_imap::error::Error>;
    
    async fn select(&mut self, mailbox_name: &str) -> Result<AsyncMailbox, async_imap::error::Error>;
    
    async fn search(&mut self, query: String) -> Result<Vec<u32>, async_imap::error::Error>;

    async fn fetch(&mut self, sequence_set: String, query: String)
        -> Result<Vec<Fetch>, async_imap::error::Error>;

    async fn uid_mv(&mut self, sequence_set: String, mailbox: &str) -> Result<(), async_imap::error::Error>;

    async fn logout(&mut self) -> Result<(), async_imap::error::Error>;

    // Add other methods as needed, e.g., uid_store for setting flags
    // async fn uid_store(&mut self, sequence_set: String, command: &str, flags: String) -> Result<(), async_imap::error::Error>;
}

// 2. Implement the trait for the real TlsImapSession
#[async_trait]
impl AsyncImapOps for TlsImapSession {
    async fn list(&mut self, reference_name: Option<&str>, mailbox_pattern: Option<&str>)
        -> Result<Vec<Name>, async_imap::error::Error> {
        let stream = self.list(reference_name, mailbox_pattern).await?;
        stream.try_collect().await
    }

    async fn create(&mut self, mailbox_name: &str) -> Result<(), async_imap::error::Error> {
        self.create(mailbox_name).await
    }

    async fn delete(&mut self, mailbox_name: &str) -> Result<(), async_imap::error::Error> {
        self.delete(mailbox_name).await
    }

    async fn rename(&mut self, current_mailbox_name: &str, new_mailbox_name: &str) -> Result<(), async_imap::error::Error> {
        self.rename(current_mailbox_name, new_mailbox_name).await
    }

    async fn select(&mut self, mailbox_name: &str) -> Result<AsyncMailbox, async_imap::error::Error> {
        self.select(mailbox_name).await
    }

    async fn search(&mut self, query: String) -> Result<Vec<u32>, async_imap::error::Error> {
        let uids_set: HashSet<u32> = self.search(query).await?;
        Ok(uids_set.into_iter().collect())
    }

    async fn fetch(&mut self, sequence_set: String, query: String)
        -> Result<Vec<Fetch>, async_imap::error::Error> {
        let stream = self.fetch(sequence_set, query).await?;
        stream.try_collect().await
    }

    async fn uid_mv(&mut self, sequence_set: String, mailbox: &str) -> Result<(), async_imap::error::Error> {
        self.uid_mv(sequence_set, mailbox).await
    }

    async fn logout(&mut self) -> Result<(), async_imap::error::Error> {
        self.logout().await
    }
}

// 3. Make AsyncImapSessionWrapper generic
/// Wrapper around a type implementing `AsyncImapOps` that implements our `ImapSession` trait.
pub struct AsyncImapSessionWrapper<T: AsyncImapOps + Send + Sync + 'static> {
    // Store the generic session type
    session: Arc<Mutex<T>>, 
}

// 4. Adjust the constructor
impl<T: AsyncImapOps + Send + Sync + 'static> AsyncImapSessionWrapper<T> {
    // The constructor now takes any type T implementing AsyncImapOps
    pub(crate) fn new(session: T) -> Self { 
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
    /// Selects a folder, making it the current folder for subsequent operations.
    /// Returns metadata about the selected folder.
    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError>;
    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError>;
    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError>;
    async fn logout(&self) -> Result<(), ImapError>;
}

// 5. Update the ImapSession implementation to use the generic T
#[async_trait]
impl<T: AsyncImapOps + Send + Sync + 'static> ImapSession for AsyncImapSessionWrapper<T> {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        let mut session = self.session.lock().await;
        let names: Vec<Name> = session.list(Some(""), Some("*")).await?;
        
        Ok(names.into_iter().map(|name| {
            Folder {
                name: name.name().to_string(), 
                delimiter: name.delimiter().map(|d| d.to_string()),
            }
        }).collect())
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.create(name).await?;
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

    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError> {
        log::debug!("ImapSession::select_folder called for '{}'", name);
        let mut session_guard = self.session.lock().await;
        let mailbox = session_guard.select(name).await?;
        log::trace!("Raw mailbox selected: {:?}", mailbox);

        // Construct and return the MailboxInfo struct
        Ok(MailboxInfo {
            flags: mailbox
                .flags
                .iter()
                .map(convert_async_flag_to_string)
                .collect(),
            exists: mailbox.exists,
            recent: mailbox.recent,
            unseen: mailbox.unseen,
            permanent_flags: mailbox
                .permanent_flags
                .iter()
                .map(convert_async_flag_to_string)
                .collect(),
            uid_next: mailbox.uid_next,
            uid_validity: mailbox.uid_validity,
        })
    }

    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        let search_string = format_search_criteria(&criteria)?;
        let mut session = self.session.lock().await;
        let uids: Vec<u32> = session.search(search_string).await?;
        Ok(uids)
    }

    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let sequence_set = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        let query = "FLAGS RFC822.SIZE ENVELOPE".to_string();

        let mut session = self.session.lock().await;
        let messages: Vec<Fetch> = session.fetch(sequence_set, query).await?;

        Ok(messages.into_iter().map(|fetch| {
            Email {
                uid: fetch.uid.unwrap_or(0),
                flags: fetch.flags().map(|f| convert_async_flag_to_string(&f)).collect(),
                size: fetch.size,
                envelope: None,
            }
        }).collect())
    }

    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(());
        }
        let mut session = self.session.lock().await;
        let seq_set_str = uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join(",");
        session.uid_mv(seq_set_str, destination_folder).await?;
        Ok(())
    }

    async fn logout(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_imap::error::{Error as AsyncImapError, ValidateError};

    #[derive(Default)]
    struct MockAsyncImapOps {
        folders_to_list: Option<Result<Vec<Name>, AsyncImapError>>,
        mailbox_to_select: Option<Result<AsyncMailbox, AsyncImapError>>,
        search_results: Option<Result<Vec<u32>, AsyncImapError>>,
        fetch_results: Option<Result<Vec<Fetch>, AsyncImapError>>,
        should_create_fail: bool,
        should_delete_fail: bool,
        should_rename_fail: bool,
        should_select_fail: bool,
        should_search_fail: bool,
        should_fetch_fail: bool,
        should_move_fail: bool,
        should_logout_fail: bool,
    }

    #[async_trait]
    impl AsyncImapOps for MockAsyncImapOps {
        async fn list(&mut self, _ref_name: Option<&str>, _pattern: Option<&str>) -> Result<Vec<Name>, AsyncImapError> {
            self.folders_to_list.take().unwrap_or_else(|| Ok(Vec::new()))
        }
        async fn create(&mut self, _name: &str) -> Result<(), AsyncImapError> { 
            if self.should_create_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { Ok(()) }
        }
        async fn delete(&mut self, _name: &str) -> Result<(), AsyncImapError> { 
            if self.should_delete_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { Ok(()) }
        }
        async fn rename(&mut self, _from: &str, _to: &str) -> Result<(), AsyncImapError> { 
            if self.should_rename_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { Ok(()) }
        }
        async fn select(&mut self, _name: &str) -> Result<AsyncMailbox, AsyncImapError> {
             if self.should_select_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { 
                self.mailbox_to_select.take().unwrap_or_else(|| 
                    Ok(AsyncMailbox {
                         flags: Vec::new(),
                         exists: 10,
                         recent: 1,
                         unseen: Some(5),
                         permanent_flags: Vec::new(),
                         uid_next: Some(11),
                         uid_validity: Some(12345),
                         highest_modseq: None, 
                    })
                )
            }
        }
        async fn search(&mut self, _query: String) -> Result<Vec<u32>, AsyncImapError> {
             if self.should_search_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else {
                 self.search_results.take().unwrap_or_else(|| Ok(vec![1,2,3]))
            }
        }
        async fn fetch(&mut self, _sequence_set: String, _query: String) -> Result<Vec<Fetch>, AsyncImapError> {
             if self.should_fetch_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else {
                self.fetch_results.take().unwrap_or_else(|| Ok(Vec::new()))
            }
        }
        async fn uid_mv(&mut self, _sequence_set: String, _mailbox: &str) -> Result<(), AsyncImapError> {
             if self.should_move_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { Ok(()) }
        }
        async fn logout(&mut self) -> Result<(), AsyncImapError> { 
             if self.should_logout_fail { 
                Err(AsyncImapError::Validate(ValidateError('?')))
            } else { Ok(()) }
        }
    }

    // TODO: Add tests for ImapSession implementation using MockAsyncImapOps
    // These tests would cover error mapping, data transformation etc.
} 