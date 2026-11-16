// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Email triage: propose a bucket for each INBOX message, and on request
//! apply the proposals by moving messages (never deleting them).
//!
//! Cheap header rules run first; only messages they cannot decide go to a
//! local model. Proposals are recorded in `triage_proposals`; nothing moves
//! until `rustymail-triage --apply`, which verifies every move.

pub mod apply;
pub mod classifier;
pub mod codes;
pub mod config;
pub mod decision;
pub mod folders;
pub mod guards;
pub mod message;
pub mod model;
pub mod people;
pub mod report;
pub mod rules;
pub mod runner;
pub mod scan;
pub mod store;

use chrono::{DateTime, Utc};
use std::fmt;

/// Where a message should go. `Keep` means it stays in the INBOX; `Trash`
/// is only for verification codes and password resets past their window.
/// `Accounting` (money records), `Legal` (agreements and terms) and `Credit`
/// (credit reports and monitoring) are the boxes later processed by the
/// accounting, legal and credit agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Spam,
    Noise,
    Accounting,
    Legal,
    Credit,
    Alerts,
    NeedsReply,
    Trash,
    Keep,
}

/// Bucket names from before Receipts became Accounting; stored proposals and
/// settings may still use them.
const LEGACY_NAMES: [(&str, Bucket); 1] = [("receipts", Bucket::Accounting)];

impl Bucket {
    pub const ALL: [Bucket; 9] = [
        Bucket::Spam,
        Bucket::Noise,
        Bucket::Accounting,
        Bucket::Legal,
        Bucket::Credit,
        Bucket::Alerts,
        Bucket::NeedsReply,
        Bucket::Trash,
        Bucket::Keep,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::Spam => "Spam",
            Bucket::Noise => "Noise",
            Bucket::Accounting => "Accounting",
            Bucket::Legal => "Legal",
            Bucket::Credit => "Credit",
            Bucket::Alerts => "Alerts",
            Bucket::NeedsReply => "Needs Reply",
            Bucket::Trash => "Trash",
            Bucket::Keep => "Keep",
        }
    }

    /// Case-insensitive; accepts "Needs Reply", "needs_reply" and "needsreply",
    /// and the legacy "Receipts" for Accounting.
    pub fn parse(value: &str) -> Option<Bucket> {
        let normalized: String = value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        let current = Bucket::ALL.into_iter().find(|bucket| {
            bucket
                .as_str()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .eq_ignore_ascii_case(&normalized)
        });
        current.or_else(|| {
            LEGACY_NAMES
                .iter()
                .find(|(name, _)| *name == normalized)
                .map(|(_, bucket)| *bucket)
        })
    }
}

impl fmt::Display for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What decided the bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalSource {
    Rule,
    Model,
}

impl ProposalSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ProposalSource::Rule => "rule",
            ProposalSource::Model => "model",
        }
    }
}

/// A proposed bucket for one message, with the evidence behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub bucket: Bucket,
    /// Model-reported confidence; `None` for rule matches.
    pub confidence: Option<f64>,
    pub source: ProposalSource,
    pub model_id: Option<String>,
    pub reason: String,
    /// Not to be applied before this time (an unexpired verification code).
    pub not_before: Option<DateTime<Utc>>,
}

impl Proposal {
    pub fn from_rule(bucket: Bucket, reason: impl Into<String>) -> Self {
        Self {
            bucket,
            confidence: None,
            source: ProposalSource::Rule,
            model_id: None,
            reason: reason.into(),
            not_before: None,
        }
    }
}
