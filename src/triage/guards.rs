// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Checks applied to a model's proposal before it is recorded. Rules are
//! trusted as they are; a model's bucket is overruled when facts the model
//! cannot see say otherwise. An overruled message stays in the INBOX.

use super::injection::addresses_the_sorter;
use super::message::TriageMessage;
use super::people::KnownPeople;
use super::{Bucket, Proposal, ProposalSource};
use chrono::{DateTime, Duration, Utc};

/// Why a model's verdict waits for the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// The message talks to the mail sorter ("note to the AI: file this as...").
    SorterInstructions,
    /// A trusted document from someone the owner has never dealt with.
    NewSender,
}

impl Hold {
    pub fn reason(self) -> &'static str {
        match self {
            Hold::SorterInstructions => "contains instructions aimed at the mail sorter",
            Hold::NewSender => "new sender: verify (no approved mail or sent mail with them)",
        }
    }
}

pub struct ModelGuards {
    /// Needs Reply only for mail newer than this; older threads stay put.
    pub needs_reply_max_age: Duration,
}

impl ModelGuards {
    pub fn check(
        &self,
        proposal: Proposal,
        message: &TriageMessage,
        known: &KnownPeople,
        now: DateTime<Utc>,
    ) -> Proposal {
        if proposal.source != ProposalSource::Model {
            return proposal;
        }
        if let Some(hold) = self.hold(&proposal, message) {
            return Proposal {
                reason: format!(
                    "{}; model said {} ({})",
                    hold.reason(),
                    proposal.bucket,
                    proposal.reason
                ),
                bucket: Bucket::Keep,
                ..proposal
            };
        }
        let overrule = match proposal.bucket {
            Bucket::Spam | Bucket::Noise if known.contains(&message.from_address) => {
                Some("sender is someone you correspond with".to_string())
            }
            // The owner rescued this sender's mail before; Noise still applies
            // (a company's newsletter after one important notice).
            Bucket::Spam if known.kept(&message.from_address) => {
                Some("you kept this sender's mail before".to_string())
            }
            Bucket::NeedsReply if message.answered => Some("you have already replied".to_string()),
            Bucket::NeedsReply if self.too_old(message, now) => Some(format!(
                "older than {} days",
                self.needs_reply_max_age.num_days()
            )),
            _ => None,
        };
        match (overrule, proposal.document) {
            // Not left for a reply: filed as the document it is.
            (Some(why), Some((document, confidence))) if proposal.bucket == Bucket::NeedsReply => {
                Proposal {
                    reason: format!("{}; filed as {} ({})", why, document, proposal.reason),
                    bucket: document,
                    confidence: Some(confidence),
                    ..proposal
                }
            }
            (Some(why), _) => Proposal {
                reason: format!(
                    "{}; model said {} ({})",
                    why, proposal.bucket, proposal.reason
                ),
                bucket: Bucket::Keep,
                ..proposal
            },
            (None, _) => proposal,
        }
    }

    /// Whether a model's verdict must wait for the owner instead of being
    /// acted on. In the INBOX a held message stays there; in the spam folder it
    /// stays there too, so phishing the server caught is never rescued.
    pub fn hold(&self, proposal: &Proposal, message: &TriageMessage) -> Option<Hold> {
        if proposal.source != ProposalSource::Model {
            return None;
        }
        if addresses_the_sorter(message) {
            return Some(Hold::SorterInstructions);
        }
        // A document that would be trusted (a payment, an agreement, an alert)
        // from someone the owner has never dealt with waits for the owner,
        // as a payables clerk verifies a new vendor before paying.
        let trusted_document = matches!(
            proposal.bucket,
            Bucket::Accounting | Bucket::Legal | Bucket::Credit | Bucket::Alerts
        );
        (message.new_counterparty && trusted_document).then_some(Hold::NewSender)
    }

    /// A message without a date counts as old: it cannot show it is recent.
    fn too_old(&self, message: &TriageMessage, now: DateTime<Utc>) -> bool {
        message
            .received_at
            .is_none_or(|received| now - received > self.needs_reply_max_age)
    }
}
