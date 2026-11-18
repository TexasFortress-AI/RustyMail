// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Classify with a decision model (e.g. autotrust-jev on Ollama): instead of
//! writing text, it returns probabilities for fixed questions, so confidence
//! means something. Three narrow questions are asked together and combined
//! here, rather than one question with every bucket.
//!
//! Only loopback servers are accepted, as for the chat model.

use super::message::TriageMessage;
use super::model::{describe, ensure_loopback};
use super::{Bucket, Proposal, ProposalSource};
use serde_json::{json, Value};
use std::future::Future;
use std::time::Duration;
use url::Url;

/// Ollama's decision endpoint, relative to the server root.
const DECISION_PATH: &str = "v1/systemone";
/// A yes/no probability at or above this counts as yes.
const YES_AT: f64 = 0.5;

const PERSONAL: &str = "personal";
const ASKS: &str = "asks";
const KIND: &str = "kind";

/// Automated-mail kinds and the bucket each one maps to.
const KINDS: [(&str, Bucket, &str); 6] = [
    (
        "accounting",
        Bucket::Accounting,
        "A record of money that moved or is owed: a receipt, invoice, bill, account \
         statement, payment or transfer sent or received, card or bank transaction \
         notice, purchase or order confirmation, a crypto or brokerage deposit, an \
         update or report to investors or shareholders, a capital call, or a tax form \
         such as a W-2, 1099 or K-1.",
    ),
    (
        "legal",
        Bucket::Legal,
        "Terms the recipient is party to: updated terms of service or privacy policy, \
         a contract or agreement, a warranty, insurance or coverage document, or a \
         legal notice.",
    ),
    (
        "credit",
        Bucket::Credit,
        "About the recipient's credit itself: a new credit report or score, a credit \
         monitoring or identity alert, a new inquiry or account on their credit file, a \
         dispute with a credit bureau, or a debt collection notice or settlement offer: \
         any message from a collection agency about a balance owed to another company, \
         including demands to pay so a service is restored.",
    ),
    (
        "alert",
        Bucket::Alerts,
        "Something about the recipient's own account that needs attention or is \
         time-sensitive: a declined or failed payment, a due date or final reminder, an \
         upcoming renewal, a delivery or shipping status, a security or sign-in notice, \
         an invitation, chat or connection request, a shared file or a signature request.",
    ),
    (
        "newsletter",
        Bucket::Noise,
        "A newsletter, marketing or promotion (including offers from banks, cards and \
         payment apps), a digest, or a social-media notification.",
    ),
    (
        "spam",
        Bucket::Spam,
        "Unsolicited junk, a scam, phishing or fraud.",
    ),
];

/// One decision request. The HTTP implementation talks to Ollama; tests use a stub.
pub trait DecisionBackend {
    /// Returns the `answers` object, keyed by question name.
    fn decide(
        &self,
        model: &str,
        state: &str,
        questions: &Value,
    ) -> impl Future<Output = Result<Value, String>> + Send;
}

pub struct LocalDecisionServer {
    http: reqwest::Client,
    decide_url: String,
}

impl LocalDecisionServer {
    /// `base_url` is the server root, e.g. `http://127.0.0.1:<port>`.
    pub fn new(base_url: &str, timeout: Duration) -> Result<Self, String> {
        let url = Url::parse(base_url).map_err(|e| format!("invalid decision URL: {}", e))?;
        ensure_loopback(&url)?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| format!("cannot build HTTP client: {}", e))?;
        Ok(Self {
            http,
            decide_url: format!("{}/{}", base_url.trim_end_matches('/'), DECISION_PATH),
        })
    }
}

impl DecisionBackend for LocalDecisionServer {
    async fn decide(&self, model: &str, state: &str, questions: &Value) -> Result<Value, String> {
        let body = json!({"model": model, "state": state, "questions": questions});
        let response = self
            .http
            .post(&self.decide_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("decision request failed: {}", e))?;
        let status = response.status();
        let mut payload: Value = response
            .json()
            .await
            .map_err(|e| format!("decision response is not JSON ({}): {}", status, e))?;
        if !status.is_success() {
            return Err(format!("decision server returned {}: {}", status, payload));
        }
        match payload.get_mut("answers") {
            Some(answers) => Ok(answers.take()),
            None => Err(format!("decision response has no answers: {}", payload)),
        }
    }
}

/// The three questions asked about every message.
pub fn questions() -> Value {
    let kinds: serde_json::Map<String, Value> = KINDS
        .iter()
        .map(|(name, _, description)| (name.to_string(), json!(description)))
        .collect();
    json!({
        PERSONAL: {
            "type": "noul",
            "instructions": "Did a person write this email themselves to the recipient?",
            "criteria": {
                "true": "A person wrote it, to the recipient or a small group that includes them.",
                "false": "A system, service, company or mailing list sent it automatically."
            }
        },
        ASKS: {
            "type": "noul",
            "instructions": "Does the sender ask the recipient to reply, answer, decide or do something?",
            "criteria": {
                "true": "It asks the recipient a question or for an action.",
                "false": "It only informs; nothing is asked of the recipient."
            }
        },
        KIND: {
            "type": "choice",
            "instructions": "If this email was sent automatically, what kind is it?",
            "criteria": kinds
        }
    })
}

/// The decision model's answers for one message.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub personal: f64,
    pub asks: f64,
    pub kind: String,
    pub kind_probability: f64,
}

/// Read the three answers; anything missing or malformed is an error.
pub fn parse_answers(answers: &Value) -> Result<Decision, String> {
    let yes = |name: &str| {
        answers[name]["noul"]
            .as_f64()
            .map(|p| p.clamp(0.0, 1.0))
            .ok_or_else(|| format!("decision answer '{}' has no probability", name))
    };
    let kind = answers[KIND]["choice"]
        .as_str()
        .ok_or("decision answer 'kind' has no choice")?
        .to_string();
    let kind_probability = answers[KIND]["probabilities"][&kind]
        .as_f64()
        .ok_or("decision answer 'kind' has no probability for its choice")?
        .clamp(0.0, 1.0);
    Ok(Decision {
        personal: yes(PERSONAL)?,
        asks: yes(ASKS)?,
        kind,
        kind_probability,
    })
}

/// A person asking something: Needs Reply. A person only informing: Keep.
/// Automated mail: the bucket of its kind. Confidence multiplies the
/// probabilities along the path taken, so doubt at any step lowers it.
pub fn combine(decision: &Decision) -> Result<(Bucket, f64), String> {
    if decision.personal >= YES_AT {
        return Ok(if decision.asks >= YES_AT {
            (Bucket::NeedsReply, decision.personal * decision.asks)
        } else {
            (Bucket::Keep, decision.personal * (1.0 - decision.asks))
        });
    }
    let bucket = KINDS
        .iter()
        .find(|(name, _, _)| *name == decision.kind)
        .map(|(_, bucket, _)| *bucket)
        .ok_or_else(|| format!("decision model chose unknown kind '{}'", decision.kind))?;
    Ok((
        bucket,
        (1.0 - decision.personal) * decision.kind_probability,
    ))
}

pub struct DecisionClassifier<B: DecisionBackend> {
    backend: B,
    model: String,
    questions: Value,
}

impl<B: DecisionBackend> DecisionClassifier<B> {
    pub fn new(backend: B, model: String) -> Self {
        Self {
            backend,
            model,
            questions: questions(),
        }
    }

    pub async fn classify(&self, message: &TriageMessage) -> Result<Proposal, String> {
        let answers = self
            .backend
            .decide(&self.model, &describe(message), &self.questions)
            .await?;
        let decision = parse_answers(&answers)?;
        let (bucket, confidence) = combine(&decision)?;
        Ok(Proposal {
            bucket,
            confidence: Some(confidence),
            source: ProposalSource::Model,
            model_id: Some(self.model.clone()),
            reason: format!(
                "personal {:.2}, asks {:.2}, kind {} {:.2}",
                decision.personal, decision.asks, decision.kind, decision.kind_probability
            ),
            not_before: None,
            list_name: None,
        })
    }
}
