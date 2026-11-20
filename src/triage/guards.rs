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
        if addresses_the_sorter(message) {
            return Proposal {
                reason: format!(
                    "contains instructions aimed at the mail sorter; model said {} ({})",
                    proposal.bucket, proposal.reason
                ),
                bucket: Bucket::Keep,
                ..proposal
            };
        }
        // A document that would be trusted (a payment, an agreement, an alert)
        // from someone the owner has never dealt with waits for the owner,
        // as a payables clerk verifies a new vendor before paying.
        if message.new_counterparty
            && matches!(
                proposal.bucket,
                Bucket::Accounting | Bucket::Legal | Bucket::Credit | Bucket::Alerts
            )
        {
            return Proposal {
                reason: format!(
                    "new sender: verify (no approved mail or sent mail with them); model said {} ({})",
                    proposal.bucket, proposal.reason
                ),
                bucket: Bucket::Keep,
                ..proposal
            };
        }
        let overrule = match proposal.bucket {
            Bucket::Spam | Bucket::Noise if known.contains(&message.from_address) => {
                Some("sender is someone you correspond with".to_string())
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

    /// A message without a date counts as old: it cannot show it is recent.
    fn too_old(&self, message: &TriageMessage, now: DateTime<Utc>) -> bool {
        message
            .received_at
            .is_none_or(|received| now - received > self.needs_reply_max_age)
    }
}
