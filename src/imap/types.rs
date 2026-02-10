// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

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
use log::debug;

use crate::imap::error::ImapError;
use crate::imap::session::DEFAULT_MAILBOX_DELIMITER;

/// Represents an email message in the IMAP system.
///
/// This struct encapsulates all the essential information about an email message,
/// including its unique identifier, flags, metadata, and MIME-parsed content.
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
///     mime_parts: vec![],
///     text_body: Some("Hello, world!".to_string()),
///     html_body: None,
///     attachments: vec![],
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
    /// Parsed MIME parts of the email
    pub mime_parts: Vec<MimePart>,
    /// Plain text body content (extracted from MIME parts)
    pub text_body: Option<String>,
    /// HTML body content (extracted from MIME parts)
    pub html_body: Option<String>,
    /// List of attachments (extracted from MIME parts)
    pub attachments: Vec<MimePart>,
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
/// let folder = Folder::new(
///     "INBOX".to_string(),
///     "INBOX".to_string(),
///     Some("/".to_string()),
/// );
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    /// Name of the folder
    pub name: String,
    /// Character used to separate folder levels in the hierarchy
    pub delimiter: Option<String>,
    /// Full path of the folder (e.g., "INBOX/Sent")
    pub full_path: String,
    /// Parent folder path (None for root folders)
    pub parent: Option<String>,
    /// Child folders (for hierarchical representation)
    pub children: Vec<Folder>,
    /// Whether this folder can be selected (contains messages)
    pub selectable: bool,
    /// Folder attributes from IMAP
    pub attributes: Vec<String>,
}

impl Folder {
    /// Creates a new folder with basic information
    pub fn new(name: String, full_path: String, delimiter: Option<String>) -> Self {
        Self {
            name,
            delimiter,
            full_path,
            parent: None,
            children: Vec::new(),
            selectable: true,
            attributes: Vec::new(),
        }
    }

    /// Creates a hierarchical folder tree from a flat list of folder paths
    pub fn build_hierarchy(folder_paths: Vec<(String, Option<String>, Vec<String>)>) -> Vec<Folder> {
                let mut folder_map = std::collections::HashMap::new();

        // First pass: create all folders
        for (full_path, delimiter, attributes) in folder_paths {
            let delim = delimiter.as_deref().unwrap_or("/");
            let parts: Vec<&str> = full_path.split(delim).collect();
            let name = parts.last().unwrap_or(&full_path.as_str()).to_string();
            let parent = if parts.len() > 1 {
                Some(parts[..parts.len()-1].join(delim))
            } else {
                None
            };

            let folder = Folder {
                name,
                delimiter,
                full_path: full_path.clone(),
                parent,
                children: Vec::new(),
                selectable: !attributes.contains(&"\\Noselect".to_string()),
                attributes,
            };
            folder_map.insert(full_path, folder);
        }

        // Second pass: build hierarchy by processing children first
        let mut root_folders = Vec::new();
        let mut sorted_names: Vec<String> = folder_map.keys().cloned().collect();

        // Sort by path depth (deeper paths first) to ensure children are processed before parents
        sorted_names.sort_by_key(|path| {
            let delim = folder_map.get(path).and_then(|f| f.delimiter.as_ref()).map(|s| s.as_str()).unwrap_or("/");
            std::cmp::Reverse(path.matches(delim).count())
        });

        // Build hierarchy by moving children into their parents
        for folder_name in sorted_names {
            if let Some(folder) = folder_map.remove(&folder_name) {
                if let Some(parent_path) = &folder.parent {
                    // Try to add to parent
                    if let Some(parent) = folder_map.get_mut(parent_path) {
                        parent.children.push(folder);
                        continue; // Successfully added to parent, don't add to roots
                    }
                    // Parent not found or already processed, will be a root
                }
                // No parent or parent not found - this is a root folder
                root_folders.push(folder);
            }
        }

        // Any remaining folders are also roots
        for (_, folder) in folder_map {
            root_folders.push(folder);
        }

        root_folders.sort_by(|a, b| a.name.cmp(&b.name));
        root_folders
    }

    /// Gets all folders in a flat list (depth-first traversal)
    pub fn flatten(&self) -> Vec<&Folder> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }
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
///     delimiter: "/".to_string(),
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
            SearchCriteria::Body(text) => write!(f, "BODY \"{}\"", Self::escape_search_text(text)),
            SearchCriteria::From(text) => write!(f, "FROM \"{}\"", Self::escape_search_text(text)),
            SearchCriteria::Subject(text) => write!(f, "SUBJECT \"{}\"", Self::escape_search_text(text)),
            SearchCriteria::Text(text) => write!(f, "TEXT \"{}\"", Self::escape_search_text(text)),
            SearchCriteria::To(text) => write!(f, "TO \"{}\"", Self::escape_search_text(text)),
            SearchCriteria::Uid(uids) => write!(f, "UID {}", uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",")),
            SearchCriteria::And(criteria) => write!(f, "({})", criteria.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")),
            SearchCriteria::Or(criteria) => write!(f, "(OR {})", criteria.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")),
            SearchCriteria::Not(criterion) => write!(f, "NOT {}", criterion),
        }
    }
}

impl SearchCriteria {
    /// Escapes text for IMAP search queries
    fn escape_search_text(text: &str) -> String {
        // Escape quotes and backslashes in search text
        text.replace('\\', "\\\\").replace('"', "\\\"")
    }

    /// Creates a compound AND search criteria
    pub fn and(criteria: Vec<SearchCriteria>) -> Self {
        SearchCriteria::And(criteria)
    }

    /// Creates a compound OR search criteria
    pub fn or(criteria: Vec<SearchCriteria>) -> Self {
        SearchCriteria::Or(criteria)
    }

    /// Creates a NOT search criteria
    pub fn not(criterion: SearchCriteria) -> Self {
        SearchCriteria::Not(Box::new(criterion))
    }

    /// Helper to create subject search
    pub fn subject<S: Into<String>>(text: S) -> Self {
        SearchCriteria::Subject(text.into())
    }

    /// Helper to create from search
    pub fn from<S: Into<String>>(text: S) -> Self {
        SearchCriteria::From(text.into())
    }

    /// Helper to create to search
    pub fn to<S: Into<String>>(text: S) -> Self {
        SearchCriteria::To(text.into())
    }

    /// Helper to create body search
    pub fn body<S: Into<String>>(text: S) -> Self {
        SearchCriteria::Body(text.into())
    }

    /// Helper to create text search (searches entire message)
    pub fn text<S: Into<String>>(text: S) -> Self {
        SearchCriteria::Text(text.into())
    }

    /// Helper to create date range search
    pub fn date_range(since: DateTime<Utc>, before: DateTime<Utc>) -> Self {
        SearchCriteria::And(vec![
            SearchCriteria::Since(since),
            SearchCriteria::Before(before),
        ])
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_search_criteria_display() {
        // Test simple criteria
        assert_eq!(SearchCriteria::All.to_string(), "ALL");
        assert_eq!(SearchCriteria::Unseen.to_string(), "UNSEEN");
        assert_eq!(SearchCriteria::Flagged.to_string(), "FLAGGED");

        // Test text criteria with proper quoting
        assert_eq!(SearchCriteria::Subject("test".to_string()).to_string(), "SUBJECT \"test\"");
        assert_eq!(SearchCriteria::From("user@example.com".to_string()).to_string(), "FROM \"user@example.com\"");
        assert_eq!(SearchCriteria::Body("hello world".to_string()).to_string(), "BODY \"hello world\"");

        // Test text escaping
        assert_eq!(SearchCriteria::Subject("test \"quoted\"".to_string()).to_string(), "SUBJECT \"test \\\"quoted\\\"\"");
    }

    #[test]
    fn test_search_criteria_compound() {
        let criteria = SearchCriteria::And(vec![
            SearchCriteria::From("sender@example.com".to_string()),
            SearchCriteria::Unseen,
        ]);
        assert_eq!(criteria.to_string(), "(FROM \"sender@example.com\" UNSEEN)");

        let or_criteria = SearchCriteria::Or(vec![
            SearchCriteria::Subject("urgent".to_string()),
            SearchCriteria::Subject("important".to_string()),
        ]);
        assert_eq!(or_criteria.to_string(), "(OR SUBJECT \"urgent\" SUBJECT \"important\")");
    }

    #[test]
    fn test_search_criteria_dates() {
        let date = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        assert_eq!(SearchCriteria::Since(date).to_string(), "SINCE 15-Jan-2024");
        assert_eq!(SearchCriteria::Before(date).to_string(), "BEFORE 15-Jan-2024");
    }

    #[test]
    fn test_search_criteria_helpers() {
        assert_eq!(SearchCriteria::subject("test"), SearchCriteria::Subject("test".to_string()));
        assert_eq!(SearchCriteria::from("user@example.com"), SearchCriteria::From("user@example.com".to_string()));

        let and_criteria = SearchCriteria::and(vec![
            SearchCriteria::Unseen,
            SearchCriteria::subject("important"),
        ]);
        assert!(matches!(and_criteria, SearchCriteria::And(_)));
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

/// Represents a MIME part within an email message.
///
/// This struct represents a single part of a MIME multipart message,
/// including its headers, content type, and body content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimePart {
    /// Content-Type header information
    pub content_type: ContentType,
    /// Content-Transfer-Encoding (e.g., "7bit", "8bit", "binary", "quoted-printable", "base64")
    pub content_transfer_encoding: Option<String>,
    /// Content-Disposition (e.g., "inline", "attachment")
    pub content_disposition: Option<ContentDisposition>,
    /// Content-ID for referencing this part
    pub content_id: Option<String>,
    /// Content-Description
    pub content_description: Option<String>,
    /// Raw headers for this part
    pub headers: HashMap<String, String>,
    /// Decoded body content as bytes
    pub body: Vec<u8>,
    /// Text content if this part is text-based
    pub text_content: Option<String>,
    /// Child parts for multipart content
    pub parts: Vec<MimePart>,
}

/// Represents a Content-Type header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentType {
    /// Main type (e.g., "text", "image", "application")
    pub main_type: String,
    /// Sub type (e.g., "plain", "html", "jpeg", "pdf")
    pub sub_type: String,
    /// Parameters (e.g., charset=utf-8, boundary=xyz)
    pub parameters: HashMap<String, String>,
}

impl ContentType {
    /// Returns the full content type as a string (e.g., "text/plain")
    pub fn mime_type(&self) -> String {
        format!("{}/{}", self.main_type, self.sub_type)
    }

    /// Checks if this is a text content type
    pub fn is_text(&self) -> bool {
        self.main_type == "text"
    }

    /// Checks if this is a multipart content type
    pub fn is_multipart(&self) -> bool {
        self.main_type == "multipart"
    }

    /// Gets the charset parameter, defaulting to "utf-8" for text types
    pub fn charset(&self) -> String {
        self.parameters.get("charset")
            .cloned()
            .unwrap_or_else(|| {
                if self.is_text() {
                    "utf-8".to_string()
                } else {
                    "us-ascii".to_string()
                }
            })
    }

    /// Gets the boundary parameter for multipart types
    pub fn boundary(&self) -> Option<&String> {
        self.parameters.get("boundary")
    }
}

/// Represents a Content-Disposition header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDisposition {
    /// Disposition type ("inline", "attachment", etc.)
    pub disposition_type: String,
    /// Parameters (e.g., filename="document.pdf")
    pub parameters: HashMap<String, String>,
}

impl ContentDisposition {
    /// Checks if this is an attachment
    pub fn is_attachment(&self) -> bool {
        self.disposition_type == "attachment"
    }

    /// Gets the filename parameter
    pub fn filename(&self) -> Option<&String> {
        self.parameters.get("filename")
    }
}

impl Email {
    /// Decode MIME RFC 2047 encoded text (e.g., "=?UTF-8?q?Subject_line?=")
    fn decode_mime_encoded_text(bytes: &[u8]) -> String {
        let raw = String::from_utf8_lossy(bytes);

        // Check if this looks like MIME encoded text
        if raw.contains("=?") && raw.contains("?=") {
            // Parse as a simple header to decode MIME encoded words
            if let Some(message) = mail_parser::Message::parse(format!("Subject: {}\r\n\r\n", raw).as_bytes()) {
                if let Some(subject) = message.subject() {
                    return subject.to_string();
                }
            }
        }

        // Fallback to raw string if not MIME encoded or decoding fails
        raw.to_string()
    }

    pub fn from_fetch(fetch: &Fetch) -> Result<Self, ImapError> {
        // Handle flags - fetch.flags() returns an iterator
        let flags: Vec<String> = fetch.flags()
            .map(|f| format!("{:?}", f))
            .collect();
        let envelope = fetch.envelope().map(|env| Envelope {
            date: env.date.as_ref().map(|d| Email::decode_mime_encoded_text(d)),
            subject: env.subject.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            from: env.from.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            // Note: sender field exists in async-imap but not in our Envelope struct
            reply_to: env.reply_to.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            to: env.to.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            cc: env.cc.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            bcc: env.bcc.as_ref().unwrap_or(&vec![]).iter().map(Self::convert_address).collect(),
            in_reply_to: env.in_reply_to.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            message_id: env.message_id.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
        });

        let internal_date = fetch.internal_date()
            .and_then(|d| DateTime::parse_from_rfc2822(&d.to_string()).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // Get raw body content
        let body = fetch.body().map(|b| b.to_vec());

        // Parse MIME content if body is available
        let (mime_parts, text_body, html_body, attachments) = if let Some(body_bytes) = &body {
            Self::parse_mime_content(body_bytes)?
        } else {
            (Vec::new(), None, None, Vec::new())
        };

        Ok(Self {
            uid: fetch.uid.unwrap_or(0),
            flags,
            internal_date,
            envelope,
            body,
            mime_parts,
            text_body,
            html_body,
            attachments,
        })
    }

    fn convert_address(addr: &async_imap::imap_proto::Address) -> crate::imap::types::Address {
        crate::imap::types::Address {
            name: addr.name.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            // Note: async-imap Address has route field but our Address doesn't
            mailbox: addr.mailbox.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            host: addr.host.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
        }
    }

    /// Parses MIME content from raw email body
    fn parse_mime_content(body_bytes: &[u8]) -> Result<(Vec<MimePart>, Option<String>, Option<String>, Vec<MimePart>), ImapError> {
        use mail_parser::Message;

        // Parse the email message
        let message = Message::parse(body_bytes)
            .ok_or_else(|| ImapError::Parse("Failed to parse email message".to_string()))?;

        let mut mime_parts = Vec::new();
        let text_body;
        let html_body;
        let mut attachments = Vec::new();

        // Extract text and HTML bodies directly from the message
        text_body = message.body_text(0).map(|s| s.to_string());
        html_body = message.body_html(0).map(|s| s.to_string());

        // DEBUG: Log part count and attachment count
        debug!("Email MIME parsing: {} total parts, {} attachments",
               message.parts.len(), message.attachment_count());

        // Process ALL parts, not just attachments
        for (i, part) in message.parts.iter().enumerate() {
            use mail_parser::MimeHeaders;

            debug!("  Part {}: content_type={:?}, attachment_name={:?}",
                   i,
                   part.content_type().map(|ct| format!("{}/{}", ct.c_type, ct.c_subtype.as_ref().unwrap_or(&"unknown".into()))),
                   part.attachment_name());

            // Check if this part should be treated as an attachment
            // Parts with filenames are attachments
            let is_attachment = part.attachment_name().is_some();

            if is_attachment && i > 0 {  // Skip part 0 which is usually the message itself
                debug!("    -> Treating as attachment");
                let mime_part = Self::create_attachment_mime_part(part);
                attachments.push(mime_part.clone());
                mime_parts.push(mime_part);
            }
        }

        debug!("Parsed {} attachments from email", attachments.len());

        Ok((mime_parts, text_body, html_body, attachments))
    }

    /// Create a MIME part from an attachment
    fn create_attachment_mime_part(attachment: &mail_parser::MessagePart) -> MimePart {
        use mail_parser::MimeHeaders;

        // Get content type information
        let content_type = if let Some(ct) = attachment.content_type() {
            ContentType {
                main_type: ct.c_type.to_string(),
                sub_type: ct.c_subtype.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "octet-stream".to_string()),
                parameters: HashMap::new(), // TODO: Extract parameters from mail_parser::ContentType
            }
        } else {
            ContentType {
                main_type: "application".to_string(),
                sub_type: "octet-stream".to_string(),
                parameters: HashMap::new(),
            }
        };

        // Get content disposition
        let content_disposition = attachment.attachment_name()
            .map(|name| ContentDisposition {
                disposition_type: "attachment".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert("filename".to_string(), name.to_string());
                    params
                },
            });

        // Get the body content
        let body = attachment.contents().to_vec();

        // Extract headers (simplified)
        let mut headers = HashMap::new();
        if let Some(name) = attachment.attachment_name() {
            headers.insert("Content-Disposition".to_string(),
                format!("attachment; filename=\"{}\"", name));
        }

        // Try to decode text content if it's a text type
        let text_content = if content_type.is_text() {
            String::from_utf8(body.clone()).ok()
        } else {
            None
        };

        MimePart {
            content_type,
            content_transfer_encoding: None, // Could be extracted from headers
            content_disposition,
            content_id: None, // Could be extracted from headers
            content_description: None, // Could be extracted from headers
            headers,
            body,
            text_content,
            parts: Vec::new(), // Attachments don't have nested parts in this context
        }
    }

    /// Parse content type from header string (simplified version)
    fn parse_content_type_from_header(header: &str) -> ContentType {
        let mut parts = header.split(';');
        let mime_type = parts.next().unwrap_or("application/octet-stream").trim();
        let mut type_parts = mime_type.split('/');
        let main_type = type_parts.next().unwrap_or("application").to_string();
        let sub_type = type_parts.next().unwrap_or("octet-stream").to_string();

        let mut parameters = HashMap::new();
        for param in parts {
            if let Some((key, value)) = param.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().trim_matches('"').to_string();
                parameters.insert(key, value);
            }
        }

        ContentType {
            main_type,
            sub_type,
            parameters,
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
            date: env.date.as_ref().map(|d| Email::decode_mime_encoded_text(d)),
            subject: env.subject.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            from: env.from.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            // Note: sender field exists in async-imap but not in our Envelope struct
            reply_to: env.reply_to.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            to: env.to.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            cc: env.cc.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            bcc: env.bcc.as_ref().unwrap_or(&vec![]).iter().map(Email::convert_address).collect(),
            in_reply_to: env.in_reply_to.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
            message_id: env.message_id.as_ref().map(|s| Email::decode_mime_encoded_text(s)),
        });

        let internal_date = fetch.internal_date()
            .map(|d| d.with_timezone(&Utc));

        let body = fetch.body().map(|b| b.to_vec());

        // Parse MIME content if body is available
        let (mime_parts, text_body, html_body, attachments) = if let Some(body_bytes) = &body {
            Email::parse_mime_content(body_bytes).unwrap_or_else(|_| {
                // If MIME parsing fails, return empty structures
                (Vec::new(), None, None, Vec::new())
            })
        } else {
            (Vec::new(), None, None, Vec::new())
        };

        Self {
            uid,
            flags,
            internal_date,
            envelope,
            body,
            mime_parts,
            text_body,
            html_body,
            attachments,
        }
    }
}