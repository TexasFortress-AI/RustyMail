// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Ask a local model for a bucket when the header rules cannot decide.
//!
//! Only loopback model servers are accepted: classification must never go to a
//! paid cloud API.

use super::message::TriageMessage;
use super::{Bucket, Proposal, ProposalSource};
use serde::Deserialize;
use std::future::Future;
use std::time::Duration;
use url::{Host, Url};

const SYSTEM_PROMPT: &str = "You sort email for a busy person. Choose exactly one bucket:\n\
- Spam: unsolicited, scam, phishing or deceptive mail.\n\
- Noise: newsletters, marketing, promotions, social-media notifications and automated digests that need no action.\n\
- Receipts: purchase confirmations, invoices, payment receipts, order and shipping updates, billing notices.\n\
- Alerts: automated notices that may matter: security alerts, sign-in or verification codes, password resets, account or system notifications.\n\
- Needs Reply: a real person wrote directly to the recipient and expects an answer or action.\n\
- Keep: anything else worth leaving in the inbox.\n\
Reply with JSON only: {\"bucket\": \"<bucket>\", \"confidence\": <0.0-1.0>, \"reason\": \"<short reason>\"}";

/// One chat completion against a model. The HTTP implementation talks to
/// model_serve; tests use a stub.
pub trait ModelBackend {
    fn complete(
        &self,
        model: &str,
        system: &str,
        user: &str,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

/// OpenAI-compatible chat endpoint on a loopback address.
pub struct LocalModelServer {
    http: reqwest::Client,
    chat_url: String,
}

impl LocalModelServer {
    /// `base_url` is the API root, e.g. `http://127.0.0.1:<port>/v1`.
    pub fn new(base_url: &str, timeout: Duration) -> Result<Self, String> {
        let url = Url::parse(base_url).map_err(|e| format!("invalid model URL: {}", e))?;
        ensure_loopback(&url)?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| format!("cannot build HTTP client: {}", e))?;
        Ok(Self {
            http,
            chat_url: format!("{}/chat/completions", base_url.trim_end_matches('/')),
        })
    }
}

/// Refuse anything but localhost, 127.0.0.0/8 or ::1.
pub fn ensure_loopback(url: &Url) -> Result<(), String> {
    let local = match url.host() {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if local {
        Ok(())
    } else {
        Err(format!(
            "model URL {} is not a loopback address; triage only uses local models",
            url
        ))
    }
}

impl ModelBackend for LocalModelServer {
    async fn complete(&self, model: &str, system: &str, user: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": model,
            "temperature": 0,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        let response = self
            .http
            .post(&self.chat_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("model request failed: {}", e))?;
        let status = response.status();
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("model response is not JSON ({}): {}", status, e))?;
        if !status.is_success() {
            return Err(format!("model server returned {}: {}", status, payload));
        }
        payload["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("model response has no message content: {}", payload))
    }
}

/// The model's answer for one message.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelVerdict {
    pub bucket: Bucket,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Deserialize)]
struct RawVerdict {
    bucket: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    reason: String,
}

/// Extract the JSON verdict, tolerating `<think>` blocks or text around it.
pub fn parse_verdict(content: &str) -> Result<ModelVerdict, String> {
    let visible = match content.rfind("</think>") {
        Some(end) => &content[end + "</think>".len()..],
        None => content,
    };
    let start = visible.find('{').ok_or("no JSON object in model reply")?;
    let end = visible.rfind('}').ok_or("no JSON object in model reply")?;
    let raw: RawVerdict = serde_json::from_str(&visible[start..=end])
        .map_err(|e| format!("model reply is not a verdict: {}", e))?;
    let bucket = Bucket::parse(&raw.bucket)
        .filter(|bucket| *bucket != Bucket::Trash)
        .ok_or_else(|| format!("model chose unknown bucket '{}'", raw.bucket))?;
    Ok(ModelVerdict {
        bucket,
        confidence: raw.confidence.clamp(0.0, 1.0),
        reason: raw.reason,
    })
}

/// The user message: sender, subject, bulk hint and body excerpt.
pub fn describe(message: &TriageMessage) -> String {
    let sender = match &message.from_name {
        Some(name) => format!("{} <{}>", name, message.from_address),
        None => message.from_address.clone(),
    };
    let bulk = if message.has_header("list-id") {
        "yes"
    } else {
        "no"
    };
    format!(
        "From: {}\nSubject: {}\nMailing list: {}\n\n{}",
        sender, message.subject, bulk, message.body_excerpt
    )
}

/// Small model first; a larger one only when the small one is unsure or fails.
pub struct EscalatingClassifier<B: ModelBackend> {
    backend: B,
    small_model: String,
    large_model: Option<String>,
    escalate_below: f64,
}

impl<B: ModelBackend> EscalatingClassifier<B> {
    pub fn new(
        backend: B,
        small_model: String,
        large_model: Option<String>,
        escalate_below: f64,
    ) -> Self {
        Self {
            backend,
            small_model,
            large_model,
            escalate_below,
        }
    }

    pub async fn classify(&self, message: &TriageMessage) -> Result<Proposal, String> {
        let first = self.ask(&self.small_model, message).await;
        let Some(large_model) = &self.large_model else {
            return first.map(|v| to_proposal(v, &self.small_model));
        };
        if let Ok(verdict) = &first {
            if verdict.confidence >= self.escalate_below {
                return Ok(to_proposal(verdict.clone(), &self.small_model));
            }
        }
        match (self.ask(large_model, message).await, first) {
            (Ok(verdict), _) => Ok(to_proposal(verdict, large_model)),
            (Err(_), Ok(verdict)) => Ok(to_proposal(verdict, &self.small_model)),
            (Err(e), Err(_)) => Err(e),
        }
    }

    async fn ask(&self, model: &str, message: &TriageMessage) -> Result<ModelVerdict, String> {
        let content = self
            .backend
            .complete(model, SYSTEM_PROMPT, &describe(message))
            .await?;
        parse_verdict(&content)
    }
}

fn to_proposal(verdict: ModelVerdict, model: &str) -> Proposal {
    Proposal {
        bucket: verdict.bucket,
        confidence: Some(verdict.confidence),
        source: ProposalSource::Model,
        model_id: Some(model.to_string()),
        reason: verdict.reason,
        not_before: None,
    }
}
