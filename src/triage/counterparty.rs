// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Who is behind a message, as a payables clerk would ask before trusting a
//! document: the sender, or for mail a platform delivers on someone's behalf
//! (document sharing, e-signature, event invitations, payment requests) the
//! person named in Reply-To. A platform message with nobody behind it has no
//! counterparty.

use super::message::TriageMessage;

/// Mailbox providers whose users are unrelated people: their history is kept
/// per address, never per domain.
const SHARED_MAILBOX_DOMAINS: &[&str] = &[
    "gmail.com",
    "googlemail.com",
    "outlook.com",
    "hotmail.com",
    "live.com",
    "msn.com",
    "yahoo.com",
    "icloud.com",
    "me.com",
    "mac.com",
    "aol.com",
    "proton.me",
    "protonmail.com",
    "gmx.com",
    "gmx.net",
    "mail.com",
    "yandex.com",
    "zoho.com",
    "fastmail.com",
    "nifty.com",
    // Apple's Hide My Email relay (privaterelay.appleid.com): every sender
    // gets its own relay address, so the domain says nothing about who it is.
    "appleid.com",
];

/// Second-level labels under which organisations register (example.co.uk).
const SECOND_LEVEL: &[&str] = &["co", "com", "org", "net", "ac", "gov", "edu"];

/// The organisation's domain: mail.mercury.com and mercury.com are one.
pub fn registrable_domain(domain: &str) -> String {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = domain.split('.').collect();
    let keep = if labels.len() > 2 && SECOND_LEVEL.contains(&labels[labels.len() - 2]) {
        3
    } else {
        2
    };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

/// What history is kept under for an address: the address itself at a shared
/// mailbox provider, the organisation's domain otherwise.
pub fn history_key(address: &str) -> Option<String> {
    let address = address.trim().to_ascii_lowercase();
    let (_, domain) = address.rsplit_once('@')?;
    let organisation = registrable_domain(domain);
    if organisation.is_empty() {
        None
    } else if SHARED_MAILBOX_DOMAINS.contains(&organisation.as_str()) {
        Some(address)
    } else {
        Some(organisation)
    }
}

/// The address of whoever is behind the message, or None for a platform
/// message that names nobody (`platforms` are delivery platforms' domains).
pub fn counterparty<'a>(message: &'a TriageMessage, platforms: &[String]) -> Option<&'a str> {
    let from_domain = message
        .from_address
        .rsplit_once('@')
        .map(|(_, d)| registrable_domain(d))
        .unwrap_or_default();
    if platforms
        .iter()
        .any(|p| registrable_domain(p) == from_domain)
    {
        let behind = message.reply_to_address.as_str();
        (!behind.is_empty() && behind != message.from_address).then_some(behind)
    } else {
        Some(message.from_address.as_str()).filter(|a| !a.is_empty())
    }
}
