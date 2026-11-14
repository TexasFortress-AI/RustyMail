// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which IMAP folder each bucket moves to, per account.
//!
//! Servers differ: some keep folders under `INBOX.`, some at the top level,
//! and the trash may be `Trash`, `INBOX.Trash` or `Deleted Items`. Per-account
//! overrides come from a JSON file (TRIAGE_FOLDER_MAP_FILE):
//! `{"chris@example.test": {"Spam": "Junk Mail", "Trash": "Deleted Items"}}`.

use super::Bucket;
use std::collections::HashMap;
use std::path::Path;

/// Namespace used when the server keeps user folders under the INBOX.
const INBOX_NAMESPACE: &str = "INBOX.";
/// Trash folder names tried, in order, after the account's own namespace.
const TRASH_CANDIDATES: [&str; 4] = ["Trash", "Deleted Items", "Deleted Messages", "INBOX.Trash"];

/// Where a bucket's messages go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The folder exists on the server.
    Existing(String),
    /// The folder does not exist yet; apply creates it.
    Create(String),
    /// Nowhere safe to move to (no trash folder found).
    Unavailable(String),
}

#[derive(Debug, Default)]
pub struct FolderMap {
    overrides: HashMap<String, HashMap<Bucket, String>>,
}

impl FolderMap {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let raw: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(text).map_err(|e| format!("invalid folder map: {}", e))?;
        let mut overrides = HashMap::new();
        for (account, entries) in raw {
            let mut buckets = HashMap::new();
            for (bucket, folder) in entries {
                let bucket = Bucket::parse(&bucket)
                    .filter(|b| *b != Bucket::Keep)
                    .ok_or_else(|| format!("unknown bucket '{}' for {}", bucket, account))?;
                buckets.insert(bucket, folder);
            }
            overrides.insert(account.trim().to_ascii_lowercase(), buckets);
        }
        Ok(Self { overrides })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        Self::from_json(&text)
    }

    /// Destination for `bucket` given the folders the server lists. A trash
    /// folder is never created: deleting is not triage's job, so without an
    /// existing trash the move is refused.
    pub fn resolve(&self, account_id: &str, bucket: Bucket, existing: &[String]) -> Destination {
        let found = |name: &str| {
            existing
                .iter()
                .find(|f| f.eq_ignore_ascii_case(name))
                .cloned()
        };
        if let Some(name) = self
            .overrides
            .get(&account_id.to_ascii_lowercase())
            .and_then(|m| m.get(&bucket))
        {
            return match found(name) {
                Some(folder) => Destination::Existing(folder),
                None if bucket == Bucket::Trash => {
                    Destination::Unavailable(format!("trash folder '{}' does not exist", name))
                }
                None => Destination::Create(name.clone()),
            };
        }
        let prefix = namespace(existing);
        if bucket == Bucket::Trash {
            let own = format!("{}Trash", prefix);
            return std::iter::once(own.as_str())
                .chain(TRASH_CANDIDATES)
                .find_map(found)
                .map(Destination::Existing)
                .unwrap_or_else(|| {
                    Destination::Unavailable(
                        "no trash folder found; set one in TRIAGE_FOLDER_MAP_FILE".to_string(),
                    )
                });
        }
        let name = format!("{}{}", prefix, bucket.as_str());
        match found(&name) {
            Some(folder) => Destination::Existing(folder),
            None => Destination::Create(name),
        }
    }
}

/// `INBOX.` when the server keeps user folders under the INBOX.
pub fn namespace(existing: &[String]) -> &'static str {
    if existing.iter().any(|f| f.starts_with(INBOX_NAMESPACE)) {
        INBOX_NAMESPACE
    } else {
        ""
    }
}
