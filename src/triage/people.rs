// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! People the account owner corresponds with: the account's own address and
//! everyone it has sent mail to, read from the cached Sent folders, and the
//! messages those sent mails replied to.
//!
//! A model must not file mail from these people as Spam or Noise; Needs Reply
//! is only for them, and only until the owner has replied.

use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Folders whose name contains this (case-insensitive) hold sent mail, e.g.
/// `Sent`, `INBOX.Sent`, `Sent Items`. The cache has no \Sent attribute.
const SENT_FOLDER_MARKER: &str = "sent";

#[derive(Debug, Default, Clone)]
pub struct KnownPeople {
    addresses: HashSet<String>,
    /// Message-IDs named by In-Reply-To or References in sent mail.
    replied_to: HashSet<String>,
}

impl KnownPeople {
    pub fn new<I, S>(addresses: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            addresses: addresses
                .into_iter()
                .map(|a| a.as_ref().trim().to_ascii_lowercase())
                .filter(|a| !a.is_empty())
                .collect(),
            replied_to: HashSet::new(),
        }
    }

    /// Record Message-IDs the owner has replied to.
    pub fn with_replies<I, S>(mut self, message_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.replied_to.extend(
            message_ids
                .into_iter()
                .filter_map(|id| normalize_id(id.as_ref())),
        );
        self
    }

    /// The account itself plus every To and Cc recipient in its Sent folders,
    /// and every message those sent mails reply to.
    pub async fn load(pool: &SqlitePool, account_id: &str) -> Result<Self, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT e.to_addresses, e.cc_addresses, e.in_reply_to, e.references_header
             FROM emails e
             JOIN folders f ON f.id = e.folder_id
             WHERE f.account_id = ? AND instr(lower(f.name), ?) > 0",
        )
        .bind(account_id)
        .bind(SENT_FOLDER_MARKER)
        .fetch_all(pool)
        .await?;
        let mut addresses = vec![account_id.to_string()];
        let mut replied = Vec::new();
        for row in rows {
            for column in ["to_addresses", "cc_addresses"] {
                let list: Option<String> = row.get(column);
                addresses.extend(parse_address_list(list.as_deref()));
            }
            for column in ["in_reply_to", "references_header"] {
                let ids: Option<String> = row.get(column);
                replied.extend(ids.unwrap_or_default().split_whitespace().map(String::from));
            }
        }
        Ok(Self::new(addresses).with_replies(replied))
    }

    /// Whether the owner has sent a reply to this message.
    pub fn replied_to(&self, message_id: Option<&str>) -> bool {
        message_id
            .and_then(normalize_id)
            .is_some_and(|id| self.replied_to.contains(&id))
    }

    pub fn contains(&self, address: &str) -> bool {
        self.addresses
            .contains(&address.trim().to_ascii_lowercase())
    }

    pub fn len(&self) -> usize {
        self.addresses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }
}

/// A Message-ID without its angle brackets; None when empty.
fn normalize_id(id: &str) -> Option<String> {
    let id = id
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// The cache stores recipients as a JSON array of addresses.
fn parse_address_list(list: Option<&str>) -> Vec<String> {
    list.and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .unwrap_or_default()
}
