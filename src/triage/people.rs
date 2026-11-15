// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! People the account owner corresponds with: the account's own address and
//! everyone it has sent mail to, read from the cached Sent folders.
//!
//! A model must not file mail from these people as Spam or Noise.

use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Folders whose name contains this (case-insensitive) hold sent mail, e.g.
/// `Sent`, `INBOX.Sent`, `Sent Items`. The cache has no \Sent attribute.
const SENT_FOLDER_MARKER: &str = "sent";

#[derive(Debug, Default, Clone)]
pub struct KnownPeople {
    addresses: HashSet<String>,
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
        }
    }

    /// The account itself plus every To and Cc recipient in its Sent folders.
    pub async fn load(pool: &SqlitePool, account_id: &str) -> Result<Self, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT e.to_addresses, e.cc_addresses FROM emails e
             JOIN folders f ON f.id = e.folder_id
             WHERE f.account_id = ? AND instr(lower(f.name), ?) > 0",
        )
        .bind(account_id)
        .bind(SENT_FOLDER_MARKER)
        .fetch_all(pool)
        .await?;
        let mut addresses = vec![account_id.to_string()];
        for row in rows {
            for column in ["to_addresses", "cc_addresses"] {
                let list: Option<String> = row.get(column);
                addresses.extend(parse_address_list(list.as_deref()));
            }
        }
        Ok(Self::new(addresses))
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

/// The cache stores recipients as a JSON array of addresses.
fn parse_address_list(list: Option<&str>) -> Vec<String> {
    list.and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .unwrap_or_default()
}
