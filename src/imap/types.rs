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
