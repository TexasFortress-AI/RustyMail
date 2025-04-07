use mail_parser::{MessageParser};
// Alias the Addr import
use mail_parser::Addr as MailAddr;
use crate::models::email::EmailDetail;
use crate::error::ImapApiError;

// This function might not be needed if get_email parses directly,
// but keep it for now adjusted for mail_parser
pub fn parse_email_body(raw_email: &[u8]) -> EmailDetail {
    let parser = MessageParser::new();
    let message = parser.parse(raw_email).expect("Failed to parse email");

    let subject = message.subject().map(|s| s.to_string()).unwrap_or_default();
    
    // Use format_address for single 'from' address
    let from = message.from()
        .and_then(|addrs| addrs.first())
        .map(format_address)
        .unwrap_or_default();
    
    // Use format_address for list of 'to' addresses
    let to = message.to()
        .map(|addrs| addrs.iter().map(format_address).collect())
        .unwrap_or_default();
    
    // Use format_address for list of 'cc' addresses
    let cc = message.cc()
        .map(|addrs| addrs.iter().map(format_address).collect())
        .unwrap_or_default();

    // Wrap bodies in Some()
    let text_body = message.text_bodies().next()
        .map(|part| String::from_utf8_lossy(part.contents()).into_owned());
    let html_body = message.html_bodies().next()
        .map(|part| String::from_utf8_lossy(part.contents()).into_owned());
        
    // Add the missing date field
    let date = message.date().map(|d| d.to_rfc3339()).unwrap_or_default();

    EmailDetail {
        subject,
        from,
        to,
        cc,
        text_body, // Now Option<String>
        html_body, // Now Option<String>
        date, // Added missing field
    }
}

// Potentially add helper to format mail_parser::Addr if needed elsewhere
// This seems redundant now that format_address exists
/*
pub fn format_mail_parser_address(addr_opt: Option<&Addr>) -> String {
     addr_opt.map(|addr| {
         match addr.name() {
             Some(name) => format!("\"{}\" <{}>", name, addr.address().unwrap_or("(no address)")),
             None => addr.address().unwrap_or("(no address)").to_string(),
         }
     }).unwrap_or_else(|| "(unknown sender)".to_string())
}
*/

// TODO: Implement subject decoding helper if needed (mail_parser might handle this)

pub fn parse_email(body: &[u8]) -> Result<EmailDetail, ImapApiError> {
    let parser = MessageParser::default();
    let message = parser.parse(body)
        .ok_or_else(|| ImapApiError::ParseError("Failed to parse email".to_string()))?;
    
    let from = message.from()
        .and_then(|addrs| addrs.first())
        .map(format_address)
        .unwrap_or_default();
    
    let to = message.to()
        .map(|addrs| addrs.iter().map(format_address).collect())
        .unwrap_or_default();
    
    let cc = message.cc()
        .map(|addrs| addrs.iter().map(format_address).collect())
        .unwrap_or_default();
    
    let subject = message.subject().unwrap_or_default().to_string();
    let text_body = message.text_bodies().next()
        .map(|part| String::from_utf8_lossy(part.contents()).into_owned()); // Keep as Option
    let html_body = message.html_bodies().next()
        .map(|part| String::from_utf8_lossy(part.contents()).into_owned()); // Keep as Option
    let date = message.date().map(|d| d.to_rfc3339()).unwrap_or_default();

    Ok(EmailDetail {
        from,
        to,
        cc,
        subject,
        text_body,
        html_body,
        date,
    })
}

// Simplify format_address to avoid problematic enum matching
fn format_address(addr: &MailAddr) -> String {
    // Prefer address if available (Mailbox)
    if let Some(address) = addr.address() {
        // Try to include name if available
        if let Some(name) = addr.name() {
            format!("{} <{}>", name, address)
        } else {
            address.to_string()
        }
    } else if let Some(name) = addr.name() { // Fallback to name (Group)
        name.to_string()
    } else {
        // Fallback if neither name nor address is available
        String::from("unformattable_address") 
    }
}
