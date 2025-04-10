use serde::{Deserialize, Serialize};
// Remove unused import
// use imap_types::envelope::Envelope;
use imap_types::core::{NString};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fmt;

// Custom Email struct (ensure it's public)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Email {
    pub uid: u32,
    pub flags: Vec<String>,
    pub internal_date: Option<DateTime<Utc>>,
    pub envelope: Option<Envelope>,
    pub body: Option<Vec<u8>>,
}

// Custom Folder struct (ensure it's public)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    pub name: String,
    pub delimiter: Option<String>,
}

/// Represents information about a selected mailbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxInfo {
    pub name: String,
    pub delimiter: Option<char>,
    pub attributes: Vec<String>,
    pub exists: u32,
    pub recent: u32,
    pub unseen: Option<u32>,
}

impl From<async_imap::types::Name> for MailboxInfo {
    fn from(name: async_imap::types::Name) -> Self {
        Self {
            name: name.name().to_string(),
            delimiter: name.delimiter(),
            attributes: name.attributes().iter().map(|a| a.to_string()).collect(),
            exists: 0,
            recent: 0,
            unseen: None,
        }
    }
}

impl From<async_imap::types::MailboxData> for MailboxInfo {
    fn from(data: async_imap::types::MailboxData) -> Self {
        Self {
            name: data.name,
            delimiter: None, // Not available in MailboxData
            attributes: vec![], // Not available in MailboxData
            exists: data.exists,
            recent: data.recent,
            unseen: data.unseen,
        }
    }
}

// Custom SearchCriteria enum (ensure it's public)
#[derive(Debug, Clone, Deserialize)]
pub enum SearchCriteria {
    All,
    Answered,
    Deleted,
    Draft,
    Flagged,
    New,
    Old,
    Recent,
    Seen,
    Unanswered,
    Undeleted,
    Undraft,
    Unflagged,
    Unseen,
    Before(DateTime<Utc>),
    On(DateTime<Utc>),
    Since(DateTime<Utc>),
    Body(String),
    From(String),
    Subject(String),
    Text(String),
    To(String),
    Uid(Vec<u32>),
    And(Vec<SearchCriteria>),
    Or(Vec<SearchCriteria>),
    Not(Box<SearchCriteria>),
}

impl fmt::Display for SearchCriteria {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchCriteria::All => write!(f, "ALL"),
            SearchCriteria::Answered => write!(f, "ANSWERED"),
            SearchCriteria::Deleted => write!(f, "DELETED"),
            SearchCriteria::Draft => write!(f, "DRAFT"),
            SearchCriteria::Flagged => write!(f, "FLAGGED"),
            SearchCriteria::New => write!(f, "NEW"),
            SearchCriteria::Old => write!(f, "OLD"),
            SearchCriteria::Recent => write!(f, "RECENT"),
            SearchCriteria::Seen => write!(f, "SEEN"),
            SearchCriteria::Unanswered => write!(f, "UNANSWERED"),
            SearchCriteria::Undeleted => write!(f, "UNDELETED"),
            SearchCriteria::Undraft => write!(f, "UNDRAFT"),
            SearchCriteria::Unflagged => write!(f, "UNFLAGGED"),
            SearchCriteria::Unseen => write!(f, "UNSEEN"),
            SearchCriteria::Before(date) => write!(f, "BEFORE {}", date.format("%d-%b-%Y")),
            SearchCriteria::On(date) => write!(f, "ON {}", date.format("%d-%b-%Y")),
            SearchCriteria::Since(date) => write!(f, "SINCE {}", date.format("%d-%b-%Y")),
            SearchCriteria::Body(text) => write!(f, "BODY {}", text),
            SearchCriteria::From(text) => write!(f, "FROM {}", text),
            SearchCriteria::Subject(text) => write!(f, "SUBJECT {}", text),
            SearchCriteria::Text(text) => write!(f, "TEXT {}", text),
            SearchCriteria::To(text) => write!(f, "TO {}", text),
            SearchCriteria::Uid(uids) => write!(f, "UID {}", uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",")),
            SearchCriteria::And(criteria) => write!(f, "({})", criteria.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")),
            SearchCriteria::Or(criteria) => write!(f, "(OR {})", criteria.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")),
            SearchCriteria::Not(criterion) => write!(f, "NOT {}", criterion),
        }
    }
}

// --- New Types for Added Features ---

/// Represents the operation to perform on flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlagOperation {
    Add,
    Remove,
    Set,
}

/// Represents a list of flags for modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flags {
    // Represent flags as simple strings for now
    #[serde(default)]
    pub items: Vec<String>,
}

/// Payload for modifying email flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyFlagsPayload {
    pub uids: Vec<u32>,
    pub operation: FlagOperation,
    pub flags: Flags, // Use the Flags struct
}

/// Payload for appending an email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEmailPayload {
    // Raw email content as bytes/string
    pub content: String, // Or consider bytes if more appropriate
    pub flags: Flags, // Flags to set on the appended message
}

/// Response after expunging a folder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExpungeResponse {
    pub message: String,
    // Potentially add expunged UIDs if the command returns them
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImapAddress {
    pub name: NString<'static>,
    pub adl: NString<'static>,
    pub mailbox: NString<'static>,
    pub host: NString<'static>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImapEnvelope {
    pub date: NString<'static>,
    pub subject: NString<'static>,
    pub from: Vec<ImapAddress>,
    pub sender: Vec<ImapAddress>,
    pub reply_to: Vec<ImapAddress>,
    pub to: Vec<ImapAddress>,
    pub cc: Vec<ImapAddress>,
    pub bcc: Vec<ImapAddress>,
    pub in_reply_to: NString<'static>,
    pub message_id: NString<'static>,
}

/// Represents a set of UIDs for IMAP operations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UidSet {
    pub items: Vec<u32>,
}

/// Represents an unsolicited response from the IMAP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnsolicitedResponse {
    Exists(u32),
    Recent(u32),
    Expunge(u32),
    Flags(Vec<String>),
    FetchFlags { uid: u32, flags: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub date: Option<String>,
    pub subject: Option<String>,
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub in_reply_to: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub name: Option<String>,
    pub mailbox: Option<String>,
    pub host: Option<String>,
}

impl Email {
    pub fn from_fetch(fetch: async_imap::types::Fetch) -> Result<Self, ImapError> {
        let uid = fetch.uid.ok_or_else(|| ImapError::OperationError("No UID in fetch response".to_string()))?;
        let flags = fetch.flags().map(|f| f.to_string()).collect();
        let internal_date = fetch.internal_date;
        
        let envelope = if let Some(env) = fetch.envelope() {
            Some(Envelope {
                date: env.date.map(|s| s.to_string()),
                subject: env.subject.map(|s| s.to_string()),
                from: env.from.into_iter().map(Address::from).collect(),
                to: env.to.into_iter().map(Address::from).collect(),
                cc: env.cc.into_iter().map(Address::from).collect(),
                bcc: env.bcc.into_iter().map(Address::from).collect(),
                reply_to: env.reply_to.into_iter().map(Address::from).collect(),
                in_reply_to: env.in_reply_to.map(|s| s.to_string()),
                message_id: env.message_id.map(|s| s.to_string()),
            })
        } else {
            None
        };

        let body = fetch.body().map(|b| b.to_vec());

        Ok(Self {
            uid,
            flags,
            internal_date,
            envelope,
            body,
        })
    }
}

impl From<async_imap::types::Address> for Address {
    fn from(addr: async_imap::types::Address) -> Self {
        Self {
            name: addr.name.map(|s| s.to_string()),
            mailbox: addr.mailbox.map(|s| s.to_string()),
            host: addr.host.map(|s| s.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StoreOperation {
    Add,
    Remove,
    Set,
}

#[derive(Debug, thiserror::Error)]
pub enum ImapError {
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Operation failed: {0}")]
    OperationError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<async_imap::error::Error> for ImapError {
    fn from(err: async_imap::error::Error) -> Self {
        match err {
            async_imap::error::Error::Io(e) => ImapError::ConnectionError(e.to_string()),
            async_imap::error::Error::Parse(e) => ImapError::ParseError(e.to_string()),
            async_imap::error::Error::No(e) => ImapError::OperationError(e.to_string()),
            async_imap::error::Error::Bad(e) => ImapError::OperationError(e.to_string()),
            _ => ImapError::OperationError(err.to_string()),
        }
    }
}
