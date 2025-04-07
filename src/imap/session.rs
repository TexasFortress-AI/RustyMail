use async_trait::async_trait;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::TryStreamExt;
use tokio_util::compat::Compat;
use tokio_rustls::client::TlsStream as TokioTlsStream;
use tokio::net::TcpStream as TokioTcpStream;

use async_imap::{
    Session as AsyncImapSession,
    types::{Fetch, Flag as AsyncImapFlag, Name},
};

use imap_types::{
    // command::Command,
    // command::CommandBody,
    // core::{Atom, Text},
    flag::Flag as ImapTypesFlag,
    // sequence::SequenceSet, // Unused
    // mailbox::Mailbox, // Unused
    core::Atom, // Import Atom
    // Import the ToStatic trait
    ToStatic,
};

use crate::imap::error::ImapError;
use crate::imap::types::{Email, OwnedMailbox, Folder, SearchCriteria, Envelope, Address};

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

// NEW Helper function to format Vec<u32> into IMAP sequence string
// Note: This is a basic implementation, assumes UIDs are mostly contiguous.
// A more robust version would handle complex grouping (1,3,5:7,9). 
fn format_uid_vec_to_imap_string(uids: &Vec<u32>) -> Result<String, ImapError> {
    if uids.is_empty() {
        return Err(ImapError::Internal("Cannot format empty UID vector".to_string()));
    }
    // Ensure sorted and unique for correct range detection
    let mut sorted_uids = uids.clone();
    sorted_uids.sort_unstable();
    sorted_uids.dedup();

    if sorted_uids.contains(&0) {
        return Err(ImapError::InvalidUid(0));
    }

    let mut parts = Vec::new();
    if sorted_uids.is_empty() {
        return Ok("".to_string()); // Should not happen due to check above, but handle defensively
    }

    let mut start = sorted_uids[0];
    let mut end = start;

    for &uid in sorted_uids.iter().skip(1) {
        if uid == end + 1 {
            end = uid;
        } else {
            if start == end {
                parts.push(start.to_string());
            } else {
                parts.push(format!("{}:{}", start, end));
            }
            start = uid;
            end = uid;
        }
    }
    // Add the last sequence part
    if start == end {
        parts.push(start.to_string());
    } else {
        parts.push(format!("{}:{}", start, end));
    }

    Ok(parts.join(","))
}

// Helper function to convert SearchCriteria into IMAP query string
fn convert_search_criteria(criteria: SearchCriteria) -> Result<String, ImapError> {
    Ok(match criteria {
        SearchCriteria::Subject(s) => format!("SUBJECT \"{}\"", s),
        SearchCriteria::From(s) => format!("FROM \"{}\"", s),
        SearchCriteria::To(s) => format!("TO \"{}\"", s),
        SearchCriteria::Body(s) => format!("BODY \"{}\"", s),
        SearchCriteria::Unseen => "UNSEEN".to_string(),
        SearchCriteria::All => "ALL".to_string(),
        SearchCriteria::Since(date_str) => {
            // Basic validation - expects "DD-Mon-YYYY"
            if chrono::NaiveDate::parse_from_str(&date_str, "%d-%b-%Y").is_err() {
                return Err(ImapError::Parse(format!("Invalid SINCE date format (expected DD-Mon-YYYY): {}", date_str)));
            }
            format!("SINCE {}", date_str)
        },
        SearchCriteria::Uid(uids) => {
            if uids.is_empty() {
                // Maybe return ALL or an error? Returning error for now.
                return Err(ImapError::Command("UID search requires at least one UID".to_string()));
            }
            let uid_str = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
            format!("UID {}", uid_str)
        },
        // Add arms for complex criteria
        SearchCriteria::And(_) => {
            return Err(ImapError::Command("Complex search criteria (And) not yet supported".to_string()))
        },
        SearchCriteria::Or(_) => {
            return Err(ImapError::Command("Complex search criteria (Or) not yet supported".to_string()))
        },
        SearchCriteria::Not(_) => {
            return Err(ImapError::Command("Complex search criteria (Not) not yet supported".to_string()))
        },
    })
}

// Helper to convert async_imap::types::Flag to our String representation
fn convert_async_flag(flag: &AsyncImapFlag) -> String {
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

// Helper to convert string flags for append into imap_types::Flag
// Ensure lifetime is 'static for the convenience method
fn convert_str_flag_to_imap_type(flag_str: &str) -> Result<ImapTypesFlag<'static>, ImapError> {
    Ok(match flag_str {
        // System flags - use lowercase associated function
        "\\Seen" | "Seen" => ImapTypesFlag::system(Atom::try_from("\\Seen").unwrap()), 
        "\\Answered" | "Answered" => ImapTypesFlag::system(Atom::try_from("\\Answered").unwrap()),
        "\\Flagged" | "Flagged" => ImapTypesFlag::system(Atom::try_from("\\Flagged").unwrap()),
        "\\Deleted" | "Deleted" => ImapTypesFlag::system(Atom::try_from("\\Deleted").unwrap()),
        "\\Draft" | "Draft" => ImapTypesFlag::system(Atom::try_from("\\Draft").unwrap()),
        "\\Recent" | "Recent" => ImapTypesFlag::system(Atom::try_from("\\Recent").unwrap()),
        // Custom flag (keyword)
        other => {
            if other.starts_with('\\') || other.is_empty() || other.contains('*') || other.contains('%') || other.contains('(') || other.contains(')') || other.contains('{') || other.contains('"') || other.contains('\\') {
                return Err(ImapError::Parse(format!("Invalid character in custom flag (keyword): {}", other)));
            }
            let atom = Atom::try_from(other.to_string()) // to_string creates owned String
                .map_err(|_| ImapError::Parse(format!("Could not create Atom for keyword: {}", other)))?;
            ImapTypesFlag::keyword(atom)
        }
    })
}

#[async_trait]
pub trait ImapSession: Send + Sync {
    async fn list_folders(&self) -> Result<Vec<Folder>, ImapError>;
    async fn create_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn delete_folder(&self, name: &str) -> Result<(), ImapError>;
    async fn rename_folder(&self, from: &str, to: &str) -> Result<(), ImapError>;
    async fn select_folder(&self, name: &str) -> Result<OwnedMailbox<'static>, ImapError>;
    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError>;
    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError>;
    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError>;
    // async fn append(&self, folder: &str, body: &[u8], flags: Option<Vec<&str>>) -> Result<(), ImapError>; // TODO: Fix append implementation
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
                attributes: name.attributes().iter().map(|a| match a {
                    // Use variants from async_imap::types::NameAttribute
                    async_imap::types::NameAttribute::NoSelect => "\\NoSelect".to_string(),
                    async_imap::types::NameAttribute::Marked => "\\Marked".to_string(),
                    async_imap::types::NameAttribute::Unmarked => "\\Unmarked".to_string(),
                    // Ignore other variants like NonExistent, Subscribed, Remote, etc.
                    _ => "".to_string(), 
                }).filter(|s| !s.is_empty()).collect(),
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

    async fn select_folder(&self, name: &str) -> Result<OwnedMailbox<'static>, ImapError> {
        let mut session_guard = self.session.lock().await;
        let _async_imap_mailbox = session_guard.select(name).await?;
        
        let target_mailbox = imap_types::mailbox::Mailbox::try_from(name)
             .map_err(|e| ImapError::Command(format!("Invalid mailbox name '{}': {}", name, e)))?;

        // Convert to static lifetime before returning (trait is now in scope)
        Ok(target_mailbox.to_static())
    }

    async fn search_emails(&self, criteria: SearchCriteria) -> Result<Vec<u32>, ImapError> {
        let query = convert_search_criteria(criteria)?;
        let mut session = self.session.lock().await;
        
        // async-imap search/uid_search returns HashSet<u32>
        let uids_set = session.uid_search(query).await?;
        // Convert HashSet to Vec for the return type
        let uids_vec: Vec<u32> = uids_set.into_iter().collect(); 
        Ok(uids_vec)
    }

    async fn fetch_emails(&self, uids: Vec<u32>) -> Result<Vec<Email>, ImapError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let mut session = self.session.lock().await;
        let query = "(FLAGS INTERNALDATE RFC822.SIZE ENVELOPE)";
        let seq_set_str = format_uid_vec_to_imap_string(&uids)?;
        let message_stream = session.uid_fetch(seq_set_str, query).await?;
        let messages: Vec<Fetch> = message_stream.try_collect().await?;

        Ok(messages.into_iter().map(|fetch| {
            // Initialize all fields of the Email struct
            let mut email = Email {
                uid: fetch.uid.unwrap_or(0),
                flags: Vec::new(),
                internal_date: None, // Placeholder
                size: fetch.size,
                envelope: None,
                body_structure: None, // Placeholder
                // Add None for the new fields from types.rs
                from: None,
                to: None,
                cc: None,
                bcc: None,
                sender: None,
                reply_to: None,
            };

            email.flags = fetch.flags().map(|f| convert_async_flag(&f)).collect();

            // Remove complex chrono conversion for now
            email.internal_date = None;
            // if let Some(imap_dt) = fetch.internal_date() { 
            //     let maybe_chrono_dt = ... ;
            //     if let Some(chrono_dt) = maybe_chrono_dt { ... } ...
            // }

            if let Some(envelope_ref) = fetch.envelope() {
                let date_str = envelope_ref.date.as_ref().map(|cow| String::from_utf8_lossy(cow.as_ref()).into_owned());
                let subject_str = envelope_ref.subject.as_ref().map(|cow| String::from_utf8_lossy(cow.as_ref()).into_owned());
                let in_reply_to_str = envelope_ref.in_reply_to.as_ref().map(|cow| String::from_utf8_lossy(cow.as_ref()).into_owned());
                let message_id_str = envelope_ref.message_id.as_ref().map(|cow| String::from_utf8_lossy(cow.as_ref()).into_owned());

                let convert_proto_addr_list = |proto_addrs: Option<&Vec<async_imap::imap_proto::Address>>| {
                    proto_addrs.map(|addrs| {
                        addrs.iter().map(convert_async_proto_address).collect()
                    })
                };

                email.envelope = Some(Envelope {
                    date: date_str,
                    subject: subject_str,
                    from: convert_proto_addr_list(envelope_ref.from.as_ref()),
                    sender: convert_proto_addr_list(envelope_ref.sender.as_ref()),
                    reply_to: convert_proto_addr_list(envelope_ref.reply_to.as_ref()),
                    to: convert_proto_addr_list(envelope_ref.to.as_ref()),
                    cc: convert_proto_addr_list(envelope_ref.cc.as_ref()),
                    bcc: convert_proto_addr_list(envelope_ref.bcc.as_ref()),
                    in_reply_to: in_reply_to_str,
                    message_id: message_id_str,
                });
            }

            email
        }).collect())
    }

    async fn move_email(&self, uids: Vec<u32>, destination_folder: &str) -> Result<(), ImapError> {
        if uids.is_empty() {
            return Ok(()); // Nothing to move
        }
        let mut session = self.session.lock().await;
        let seq_set_str = format_uid_vec_to_imap_string(&uids)?;
        session.uid_mv(seq_set_str, destination_folder).await?;
        Ok(())
    }

    /* // TODO: Fix append implementation (convenience method signature issue)
    async fn append(&self, folder: &str, body: &[u8], flags: Option<Vec<&str>>) -> Result<(), ImapError> {
        let mut session = self.session.lock().await;
        let maybe_imap_flags_vec: Option<Result<Vec<ImapTypesFlag<'static>>, ImapError>> = 
            flags.map(|fs| fs.into_iter().map(convert_str_flag_to_imap_type).collect());
        let imap_flags_vec: Option<Vec<ImapTypesFlag<'static>>> = match maybe_imap_flags_vec {
            Some(Ok(vec)) => Some(vec),
            Some(Err(e)) => return Err(e),
            None => None,
        };
        let imap_flags_slice: Option<&[ImapTypesFlag<'static>]> = imap_flags_vec.as_deref();
        let date: Option<ImapDateTime> = None; 
        session.append(folder, body, imap_flags_slice, date).await?;
        Ok(())
    }
    */

    // Consume Arc<Self> for logout to ensure session is dropped properly
    async fn logout(self: Arc<Self>) -> Result<(), ImapError> {
        let mut session_guard = self.session.lock().await;
        session_guard.logout().await?;
        Ok(())
    }
}

// NEW Helper to convert async_imap::imap_proto::Address to our Address type
fn convert_async_proto_address(proto_addr: &async_imap::imap_proto::Address) -> Address {
    let nstring_cow_to_opt_string = |opt_cow: &Option<Cow<[u8]>>| {
        opt_cow.as_ref().map(|cow| String::from_utf8_lossy(cow).into_owned())
    };

    Address {
        name: nstring_cow_to_opt_string(&proto_addr.name),
        adl: nstring_cow_to_opt_string(&proto_addr.adl),
        mailbox: nstring_cow_to_opt_string(&proto_addr.mailbox),
        host: nstring_cow_to_opt_string(&proto_addr.host),
    }
}

#[cfg(test)]
mod tests {
    // ... any test code ...
} 