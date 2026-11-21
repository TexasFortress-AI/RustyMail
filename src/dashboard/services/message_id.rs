// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The Message-ID of mail RustyMail sends (RFC 5322 section 3.6.4).
//!
//! lettre adds a Message-ID only when asked, and its default names the
//! machine's hostname, so every message would reveal the computer it was sent
//! from. Replies thread on this ID, the Outbox copy is found and removed by it,
//! and moves are verified with it, so every sent message needs one.

/// Used only if the sender's address has no domain, which a parsed sender
/// mailbox always has.
const FALLBACK_DOMAIN: &str = "localhost";

/// A new, globally unique Message-ID on the sender's own domain,
/// e.g. `<3f2a...@example.com>`.
pub fn new_message_id(sender_address: &str) -> String {
    let domain = sender_address
        .rsplit_once('@')
        .map(|(_, domain)| domain.trim())
        .filter(|domain| !domain.is_empty())
        .unwrap_or(FALLBACK_DOMAIN);
    format!("<{}@{}>", uuid::Uuid::new_v4().simple(), domain)
}
