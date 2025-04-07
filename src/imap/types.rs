use serde::{Deserialize, Serialize};
// Remove unused import
// use imap_types::envelope::Envelope;
use imap_types::core::NString;

// Custom Email struct (ensure it's public)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Email {
    pub uid: u32,
    pub flags: Vec<String>,
    pub size: Option<u32>,
    pub envelope: Option<ImapEnvelope>,
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
    /// Flags defined for this mailbox.
    pub flags: Vec<String>,
    /// Number of messages in the mailbox.
    pub exists: u32,
    /// Number of recent messages.
    pub recent: u32,
    /// Number of unseen messages (if available).
    pub unseen: Option<u32>,
    /// Permanent flags that can be set.
    pub permanent_flags: Vec<String>,
    /// Predicted next UID.
    pub uid_next: Option<u32>,
    /// UID validity value.
    pub uid_validity: Option<u32>,
    // highest_modseq is often not needed for clients, omitting for simplicity
}

// Custom SearchCriteria enum (ensure it's public)
#[derive(Debug, Clone, Deserialize)]
pub enum SearchCriteria {
    All,
    Subject(String),
    From(String),
    To(String),
    Body(String),
    Since(String), // Keep as string, parse later if needed
    Uid(String), // Comma-separated UIDs as string
    Unseen,
    And(Vec<SearchCriteria>),
    Or(Vec<SearchCriteria>),
    Not(Box<SearchCriteria>),
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
