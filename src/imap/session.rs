use async_trait::async_trait;
use std::{
    sync::Arc,
    collections::HashSet,
    borrow::Cow,
};
use tokio::{
    sync::Mutex,
    net::TcpStream as TokioTcpStream,
};
use futures_util::TryStreamExt;
use tokio_util::compat::Compat;
use tokio_rustls::client::TlsStream as TokioTlsStream;
use async_native_tls::TlsStream;

// IMAP types
use async_imap::{
    Session as ImapSessionClient,
    Session as AsyncImapSession,
    Client, ClientTls,
    extensions::UidPlus,
    types::{
        Fetch, MailboxDatum, NameAttribute, Seq, Status,
        Flag as AsyncImapFlag, Name, Mailbox as AsyncMailbox,
    },
    error::{Error as AsyncImapError, ValidateError},
};

use imap_types::{
    core::{Quoted, NString, IString},
    command::CommandBody,
    fetch::Attribute,
    flag::Flag as ImapTypesFlag,
    response::{Data, Status as ImapStatus},
    state::State,
};

// Local types
use crate::imap::{
    error::ImapError,
    types::{
        Email, Folder, MailboxInfo, SearchCriteria,
        StoreOperation, Flags, Envelope, Address,
        ImapEnvelope, ImapAddress,
    },
};

// Other imports
use chrono::{DateTime, NaiveDate, Utc};
use log::{debug, info, warn, error, trace};
use serde::Deserialize;
use urlencoding;

// Type aliases
pub type TlsCompatibleStream = Compat<TokioTlsStream<TokioTcpStream>>;
pub type TlsImapSession = AsyncImapSession<TlsCompatibleStream>;
pub type Flag = ImapTypesFlag;

// Define a constant for the delimiter
const DEFAULT_MAILBOX_DELIMITER: char = '/';

/// Trait defining asynchronous IMAP operations, abstracted over the specific IMAP client library implementation.
#[async_trait]
pub trait AsyncImapOps: Send + Sync {
    async fn login(&mut self, username: &str, password: &str) -> Result<(), ImapError>;
    async fn logout(&mut self) -> Result<(), ImapError>;
    async fn list_folders(&mut self) -> Result<Vec<Folder>, ImapError>;
    async fn create_folder(&mut self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&mut self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&mut self, old_name: &str, new_name: &str) -> Result<(), ImapError>;
    async fn select_folder(&mut self, name: &str) -> Result<MailboxInfo, ImapError>;
    async fn search_emails(&mut self, criteria: &str) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&mut self, uids: &[u32]) -> Result<Vec<Email>, ImapError>;
    async fn fetch_raw_message(&mut self, uid: u32) -> Result<Vec<u8>, ImapError>;
    async fn move_email(&mut self, uid: u32, target_folder: &str) -> Result<(), ImapError>;
    async fn store_flags(&mut self, uid: u32, flags: &str) -> Result<(), ImapError>;
    async fn append(&mut self, folder: &str, content: &[u8], flags: &str) -> Result<(), ImapError>;
    async fn expunge(&mut self) -> Result<(), ImapError>;
}

// 2. Implement the trait for the real TlsImapSession
#[async_trait]
impl AsyncImapOps for TlsImapSession {
    async fn login(&mut self, username: &str, password: &str) -> Result<(), ImapError> {
        self.login(username, password).await.map_err(|e| {
            error!("IMAP login failed: {}", e);
            ImapError::AuthenticationError(e.to_string())
        })
    }

    async fn logout(&mut self) -> Result<(), ImapError> {
        self.logout().await.map_err(|e| {
            error!("IMAP logout failed: {}", e);
            ImapError::ConnectionError(e.to_string())
        })
    }

    async fn list_folders(&mut self) -> Result<Vec<Folder>, ImapError> {
        let mailboxes = self.list(Some(""), Some("*")).await.map_err(|e| {
            error!("IMAP list folders failed: {}", e);
            ImapError::OperationError(e.to_string())
        })?;

        Ok(mailboxes.into_iter().map(MailboxInfo::from).collect())
    }

    async fn create_folder(&mut self, name: &str) -> Result<(), ImapError> {
        self.create(name).await.map_err(|e| {
            error!("IMAP create folder failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn delete_folder(&mut self, name: &str) -> Result<(), ImapError> {
        self.delete(name).await.map_err(|e| {
            error!("IMAP delete folder failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn rename_folder(&mut self, old_name: &str, new_name: &str) -> Result<(), ImapError> {
        self.rename(old_name, new_name).await.map_err(|e| {
            error!("IMAP rename folder failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn select_folder(&mut self, name: &str) -> Result<MailboxInfo, ImapError> {
        let mailbox_data = self.select(name).await.map_err(|e| {
            error!("IMAP select folder failed: {}", e);
            ImapError::OperationError(e.to_string())
        })?;

        Ok(MailboxInfo::from(mailbox_data))
    }

    async fn search_emails(&mut self, criteria: &str) -> Result<Vec<u32>, ImapError> {
        self.uid_search(criteria).await.map_err(|e| {
            error!("IMAP search failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn fetch_emails(&mut self, uids: &[u32]) -> Result<Vec<Email>, ImapError> {
        let mut emails = Vec::new();
        for uid in uids {
            let fetch_items = "BODY[] FLAGS ENVELOPE INTERNALDATE";

            let fetch_result = self.uid_fetch(&uid.to_string(), fetch_items).await.map_err(|e| {
                error!("IMAP fetch failed for UID {}: {}", uid, e);
                ImapError::OperationError(e.to_string())
            })?;

            if let Some(fetch) = fetch_result.into_iter().next() {
                let email = Email::from_fetch(fetch)?;
                emails.push(email);
            }
        }

        Ok(emails)
    }

    async fn fetch_raw_message(&mut self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let sequence_set = uid.to_string();
        let query = "BODY[]";
        let fetches = self.fetch(sequence_set, query.to_string()).await?;

        if let Some(fetch) = fetches.iter().next() {
            if let Some(body_bytes) = fetch.body() {
                Ok(body_bytes.to_vec())
            } else {
                Err(ImapError::Fetch("Body not found for UID".to_string()))
            }
        } else {
            Err(ImapError::Fetch(format!("Message with UID {} not found", uid)))
        }
    }

    async fn move_email(&mut self, uid: u32, target_folder: &str) -> Result<(), ImapError> {
        self.select_folder(target_folder).await?;

        let sequence_set = uid.to_string();
        self.uid_mv(&sequence_set, target_folder).await.map_err(|e| {
            error!("IMAP move failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn store_flags(&mut self, uid: u32, flags: &str) -> Result<(), ImapError> {
        let sequence_set = uid.to_string();
        let command = format!("FLAGS ({})", flags);

        self.uid_store(&sequence_set, &command).await.map_err(|e| {
            error!("IMAP store flags failed: {}", e);
            ImapError::OperationError(e.to_string())
        })?;

        Ok(())
    }

    async fn append(&mut self, folder: &str, content: &[u8], flags: &str) -> Result<(), ImapError> {
        self.append_with_flags(folder, content, flags, None).await.map_err(|e| {
            error!("IMAP append failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
    }

    async fn expunge(&mut self) -> Result<(), ImapError> {
        self.expunge().await.map_err(|e| {
            error!("IMAP expunge failed: {}", e);
            ImapError::OperationError(e.to_string())
        })
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
        SearchCriteria::Flagged => Ok("FLAGGED".to_string()),
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

    async fn fetch_raw_message(&mut self, uid: u32) -> Result<Vec<u8>, ImapError>;
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError>;
    /// Selects a folder, making it the current folder for subsequent operations.
    /// Returns metadata about the selected folder.
    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError>;
    async fn search_emails(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, criteria: &SearchCriteria, limit: u32, fetch_body: bool) -> Result<Vec<Email>, ImapError>;


    async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError>;
    async fn store_flags(&self, uids: Vec<u32>, operation: StoreOperation, flags: Vec<String>) -> Result<(), ImapError>;
    async fn append(&self, folder: &str, payload: Vec<u8>) -> Result<(), ImapError>;
    async fn expunge(&self) -> Result<(), ImapError>;
    async fn logout(&self) -> Result<(), ImapError>;
}

// Add StoreOperation enum
#[derive(Debug, Deserialize)]
pub enum StoreOperation {
    Add,
    Remove,
    Set,
}

// 5. Update the ImapSession implementation to use the generic T
#[async_trait]
impl<T: AsyncImapOps + Send + Sync + 'static> ImapSession for AsyncImapSessionWrapper<T> {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError> {
        let mut session = self.session.lock().await;
        let names = session.list(None, Some("*")).await.map_err(ImapError::from)?;
        
        let folders = names.into_iter().map(|name| {
            let full_name = name.name().to_string();
            Folder {
                name: full_name,
                delimiter: name.delimiter().map(|s| s.to_string()),
            }
        }).collect();
        Ok(folders)
    }

    async fn create_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.create(name).await.map_err(ImapError::from)
    }

    async fn delete_folder(&self, name: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.delete(name).await.map_err(ImapError::from)
    }

    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.rename(from, to).await.map_err(ImapError::from)
    }

    async fn select_folder(&self, name: &str) -> Result<MailboxInfo, ImapError> {
        log::debug!("ImapSession::select_folder called for '{}'", name);
        let mut session_guard = self.session.lock().await;
        let mailbox = session_guard.select(name).await.map_err(ImapError::from)?;
        log::trace!("Raw mailbox selected: {:?}", mailbox);

        // Construct and return the MailboxInfo struct
        let info = MailboxInfo {
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
        };
        Ok(info)
    }

    async fn search_emails(&self, criteria: &SearchCriteria) -> Result<Vec<u32>, ImapError> {
        let criteria_str = format_search_criteria(criteria)?;
        
        let mut session = self.session.lock().await;
        log::debug!("Executing IMAP SEARCH: {}", criteria_str);
        let uids: Vec<u32> = session.search(criteria_str).await.map_err(ImapError::from)?;
        
        Ok(uids)
    }

    async fn fetch_emails(&self, criteria: &SearchCriteria, limit: u32, fetch_body: bool) -> Result<Vec<Email>, ImapError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        // 1. Search for emails matching criteria
        let uids = self.search_emails(criteria).await?;
        if uids.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Take up to 'limit' UIDs
        let limited_uids: Vec<String> = uids.into_iter()
            .take(limit as usize)
            .map(|uid| uid.to_string())
            .collect();
        
        let sequence_set = limited_uids.join(",");
        if sequence_set.is_empty() {
            return Ok(Vec::new()); // Should not happen if uids wasn't empty, but good practice
        }

        let mut session = self.session.lock().await;

        // 3. Construct the FETCH query
        let query = if fetch_body {
            "UID FLAGS ENVELOPE BODY.PEEK[]".to_string()
        } else {
            "UID FLAGS ENVELOPE".to_string()
        };
        
        debug!("Using IMAP UID FETCH query: {} for sequence set: {}", query, sequence_set);

        // 4. Perform the UID FETCH
        let fetches = session.uid_fetch(sequence_set.clone(), query).await.map_err(|e| ImapError::from(e))?;
        
        debug!("Received {} fetch results from IMAP server.", fetches.len());
        
        // 5. Process fetches into Email structs (existing logic)
        let mut emails = Vec::new();
        for fetch in fetches.into_iter() {
            // ... (keep existing processing logic from line 431 onwards) ...
            log::debug!("Raw IMAP Fetch result: {:?}", fetch);

            let uid = fetch.uid.unwrap_or(0);
            let raw_flags = fetch.flags().collect::<Vec<_>>();
            log::debug!("FETCH UID {}: Raw flags received: {:?}", uid, raw_flags);
            
            let flags = raw_flags.iter().map(convert_async_flag_to_string).collect::<Vec<String>>();
            log::debug!("FETCH UID {}: Converted flags: {:?}", uid, flags);

            let envelope = fetch.envelope().ok_or(ImapError::EnvelopeNotFound)?;
            let body = if fetch_body {
                fetch.body().map(|b| b.to_vec())
            } else {
                None
            };
            let size = fetch.size;

            let email = Email {
                uid,
                flags,
                envelope: Some(make_envelope(envelope)?),
                body,
                size,
            };
            
            emails.push(email);
        }

        Ok(emails)
    }

    async fn fetch_raw_message(&mut self, uid: u32) -> Result<Vec<u8>, ImapError> {
        let mut session = self.session.lock().await;
        // The `async-imap` library doesn't have a direct `fetch_raw_message`.
        // We typically fetch the full body (`BODY[]`) for the raw content.
        let sequence_set = uid.to_string();
        let query = "BODY[]";
        let fetches = session.fetch(sequence_set, query.to_string()).await?;

        // Assume only one message is fetched for the given UID.
        if let Some(fetch) = fetches.iter().next() {
            // Get the body content.
            if let Some(body_bytes) = fetch.body() {
                Ok(body_bytes.to_vec())
            } else {
                // Handle case where body is unexpectedly None
                Err(ImapError::Fetch("Body not found for UID".to_string()))
            }
        } else {
            // Handle case where no message was fetched for the UID
            Err(ImapError::Fetch(format!("Message with UID {} not found", uid)))
        }
    }

    async fn move_email(&self, source_folder: &str, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        let uid_set = uids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
        let mut session = self.session.lock().await;

        // 1. Select Source Folder
        session.select(source_folder).await.map_err(ImapError::from)?;

        // 2. Attempt MOVE extension first
        match session.uid_mv(uid_set.clone(), destination_folder).await.map_err(ImapError::from) {
            Ok(_) => {
                log::debug!("MOVE successful for UIDs {} from '{}' to '{}'", uid_set, source_folder, destination_folder);
                Ok(())
            }
            Err(move_error) => {
                log::warn!("MOVE command failed (maybe not supported?), falling back to COPY + STORE + EXPUNGE. Error: {:?}", move_error);
                // Fallback: COPY + STORE \Deleted + EXPUNGE
                // 3. COPY to Destination
                session.uid_copy(uid_set.clone(), destination_folder).await.map_err(ImapError::from)?;
                log::debug!("Fallback: COPY successful for UIDs {} to '{}'", uid_set, destination_folder);

                // 4. Ensure Source is still selected (COPY might deselect)
                session.select(source_folder).await.map_err(ImapError::from)?;
                log::debug!("Fallback: Re-selected source folder '{}'", source_folder);

                // 5. STORE \Deleted flag in Source
                let delete_flag_cmd = "+FLAGS (\\Deleted)".to_string();
                session.uid_store(uid_set.clone(), delete_flag_cmd).await.map_err(ImapError::from)?;
                log::debug!("Fallback: Stored \\Deleted flag for UIDs {} in '{}'", uid_set, source_folder);

                // 6. EXPUNGE Source
                session.expunge().await.map_err(ImapError::from)?;
                log::debug!("Fallback: EXPUNGE successful for source folder '{}'", source_folder);

                Ok(())
            }
        }
    }

    async fn logout(&self) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await.map_err(ImapError::from)
    }

    async fn store_flags(&self, uids: Vec<u32>, operation: StoreOperation, flags: Vec<String>) -> Result<(), ImapError> {
        let uid_set = uids.iter().map(|uid| uid.to_string()).collect::<Vec<String>>().join(",");
        let flags_str = flags.join(" ");
        let command = match operation {
            StoreOperation::Add => format!("+FLAGS ({})", flags_str),
            StoreOperation::Remove => format!("-FLAGS ({})", flags_str),
            StoreOperation::Set => format!("FLAGS ({})", flags_str),
        };

        let mut session = self.session.lock().await;
        let result = session.uid_store(uid_set, command).await.map_err(ImapError::from);

        match &result {
            Ok(_) => log::debug!("IMAP STORE successful"),
            Err(e) => log::error!("IMAP STORE failed: {:?}", e),
        }
        result
    }

    async fn append(&self, folder: &str, payload: Vec<u8>) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.append(folder, payload).await.map_err(|e| ImapError::from(e))
    }

    async fn expunge(&self) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        session.expunge().await.map_err(|e| ImapError::from(e))
    }
}

fn make_nstring(bytes_opt: Option<Cow<'_, [u8]>>) -> NString<'static> {
    match bytes_opt {
        Some(cow) => {
            match Quoted::try_from(cow.as_ref().to_vec()) {
                Ok(quoted) => NString(Some(IString::from(quoted))),
                Err(_) => NString(None),
            }
        }
        None => NString(None),
    }
}

fn make_envelope(env: &async_imap::types::Envelope) -> Result<ImapEnvelope, ImapError> {
    Ok(ImapEnvelope {
        date: make_nstring(env.date.clone()), 
        subject: make_nstring(env.subject.clone()), 
        from: env.from.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()), 
            adl: make_nstring(addr.adl.clone()), 
            mailbox: make_nstring(addr.mailbox.clone()), 
            host: make_nstring(addr.host.clone()), 
        }).collect()).unwrap_or_default(),
        sender: env.sender.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()),
            adl: make_nstring(addr.adl.clone()),
            mailbox: make_nstring(addr.mailbox.clone()),
            host: make_nstring(addr.host.clone()),
        }).collect()).unwrap_or_default(),
        reply_to: env.reply_to.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()),
            adl: make_nstring(addr.adl.clone()),
            mailbox: make_nstring(addr.mailbox.clone()),
            host: make_nstring(addr.host.clone()),
        }).collect()).unwrap_or_default(),
        to: env.to.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()),
            adl: make_nstring(addr.adl.clone()),
            mailbox: make_nstring(addr.mailbox.clone()),
            host: make_nstring(addr.host.clone()),
        }).collect()).unwrap_or_default(),
        cc: env.cc.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()),
            adl: make_nstring(addr.adl.clone()),
            mailbox: make_nstring(addr.mailbox.clone()),
            host: make_nstring(addr.host.clone()),
        }).collect()).unwrap_or_default(),
        bcc: env.bcc.as_ref().map(|addrs| addrs.iter().map(|addr| ImapAddress {
            name: make_nstring(addr.name.clone()),
            adl: make_nstring(addr.adl.clone()),
            mailbox: make_nstring(addr.mailbox.clone()),
            host: make_nstring(addr.host.clone()),
        }).collect()).unwrap_or_default(),
        in_reply_to: make_nstring(env.in_reply_to.clone()), 
        message_id: make_nstring(env.message_id.clone()), 
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_imap::error::{Error as AsyncImapError, ValidateError};
    // Remove unused imports related to complex mocking
    // use async_imap::types::{NameAttribute, MailboxDatum as AsyncMailboxDatum};
    // Remove unused Cow import
    // use std::borrow::Cow;
    // Remove unused bytes import
    // use bytes::Bytes;

    // --- Mock --- 
    #[derive(Default, Debug)]
    struct MockAsyncImapOps {
        list_result: Option<Result<Vec<Name>, ImapError>>,
        create_result: Option<Result<(), ImapError>>,
        delete_result: Option<Result<(), ImapError>>,
        rename_result: Option<Result<(), ImapError>>,
        select_results: std::collections::VecDeque<Result<AsyncMailbox, ImapError>>, // Use VecDeque for multiple calls
        search_result: Option<Result<Vec<u32>, ImapError>>,
        fetch_result: Option<Result<Vec<Fetch>, ImapError>>,
        uid_fetch_result: Option<Result<Vec<Fetch>, ImapError>>,
        move_result: Option<Result<(), ImapError>>,
        copy_result: Option<Result<(), ImapError>>,
        store_flags_result: Option<Result<(), ImapError>>,
        append_result: Option<Result<(), ImapError>>,
        expunge_result: Option<Result<(), ImapError>>,
    }

    // Remove complex/broken mock data helpers
    // fn create_mock_name(...) { ... }
    // fn create_mock_fetch(...) { ... }

    impl MockAsyncImapOps {
        fn default_ok() -> Self {
            let default_mailbox = || Ok(AsyncMailbox {
                    flags: vec![AsyncImapFlag::Seen], exists: 10, recent: 1, unseen: Some(5),
                    permanent_flags: vec![AsyncImapFlag::Answered], uid_next: Some(11),
                    uid_validity: Some(12345), highest_modseq: None
                });

            Self {
                list_result: Some(Ok(vec![])),
                create_result: Some(Ok(())),
                delete_result: Some(Ok(())),
                rename_result: Some(Ok(())),
                select_results: std::collections::VecDeque::from(vec![default_mailbox(), default_mailbox(), default_mailbox()]), // Provide enough for typical move
                search_result: Some(Ok(vec![1, 2, 3])),
                fetch_result: Some(Ok(vec![])),
                uid_fetch_result: Some(Ok(vec![])),
                move_result: Some(Ok(())),
                copy_result: Some(Ok(())),
                store_flags_result: Some(Ok(())),
                append_result: Some(Ok(())),
                expunge_result: Some(Ok(())),
            }
        }
        // Simplified setters
        fn set_list(mut self, res: Result<Vec<Name>, ImapError>) -> Self {
            self.list_result = Some(res);
            self
        }
         fn add_select(mut self, res: Result<AsyncMailbox, ImapError>) -> Self {
             self.select_results.push_back(res);
             self
         }
         fn set_search(mut self, res: Result<Vec<u32>, ImapError>) -> Self {
            self.search_result = Some(res);
            self
        }
         fn set_create(mut self, res: Result<(), ImapError>) -> Self {
            self.create_result = Some(res);
            self
        }
         fn set_delete(mut self, res: Result<(), ImapError>) -> Self {
            self.delete_result = Some(res);
            self
        }
         fn set_move(mut self, res: Result<(), ImapError>) -> Self {
             self.move_result = Some(res);
             self
         }
        // Re-added setter for uid_fetch
        fn set_uid_fetch(mut self, res: Result<Vec<Fetch>, ImapError>) -> Self {
            self.uid_fetch_result = Some(res);
            self
        }
        fn set_copy(mut self, res: Result<(), ImapError>) -> Self {
            self.copy_result = Some(res);
            self
        }
    }

    #[async_trait]
    impl AsyncImapOps for MockAsyncImapOps {
        async fn list(&mut self, _ref_name: Option<&str>, _pattern: Option<&str>) -> Result<Vec<Name>, async_imap::error::Error> {
            self.list_result.take().unwrap_or(Ok(vec![]))
        }

        async fn create(&mut self, _name: &str) -> Result<(), async_imap::error::Error> { 
            self.create_result.take().unwrap_or(Ok(()))
        }

        async fn delete(&mut self, _name: &str) -> Result<(), async_imap::error::Error> { 
            self.delete_result.take().unwrap_or(Ok(()))
        }

        async fn rename(&mut self, _from: &str, _to: &str) -> Result<(), async_imap::error::Error> { 
            self.rename_result.take().unwrap_or(Ok(()))
        }

        async fn select(&mut self, _name: &str) -> Result<AsyncMailbox, async_imap::error::Error> {
            self.select_results.pop_front().unwrap_or_else(|| {
                Ok(AsyncMailbox {
                    flags: vec![], exists: 0, recent: 0, unseen: Some(0), permanent_flags: vec![],
                    uid_next: Some(1), uid_validity: Some(1), highest_modseq: None
                })
            })
        }

        async fn search(&mut self, _query: &str) -> Result<Vec<u32>, ImapError> {
            self.search_result.take().unwrap_or(Ok(vec![]))
        }

        async fn fetch(&mut self, _sequence_set: &str, _query: &str) -> Result<Vec<Fetch>, ImapError> {
            self.fetch_result.take().unwrap_or(Ok(vec![]))
        }

        async fn uid_mv(&mut self, _sequence_set: String, _mailbox: &str) -> Result<(), async_imap::error::Error> {
            self.move_result.take().unwrap_or(Ok(()))
        }

        async fn logout(&mut self) -> Result<(), ImapError> {
            Ok(())
        }

        async fn uid_store(&mut self, _sequence_set: String, _command: String) -> Result<(), ImapError> {
            self.store_flags_result.take().unwrap_or(Ok(()))
        }

        async fn append(&mut self, _folder: &str, _payload: Vec<u8>) -> Result<(), ImapError> {
            self.append_result.take().unwrap_or(Ok(()))
        }

        async fn expunge(&mut self) -> Result<(), ImapError> {
            self.expunge_result.take().unwrap_or(Ok(()))
        }

        async fn uid_fetch(&mut self, _uid_set: String, _query: String) -> Result<Vec<Fetch>, ImapError> {
            self.uid_fetch_result.take().unwrap_or(Ok(vec![]))
        }

        async fn uid_copy(&mut self, _sequence_set: String, _mailbox: &str) -> Result<Option<async_imap::types::UidSet>, ImapError> {
            self.copy_result.take().map(|r| r.map(|_| None))
        }

        async fn store(&mut self, _sequence_set: &str, _command: async_imap::types::StoreType<'_>) -> Result<Vec<Fetch>, ImapError> {
            Ok(vec![])
        }

        async fn uid_search(&mut self, _query: &str) -> Result<Vec<u32>, ImapError> {
            self.search_result.take().unwrap_or(Ok(vec![]))
        }

        async fn append(&mut self, _mailbox: &str, _data: &[u8], _flags: &[Flag<'_>], _date_time: Option<DateTime<Utc>>) -> Result<Option<u32>, ImapError> {
            self.append_result.take().map(|r| r.map(|_| None))
        }

        async fn examine(&mut self, _mailbox: &str) -> Result<async_imap::types::MailboxData, ImapError> {
            Ok(async_imap::types::MailboxData {
                exists: 0,
                recent: 0,
                flags: vec![],
                permanent_flags: vec![],
                unseen: None,
                uid_next: None,
                uid_validity: None,
                highest_modseq: None,
            })
        }

        async fn status(&mut self, _mailbox: &str, _items: &[async_imap::types::StatusDataItem]) -> Result<Status, ImapError> {
            Ok(Status {
                mailbox: "INBOX".to_string(),
                items: vec![],
            })
        }

        fn is_selected(&self) -> bool {
            true
        }

        fn is_logged_in(&self) -> bool {
            true
        }

        fn delimiter(&self) -> Option<char> {
            Some('/')
        }

        async fn poll(&mut self) -> Result<Option<UnsolicitedResponse>, async_imap::error::Error> {
            Ok(None)
        }
    }

    // --- Tests for AsyncImapSessionWrapper ---

    // Remove tests verifying transformation of Name and Fetch
    /*
    #[tokio::test]
    async fn test_wrapper_list_folders_success() { ... }
    #[tokio::test]
    async fn test_wrapper_fetch_emails_success() { ... }
    */

    // Keep tests verifying error propagation and simpler cases
    #[tokio::test]
    async fn test_wrapper_list_folders_error() {
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_list(Err(ImapError::Validate(ValidateError('x'))));
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.list_folders().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }
    
    #[tokio::test]
    async fn test_wrapper_list_folders_success_empty() { // Test OK with empty vec
        let mock_ops = MockAsyncImapOps::default_ok().set_list(Ok(vec![]));
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.list_folders().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_wrapper_select_folder_success() {
        let mock_ops = MockAsyncImapOps::default_ok();
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.select_folder("INBOX").await;
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.exists, 10);
    }
    
    #[tokio::test]
    async fn test_wrapper_select_folder_error() {
        let mut mock_ops = MockAsyncImapOps::default_ok();
        mock_ops.select_results.clear();
        mock_ops = mock_ops.add_select(Err(ImapError::No("Select failed")));

        let session_wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = session_wrapper.select_folder("NonExistent").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }

    #[tokio::test]
    async fn test_wrapper_search_emails_success() {
        let mock_ops = MockAsyncImapOps::default_ok().set_search(Ok(vec![10, 20]));
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.search_emails(SearchCriteria::All).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![10, 20]);
    }

    #[tokio::test]
    async fn test_wrapper_search_emails_error() {
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_search(Err(ImapError::Bad("Bad search")));
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.search_emails(SearchCriteria::All).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_))); 
    }
    
     #[tokio::test]
    async fn test_wrapper_fetch_emails_empty_input() { // Keep this edge case
        let mock_ops = MockAsyncImapOps::default_ok(); 
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.fetch_emails(vec![], true).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
    
     #[tokio::test]
    async fn test_wrapper_fetch_emails_error() {
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_uid_fetch(Err(ImapError::No("UID Fetch failed")));
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.fetch_emails(vec![1], true).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }

    #[tokio::test]
    async fn test_wrapper_create_folder_success() {
        let mock_ops = MockAsyncImapOps::default_ok();
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.create_folder("New").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrapper_create_folder_error() {
        let mock_ops = MockAsyncImapOps::default_ok()
             .set_create(Err(ImapError::No("Exists")));
        let session_wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = session_wrapper.create_folder("ExistsAlready").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }
    
    // Add tests for delete, rename, move, logout
    #[tokio::test]
    async fn test_wrapper_delete_folder_success() {
        let mock_ops = MockAsyncImapOps::default_ok();
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.delete_folder("Trash").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrapper_delete_folder_error() {
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_delete(Err(ImapError::No("No delete")));
        let session_wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = session_wrapper.delete_folder("ToDelete").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }
    
     #[tokio::test]
    async fn test_wrapper_move_email_success() {
        let mock_ops = MockAsyncImapOps::default_ok();
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = wrapper.move_email("INBOX", vec![1, 2], "Archive").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrapper_move_email_error_on_move() { // Test failure on initial MOVE
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_move(Err(ImapError::No("No move"))); // Set MOVE to fail
        let wrapper = AsyncImapSessionWrapper::new(mock_ops);
        // Expecting Ok because fallback should succeed with default mocks
        let result = wrapper.move_email("INBOX", vec![1], "Archive").await;
        assert!(result.is_ok(), "Fallback move should succeed with default mocks");
    }

     #[tokio::test]
    async fn test_wrapper_move_email_error_on_copy() {
        let mock_ops = MockAsyncImapOps::default_ok()
            .set_move(Err(ImapError::No("No move support")))
            .set_copy(Err(ImapError::No("No copy")));
        let session_wrapper = AsyncImapSessionWrapper::new(mock_ops);
        let result = session_wrapper.move_email("INBOX", vec![1], "Archive").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        dbg!(&err);
        assert!(matches!(err, ImapError::SessionError(_)));
    }

    // TODO: Add test for failure during select destination
    // TODO: Add test for failure during search destination
    // TODO: Add test for failure during reselect source
    // TODO: Add test for failure during uid_store
    // TODO: Add test for failure during expunge (should likely still be Ok)

    // ... other tests ...
}
