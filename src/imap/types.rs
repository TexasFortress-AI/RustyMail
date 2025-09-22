use std::{
    borrow::Cow,
    collections::HashMap,
    fmt,
    convert::Infallible,
};
// use std::str::FromStr; // Unused

use async_imap::types::{
    Fetch,
    Flag as AsyncImapFlag,
    Name as AsyncImapName,
    Mailbox as AsyncImapMailbox,
};
use chrono::{DateTime, Utc};
// imap_types removed - NString was unused 
use serde::{Deserialize, Serialize};
// use thiserror::Error; // Unused

use crate::imap::error::ImapError;
use crate::imap::session::DEFAULT_MAILBOX_DELIMITER;

/// Represents an email message in the IMAP system.
///
/// This struct encapsulates all the essential information about an email message,
/// including its unique identifier, flags, metadata, and content.
///
/// # Examples
///
/// ```rust
/// use chrono::Utc;
/// use rustymail::imap::types::{Email, Envelope, Address};
///
/// let email = Email {
///     uid: 42,
///     flags: vec!["\\Seen".to_string(), "\\Flagged".to_string()],
///     internal_date: Some(Utc::now()),
///     envelope: Some(Envelope {
///         subject: Some("Hello".to_string()),
///         from: vec![Address {
///             name: Some("Alice".to_string()),
///             mailbox: Some("alice".to_string()),
///             host: Some("example.com".to_string()),
///         }],
///         to: vec![],
///         cc: vec![],
///         bcc: vec![],
///         reply_to: vec![],
///         date: None,
///         in_reply_to: None,
///         message_id: None,
///     }),
///     body: Some(b"Hello, world!".to_vec()),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Email {
    /// Unique identifier for the email in the current folder
    pub uid: u32,
    /// List of IMAP flags associated with the email
    pub flags: Vec<String>,
    /// Internal date when the email was received by the server
    pub internal_date: Option<DateTime<Utc>>,
    /// Email envelope containing metadata like subject, sender, recipients
    pub envelope: Option<Envelope>,
    /// Raw email body content as bytes
    pub body: Option<Vec<u8>>,
}

/// Represents an IMAP folder (mailbox) in the email system.
///
/// A folder is a container for emails, organized hierarchically with a delimiter
/// character separating folder levels.
///
/// # Examples
///
/// ```rust
/// use rustymail::imap::types::Folder;
///
/// let folder = Folder {
///     name: "INBOX".to_string(),
///     delimiter: Some("/".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    /// Name of the folder
    pub name: String,
    /// Character used to separate folder levels in the hierarchy
    pub delimiter: Option<String>,
}

/// Represents information about a selected mailbox.
///
/// This struct contains metadata about a mailbox after it has been selected,
/// including its name, hierarchy delimiter, attributes, and message counts.
///
/// # Examples
///
/// ```rust
/// use rustymail::imap::types::MailboxInfo;
///
/// let mailbox = MailboxInfo {
///     name: "INBOX".to_string(),
///     delimiter: Some('/'),
///     exists: 42,
///     recent: 5,
///     unseen: Some(10),
///     selectable: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxInfo {
    /// Name of the mailbox
    pub name: String,
    /// Character used to separate mailbox levels in the hierarchy
    pub delimiter: String,
    /// Whether the mailbox can be selected
    pub selectable: bool,
    /// Total number of messages in the mailbox
    pub exists: u32,
    /// Number of messages with the \Recent flag
    pub recent: u32,
    /// Number of unseen messages
    pub unseen: Option<u32>,
}

impl From<AsyncImapName> for MailboxInfo {
    fn from(name: AsyncImapName) -> Self {
        Self {
            name: name.name().to_string(),
            delimiter: DEFAULT_MAILBOX_DELIMITER.to_string(), // async-imap Name doesn't have delimiter method
            selectable: true, // Assume selectable by default
            exists: 0,
            recent: 0,
            unseen: None,
        }
    }
}

impl From<AsyncImapMailbox> for MailboxInfo {
    fn from(_mailbox: AsyncImapMailbox) -> Self {
        Self {
            name: String::new(), // async-imap Mailbox doesn't have name method
            delimiter: DEFAULT_MAILBOX_DELIMITER.to_string(),
            selectable: true,
            exists: 0, // async-imap Mailbox doesn't have exists method
            recent: 0, // async-imap Mailbox doesn't have recent method
            unseen: None, // async-imap Mailbox doesn't have unseen method
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlagOperation {
    Add,
    Remove,
    Set,
}

/// Represents a list of flags for modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flags {
    #[serde(default)]
    pub items: Vec<String>,
}

/// Payload for modifying email flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyFlagsPayload {
    pub uids: Vec<u32>,
    pub operation: FlagOperation,
    pub flags: Flags,
}

/// Payload for appending an email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEmailPayload {
    pub content: String,
    pub flags: Flags,
}

/// Response after expunging a folder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExpungeResponse {
    pub message: String,
}

/// Represents the envelope of an email message.
///
/// The envelope contains metadata about an email message, including
/// its subject, sender, recipients, and various identifiers.
///
/// # Examples
///
/// ```rust
/// use rustymail::imap::types::{Envelope, Address};
///
/// let envelope = Envelope {
///     date: Some("2024-01-01T12:00:00Z".to_string()),
///     subject: Some("Hello".to_string()),
///     from: vec![Address {
///         name: Some("Alice".to_string()),
///         mailbox: Some("alice".to_string()),
///         host: Some("example.com".to_string()),
///     }],
///     to: vec![],
///     cc: vec![],
///     bcc: vec![],
///     reply_to: vec![],
///     in_reply_to: Some("<message-id@example.com>".to_string()),
///     message_id: Some("<unique-id@example.com>".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Envelope {
    /// Date when the message was sent
    pub date: Option<String>,
    /// Subject of the message
    pub subject: Option<String>,
    /// List of sender addresses
    pub from: Vec<Address>,
    /// List of primary recipient addresses
    pub to: Vec<Address>,
    /// List of carbon copy recipient addresses
    pub cc: Vec<Address>,
    /// List of blind carbon copy recipient addresses
    pub bcc: Vec<Address>,
    /// List of reply-to addresses
    pub reply_to: Vec<Address>,
    /// Message-ID of the message this one is in reply to
    pub in_reply_to: Option<String>,
    /// Unique message identifier
    pub message_id: Option<String>,
}

/// Represents an email address in the IMAP system.
///
/// This struct contains the components of an email address, including
/// the display name, mailbox, and host parts.
///
/// # Examples
///
/// ```rust
/// use rustymail::imap::types::Address;
///
/// let address = Address {
///     name: Some("Alice Smith".to_string()),
///     mailbox: Some("alice".to_string()),
///     host: Some("example.com".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address {
    /// Optional name associated with the address
    pub name: Option<String>,
    /// Optional mailbox part of the address
    pub mailbox: Option<String>,
    /// Optional host part of the address
    pub host: Option<String>,
}

impl Email {
    pub fn from_fetch(fetch: &Fetch) -> Result<Self, ImapError> {
        // Handle flags - fetch.flags() returns an iterator
        let flags: Vec<String> = fetch.flags()
            .map(|f| format!("{:?}", f))
            .collect();
        let envelope = fetch.envelope().map(|env| Envelope {
            date: env.date.as_ref().map(|d| String::from_utf8_lossy(d).to_string()),
            subject: env.subject.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            from: env.from.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            // Note: sender field exists in async-imap but not in our Envelope struct
            reply_to: env.reply_to.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            to: env.to.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            cc: env.cc.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            bcc: env.bcc.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            in_reply_to: env.in_reply_to.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            message_id: env.message_id.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
        });

        let internal_date = fetch.internal_date()
            .and_then(|d| DateTime::parse_from_rfc2822(&d.to_string()).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(Self {
            uid: fetch.uid.unwrap_or(0),
            flags,
            internal_date,
            envelope,
            body: None, // Body handling would go here
        })
    }

    fn convert_address(addr: &async_imap::imap_proto::Address) -> crate::imap::types::Address {
        crate::imap::types::Address {
            name: addr.name.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            // Note: async-imap Address has route field but our Address doesn't
            mailbox: addr.mailbox.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            host: addr.host.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
        }
    }
}

impl From<Fetch> for Email {
    fn from(fetch: Fetch) -> Self {
        let uid = fetch.uid.unwrap_or(0);
        // Handle flags - fetch.flags() returns an iterator
        let flags: Vec<String> = fetch.flags()
            .map(|f| format!("{:?}", f))
            .collect();
        
        let envelope = fetch.envelope().map(|env| Envelope {
            date: env.date.as_ref().map(|d| String::from_utf8_lossy(d).to_string()),
            subject: env.subject.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            from: env.from.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            // Note: sender field exists in async-imap but not in our Envelope struct
            reply_to: env.reply_to.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            to: env.to.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            cc: env.cc.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            bcc: env.bcc.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            in_reply_to: env.in_reply_to.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
            message_id: env.message_id.as_ref().map(|s| String::from_utf8_lossy(s).to_string()),
        });

        let internal_date = fetch.internal_date()
            .map(|d| d.with_timezone(&Utc));

        let body = fetch.body().map(|b| b.to_vec());

        Self {
            uid,
            flags,
            internal_date,
            envelope,
            body,
        }
    }
}