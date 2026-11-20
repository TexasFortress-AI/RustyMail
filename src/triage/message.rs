// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The parts of a message that triage looks at, parsed from its raw bytes.

use chrono::{DateTime, Utc};
use mail_parser::{Address, MessageParser};

#[derive(Debug, Clone)]
pub struct TriageMessage {
    pub uid: u32,
    pub message_id: Option<String>,
    /// Lowercased sender address, empty if the From header has none.
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    /// Header names lowercased, in message order (topmost first).
    headers: Vec<(String, String)>,
    /// Plain-text body with whitespace collapsed, cut to the configured length.
    pub body_excerpt: String,
    /// When the message arrived: the Date header after parsing; the runner
    /// replaces it with the server's INTERNALDATE when there is one.
    pub received_at: Option<DateTime<Utc>>,
    /// The sender is someone the account owner has written to (from the Sent
    /// folders). Set by the runner; false until then.
    pub from_correspondent: bool,
    /// The owner has already replied (`\Answered`, or sent mail names it).
    /// Set by the runner; false until then.
    pub answered: bool,
}

impl TriageMessage {
    /// Parse a raw RFC 822 message. Returns None if it cannot be parsed.
    pub fn from_raw(uid: u32, raw: &[u8], excerpt_chars: usize) -> Option<Self> {
        let message = MessageParser::default().parse(raw)?;
        let (from_address, from_name) = first_sender(message.from());
        let headers = message
            .headers_raw()
            .map(|(name, value)| (name.to_ascii_lowercase(), unfold(value)))
            .collect();
        let body = message.body_text(0).unwrap_or_default();
        Some(Self {
            uid,
            message_id: message.message_id().map(str::to_string),
            from_address,
            from_name,
            subject: message.subject().unwrap_or_default().to_string(),
            headers,
            body_excerpt: excerpt(&body, excerpt_chars),
            received_at: message
                .date()
                .and_then(|date| DateTime::from_timestamp(date.to_timestamp(), 0)),
            from_correspondent: false,
            answered: false,
        })
    }

    /// The topmost header with this name (case-insensitive). For trace headers
    /// such as Authentication-Results, that is the one our own server added.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn has_header(&self, name: &str) -> bool {
        self.header(name).is_some()
    }

    /// Domain part of the sender address, lowercased.
    pub fn from_domain(&self) -> Option<&str> {
        self.from_address.rsplit_once('@').map(|(_, domain)| domain)
    }
}

fn first_sender(value: Option<&Address<'_>>) -> (String, Option<String>) {
    match value.and_then(Address::first) {
        Some(addr) => (
            addr.address
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            addr.name.as_ref().map(|n| n.to_string()),
        ),
        None => (String::new(), None),
    }
}

/// Join folded header lines into one line.
fn unfold(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(max_chars).collect()
}
