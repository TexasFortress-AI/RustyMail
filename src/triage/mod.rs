// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Email triage, phase 1: propose a bucket for each new INBOX message.
//!
//! Cheap header rules run first; only messages they cannot decide go to a
//! local model. Proposals are recorded in `triage_proposals`. Nothing is moved.

pub mod store;

use std::fmt;

/// Where a message should go. `Keep` means it stays in the INBOX.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Spam,
    Noise,
    Receipts,
    Alerts,
    NeedsReply,
    Keep,
}

impl Bucket {
    pub const ALL: [Bucket; 6] = [
        Bucket::Spam,
        Bucket::Noise,
        Bucket::Receipts,
        Bucket::Alerts,
        Bucket::NeedsReply,
        Bucket::Keep,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::Spam => "Spam",
            Bucket::Noise => "Noise",
            Bucket::Receipts => "Receipts",
            Bucket::Alerts => "Alerts",
            Bucket::NeedsReply => "Needs Reply",
            Bucket::Keep => "Keep",
        }
    }

    /// Case-insensitive; accepts "Needs Reply", "needs_reply" and "needsreply".
    pub fn parse(value: &str) -> Option<Bucket> {
        let normalized: String = value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        Bucket::ALL.into_iter().find(|bucket| {
            bucket
                .as_str()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .eq_ignore_ascii_case(&normalized)
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
}

impl Proposal {
    pub fn from_rule(bucket: Bucket, reason: impl Into<String>) -> Self {
        Self {
            bucket,
            confidence: None,
            source: ProposalSource::Rule,
            model_id: None,
            reason: reason.into(),
        }
    }
}
