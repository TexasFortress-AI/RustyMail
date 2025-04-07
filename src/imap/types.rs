use serde::{Deserialize, Serialize};

// Re-export necessary types from imap-types for external use
pub use imap_types::mailbox::Mailbox as OwnedMailbox;
// pub use imap_types::fetch::MessageDataItem; // Keep commented or remove if not used
pub use imap_types::flag::Flag;

// Custom Email struct (ensure it's public)
#[derive(Debug, Clone, Serialize, Deserialize)] // Removed Default here as Envelope might not be Default
pub struct Email {
    pub uid: u32,
    // Use the Address struct defined below
    pub from: Option<Vec<Address>>,
    pub to: Option<Vec<Address>>,
    pub cc: Option<Vec<Address>>,
    pub bcc: Option<Vec<Address>>,
    pub sender: Option<Vec<Address>>,
    pub reply_to: Option<Vec<Address>>,
    // Keep other fields from previous definition
    pub internal_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub flags: Vec<String>,
    pub size: Option<u32>,
    pub envelope: Option<Envelope>, // Use Envelope struct defined below
    pub body_structure: Option<String>, // Assuming String representation for now
}

// Custom Folder struct (ensure it's public)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)] // Added Serialize/Deserialize
pub struct Folder {
    pub name: String,
    pub attributes: Vec<String>,
    pub delimiter: Option<String>,
}

// Custom SearchCriteria enum (ensure it's public)
#[derive(Debug, Clone, Serialize, Deserialize)] // Added Serialize/Deserialize
pub enum SearchCriteria {
    Subject(String),
    From(String),
    To(String),
    Body(String),
    Unseen,
    All,
    Since(String), // Expects "DD-Mon-YYYY"
    Uid(Vec<u32>),
    And(Vec<SearchCriteria>),
    Or(Vec<SearchCriteria>),
    Not(Box<SearchCriteria>),
}

// Remove the conflicting struct definition for OwnedMailbox
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
// pub struct OwnedMailbox { ... fields ... }

// Define Address struct used in Email and Envelope (make public)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Address {
    pub name: Option<String>,
    pub adl: Option<String>,
    pub mailbox: Option<String>,
    pub host: Option<String>,
}

// Define Envelope struct used in Email (make public)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Envelope {
    pub date: Option<String>, // Consider parsing to DateTime later
    pub subject: Option<String>,
    pub from: Option<Vec<Address>>,
    pub sender: Option<Vec<Address>>,
    pub reply_to: Option<Vec<Address>>,
    pub to: Option<Vec<Address>>,
    pub cc: Option<Vec<Address>>,
    pub bcc: Option<Vec<Address>>,
    pub in_reply_to: Option<String>,
    pub message_id: Option<String>,
}
