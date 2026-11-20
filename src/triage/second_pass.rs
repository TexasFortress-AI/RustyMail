// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A second opinion for mail the first model is unsure about. A model
//! proposal below the bar a move needs goes to a larger local model, which
//! reads a longer excerpt of the body; its answer replaces the first one.
//! If the larger model fails, the first answer stands and the failure is
//! logged: a missing or misnamed model is a configuration error to fix, not
//! something to retry.

use super::message::TriageMessage;
use super::model::{describe, parse_verdict, ModelBackend, SYSTEM_PROMPT};
use super::{Proposal, ProposalSource};
use log::warn;

pub struct SecondPass<B: ModelBackend> {
    backend: B,
    model: String,
    /// Model proposals below this confidence get a second pass.
    below: f64,
}

impl<B: ModelBackend> SecondPass<B> {
    pub fn new(backend: B, model: String, below: f64) -> Self {
        Self {
            backend,
            model,
            below,
        }
    }

    /// Whether `first` is a model answer too unsure to act on.
    pub fn wanted(&self, first: &Proposal) -> bool {
        first.source == ProposalSource::Model
            && first
                .confidence
                .is_some_and(|confidence| confidence < self.below)
    }

    /// The larger model's answer when `first` is unsure, else `first`.
    pub async fn review(&self, message: &TriageMessage, first: Proposal) -> Proposal {
        if !self.wanted(&first) {
            return first;
        }
        let mut fuller = message.clone();
        fuller.body_excerpt = message.fuller_excerpt.clone();
        let answer = self
            .backend
            .complete(&self.model, SYSTEM_PROMPT, &describe(&fuller))
            .await
            .and_then(|content| parse_verdict(&content));
        match answer {
            Ok(verdict) => Proposal {
                bucket: verdict.bucket,
                confidence: Some(verdict.confidence),
                model_id: Some(self.model.clone()),
                reason: format!(
                    "{} (second pass; {} said {} {:.2}: {})",
                    verdict.reason,
                    first.model_id.as_deref().unwrap_or("first model"),
                    first.bucket,
                    first.confidence.unwrap_or_default(),
                    first.reason
                ),
                ..first
            },
            Err(e) => {
                warn!(
                    "second-pass model {} failed for uid {}; keeping the first answer: {}",
                    self.model, message.uid, e
                );
                first
            }
        }
    }
}
