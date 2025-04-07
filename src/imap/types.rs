use serde::{Deserialize, Serialize};
use imap_types::envelope::Envelope as ImapEnvelope;

// Custom Email struct (ensure it's public)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Email {
    pub uid: u32,
    pub flags: Vec<String>,
    pub size: Option<u32>,
    pub envelope: Option<ImapEnvelope<'static>>,
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
