// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! What the runner asks when the rules cannot decide: one proposal per
//! message, from whichever local model the configuration selects.

use super::decision::{DecisionBackend, DecisionClassifier, LocalDecisionServer};
use super::message::TriageMessage;
use super::model::{EscalatingClassifier, LocalModelServer, ModelBackend};
use super::second_pass::SecondPass;
use super::Proposal;
use std::future::Future;

pub trait Classifier {
    fn classify(&self, message: &TriageMessage) -> impl Future<Output = Result<Proposal, String>>;
}

impl<B: ModelBackend> Classifier for EscalatingClassifier<B> {
    fn classify(&self, message: &TriageMessage) -> impl Future<Output = Result<Proposal, String>> {
        EscalatingClassifier::classify(self, message)
    }
}

impl<B: DecisionBackend> Classifier for DecisionClassifier<B> {
    fn classify(&self, message: &TriageMessage) -> impl Future<Output = Result<Proposal, String>> {
        DecisionClassifier::classify(self, message)
    }
}

/// The classifier the binary builds from TRIAGE_* settings.
pub enum ConfiguredClassifier {
    /// A chat model that answers with a JSON bucket.
    Chat(EscalatingClassifier<LocalModelServer>),
    /// A decision model that returns probabilities for fixed questions, with
    /// an optional larger model for the answers it is unsure about.
    Decision(
        DecisionClassifier<LocalDecisionServer>,
        Option<SecondPass<LocalModelServer>>,
    ),
}

impl Classifier for ConfiguredClassifier {
    async fn classify(&self, message: &TriageMessage) -> Result<Proposal, String> {
        match self {
            ConfiguredClassifier::Chat(chat) => chat.classify(message).await,
            ConfiguredClassifier::Decision(decision, None) => decision.classify(message).await,
            ConfiguredClassifier::Decision(decision, Some(second)) => {
                let first = decision.classify(message).await?;
                Ok(second.review(message, first).await)
            }
        }
    }
}
