// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! People the account owner corresponds with: the account's own address and
//! everyone it has sent mail to, read from the cached Sent folders, and the
//! messages those sent mails replied to.
//!
//! A model must not file mail from these people as Spam or Noise; Needs Reply
//! is only for them, and only until the owner has replied.

use super::counterparty::history_key;
use super::message::{thread_root, TriageMessage};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};

/// Folders whose name contains this (case-insensitive) hold sent mail, e.g.
/// `Sent`, `INBOX.Sent`, `Sent Items`. The cache has no \Sent attribute.
const SENT_FOLDER_MARKER: &str = "sent";

#[derive(Debug, Default, Clone)]
pub struct KnownPeople {
    addresses: HashSet<String>,
    /// Message-IDs named by In-Reply-To or References in sent mail.
    replied_to: HashSet<String>,
    /// When the owner last wrote in each conversation, by its root ID.
    last_sent_in_thread: HashMap<String, DateTime<Utc>>,
    /// Mail the owner approved into Accounting, Legal, Credit or Alerts, by
    /// sender (see `counterparty::history_key`): his payables history.
    approved: HashMap<String, usize>,
    /// Addresses whose mail the owner corrected to Needs Reply or Keep: the
    /// owner wants their mail, so a model may not call it Spam again.
    kept: HashSet<String>,
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
            last_sent_in_thread: HashMap::new(),
            approved: HashMap::new(),
            kept: HashSet::new(),
        }
    }

    /// Record senders whose mail the owner approved into a trusted bucket.
    pub fn with_approved<I, S>(mut self, senders: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for sender in senders {
            if let Some(key) = history_key(sender.as_ref()) {
                *self.approved.entry(key).or_default() += 1;
            }
        }
        self
    }

    /// Record addresses whose mail the owner kept (corrected to Needs Reply
    /// or Keep).
    pub fn with_kept<I, S>(mut self, senders: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.kept.extend(
            senders
                .into_iter()
                .map(|a| a.as_ref().trim().to_ascii_lowercase())
                .filter(|a| !a.is_empty()),
        );
        self
    }

    /// Whether the owner kept this sender's mail before (per address).
    pub fn kept(&self, address: &str) -> bool {
        self.kept.contains(&address.trim().to_ascii_lowercase())
    }

    /// How many approved messages the owner has from this sender's
    /// organisation (or from this address, at a shared mailbox provider).
    pub fn approved_count(&self, address: &str) -> usize {
        history_key(address)
            .and_then(|key| self.approved.get(&key).copied())
            .unwrap_or(0)
    }

    /// Whether the owner deals with this sender: has written to them or to
    /// their organisation, or has approved their documents before.
    pub fn deals_with(&self, address: &str) -> bool {
        let Some(key) = history_key(address) else {
            return false;
        };
        self.contains(address)
            || self.approved.contains_key(&key)
            || self
                .addresses
                .iter()
                .any(|a| history_key(a).as_deref() == Some(&key))
    }

    /// Record that the owner wrote in a conversation at a given time.
    pub fn with_sent_in_thread(mut self, root: &str, sent_at: DateTime<Utc>) -> Self {
        let latest = self
            .last_sent_in_thread
            .entry(root.to_string())
            .or_insert(sent_at);
        *latest = (*latest).max(sent_at);
        self
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
    /// Everyone written to from the Sent folders of `accounts` (all the
    /// owner's accounts), so a person written to from one account is a
    /// correspondent on all of them.
    pub async fn load(pool: &SqlitePool, accounts: &[String]) -> Result<Self, sqlx::Error> {
        let mut rows = Vec::new();
        let mut approved = Vec::new();
        let mut kept = Vec::new();
        for account_id in accounts {
            rows.extend(Self::sent_rows(pool, account_id).await?);
            approved.extend(Self::approved_senders(pool, account_id).await?);
            kept.extend(Self::kept_senders(pool, account_id).await?);
        }
        Ok(Self::from_sent(accounts, rows)
            .with_approved(approved)
            .with_kept(kept))
    }

    /// Who was behind mail moved into Accounting, Legal, Credit or Alerts, as
    /// proposed or as the owner corrected it: the recorded counterparty (the
    /// person behind a platform message), else the sender.
    async fn approved_senders(
        pool: &SqlitePool,
        account_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT coalesce(counterparty, from_address) AS from_address FROM triage_proposals
             WHERE account_id = ? AND applied_at IS NOT NULL
               AND bucket IN ('Accounting', 'Legal', 'Credit', 'Alerts')",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("from_address"))
            .collect())
    }

    /// Senders of mail the owner corrected to Needs Reply or Keep.
    async fn kept_senders(pool: &SqlitePool, account_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT from_address FROM triage_proposals
             WHERE account_id = ? AND correction IN ('Needs Reply', 'Keep')",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("from_address"))
            .collect())
    }

    async fn sent_rows(
        pool: &SqlitePool,
        account_id: &str,
    ) -> Result<Vec<sqlx::sqlite::SqliteRow>, sqlx::Error> {
        sqlx::query(
            "SELECT e.to_addresses, e.cc_addresses, e.in_reply_to, e.references_header,
                    e.message_id, e.date
             FROM emails e
             JOIN folders f ON f.id = e.folder_id
             WHERE f.account_id = ? AND instr(lower(f.name), ?) > 0",
        )
        .bind(account_id)
        .bind(SENT_FOLDER_MARKER)
        .fetch_all(pool)
        .await
    }

    fn from_sent(accounts: &[String], rows: Vec<sqlx::sqlite::SqliteRow>) -> Self {
        let mut addresses = accounts.to_vec();
        let mut replied = Vec::new();
        let mut threads = Vec::new();
        for row in rows {
            let root = thread_root(
                row.get::<Option<String>, _>("references_header").as_deref(),
                row.get::<Option<String>, _>("in_reply_to").as_deref(),
                row.get::<Option<String>, _>("message_id").as_deref(),
            );
            let sent_at = row
                .get::<Option<String>, _>("date")
                .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc));
            if let (Some(root), Some(sent_at)) = (root, sent_at) {
                threads.push((root, sent_at));
            }
            for column in ["to_addresses", "cc_addresses"] {
                let list: Option<String> = row.get(column);
                addresses.extend(parse_address_list(list.as_deref()));
            }
            for column in ["in_reply_to", "references_header"] {
                let ids: Option<String> = row.get(column);
                replied.extend(ids.unwrap_or_default().split_whitespace().map(String::from));
            }
        }
        let known = Self::new(addresses).with_replies(replied);
        threads.into_iter().fold(known, |known, (root, at)| {
            known.with_sent_in_thread(&root, at)
        })
    }

    /// The owner has answered this message: replied to it directly, or
    /// written in its conversation after it arrived.
    pub fn answered(&self, message: &TriageMessage) -> bool {
        if self.replied_to(message.message_id.as_deref()) {
            return true;
        }
        let latest = message
            .thread_root()
            .and_then(|root| self.last_sent_in_thread.get(&root).copied());
        matches!((latest, message.received_at), (Some(sent), Some(arrived)) if sent > arrived)
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

/// The owner's accounts whose Sent folders define correspondents: the
/// configured triage accounts, plus `account_id` if a run names another.
pub fn owner_accounts(configured: &[String], account_id: &str) -> Vec<String> {
    let mut accounts = configured.to_vec();
    if !accounts.iter().any(|a| a.eq_ignore_ascii_case(account_id)) {
        accounts.push(account_id.to_string());
    }
    accounts
}
