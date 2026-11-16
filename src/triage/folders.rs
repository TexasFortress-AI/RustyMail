// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which IMAP folder each bucket moves to, per account.
//!
//! Servers differ: some keep folders under `INBOX.`, some at the top level,
//! and the trash may be `Trash`, `INBOX.Trash` or `Deleted Items`. The folder
//! the server marks `\Trash` or `\Junk` wins over names, which matters on
//! accounts that kept an old server's folders. Per-account overrides, which
//! win over both, come from a JSON file (TRIAGE_FOLDER_MAP_FILE):
//! `{"chris@example.test": {"Spam": "Junk Mail", "Trash": "Deleted Items"}}`.

use super::Bucket;
use crate::imap::special_folders::SpecialUse;
use std::collections::HashMap;
use std::path::Path;

/// Prefix of bucket folders made on `/`-delimited servers before the
/// namespace came from the folder tree.
const LEGACY_PREFIX: &str = "INBOX.";
/// The Accounting bucket's name before Receipts became Accounting.
const LEGACY_RECEIPTS: &str = "Receipts";
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

    /// Destination for `bucket` given the folders the server lists, the
    /// folder it marks for this bucket's special use (see `special_use`) and
    /// the account's namespace (see `inbox_namespace`). A trash folder is
    /// never created: deleting is not triage's job, so without an existing
    /// trash the move is refused.
    pub fn resolve(
        &self,
        account_id: &str,
        bucket: Bucket,
        existing: &[String],
        marked: Option<&str>,
        namespace: &str,
    ) -> Destination {
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
        if let Some(name) = marked {
            return Destination::Existing(name.to_string());
        }
        if bucket == Bucket::Trash {
            let own = format!("{}Trash", namespace);
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
        let name = format!("{}{}", namespace, bucket.as_str());
        match found(&name) {
            Some(folder) => Destination::Existing(folder),
            None => Destination::Create(name),
        }
    }
}

/// Prefix for bucket folders. The server's INBOX namespace (`INBOX.`,
/// `INBOX/`) when it keeps folders under the INBOX; otherwise `INBOX.` when
/// this account's bucket folders already carry it (servers with a `/`
/// delimiter list them as top-level folders named "INBOX.Noise"), so new
/// ones sit beside them; otherwise none. Only bucket folders count: a stray
/// "INBOX.Outbox" label on Gmail does not.
pub fn bucket_namespace(existing: &[String], inbox_namespace: &str) -> String {
    if !inbox_namespace.is_empty() {
        return inbox_namespace.to_string();
    }
    let prefixed = |name: &str| {
        let folder = format!("{}{}", LEGACY_PREFIX, name);
        existing.iter().any(|f| f.eq_ignore_ascii_case(&folder))
    };
    let made_by_triage = Bucket::ALL
        .iter()
        .filter(|b| !matches!(b, Bucket::Trash | Bucket::Keep))
        .any(|b| prefixed(b.as_str()))
        || prefixed(LEGACY_RECEIPTS);
    if made_by_triage {
        LEGACY_PREFIX.to_string()
    } else {
        String::new()
    }
}

/// The special use a bucket's folder has on servers that mark one.
pub fn special_use(bucket: Bucket) -> Option<SpecialUse> {
    match bucket {
        Bucket::Trash => Some(SpecialUse::Trash),
        Bucket::Spam => Some(SpecialUse::Junk),
        _ => None,
    }
}
