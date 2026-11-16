// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Model verdict parsing, the loopback guard, and small-to-large escalation.

use rustymail::triage::message::TriageMessage;
use rustymail::triage::model::{
    ensure_loopback, parse_verdict, EscalatingClassifier, LocalModelServer, ModelBackend,
};
use rustymail::triage::{Bucket, ProposalSource};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// Replies per model name; a missing model is an error, like a server failure.
struct StubBackend(HashMap<&'static str, &'static str>);

impl ModelBackend for StubBackend {
    async fn complete(&self, model: &str, _system: &str, _user: &str) -> Result<String, String> {
        self.0
            .get(model)
            .map(|reply| reply.to_string())
            .ok_or_else(|| format!("{} unavailable", model))
    }
}

fn message() -> TriageMessage {
    let raw = b"From: Pat <pat@example.test>\r\nSubject: Lunch?\r\n\r\nAre you free Friday?";
    TriageMessage::from_raw(1, raw, 200).unwrap()
}

fn classifier(
    replies: &[(&'static str, &'static str)],
    large: bool,
) -> EscalatingClassifier<StubBackend> {
    EscalatingClassifier::new(
        StubBackend(replies.iter().copied().collect()),
        "small".to_string(),
        large.then(|| "large".to_string()),
        0.7,
    )
}

const SURE_REPLY: &str =
    r#"{"bucket": "Needs Reply", "confidence": 0.9, "reason": "direct question"}"#;
const UNSURE_NOISE: &str = r#"{"bucket": "Noise", "confidence": 0.4, "reason": "maybe bulk"}"#;
const LARGE_REPLY: &str =
    r#"{"bucket": "needs_reply", "confidence": 0.85, "reason": "asks a question"}"#;

#[test]
fn parses_plain_fenced_and_thinking_replies() {
    let plain = parse_verdict(SURE_REPLY).unwrap();
    assert_eq!(plain.bucket, Bucket::NeedsReply);
    assert_eq!(plain.confidence, 0.9);
    assert_eq!(plain.reason, "direct question");

    let fenced = "```json\n{\"bucket\": \"Accounting\", \"confidence\": 1.7}\n```";
    let verdict = parse_verdict(fenced).unwrap();
    assert_eq!(verdict.bucket, Bucket::Accounting);
    assert_eq!(verdict.confidence, 1.0, "confidence is clamped");

    let thinking = "<think>Is it {spam}?</think>{\"bucket\": \"Spam\", \"confidence\": 0.8}";
    assert_eq!(parse_verdict(thinking).unwrap().bucket, Bucket::Spam);
}

#[test]
fn rejects_unknown_buckets_and_non_json() {
    assert!(parse_verdict(r#"{"bucket": "Newsletter"}"#)
        .unwrap_err()
        .contains("Newsletter"));
    assert!(parse_verdict("I think this is spam").is_err());
}

#[test]
fn only_loopback_model_urls_are_allowed() {
    for ok in [
        "http://127.0.0.1:5846/v1",
        "http://localhost:5846/v1",
        "http://[::1]:5846/v1",
    ] {
        assert!(ensure_loopback(&Url::parse(ok).unwrap()).is_ok(), "{}", ok);
    }
    for bad in [
        "https://api.openai.com/v1",
        "http://192.168.1.5:5846/v1",
        "http://localhost.evil.example/v1",
    ] {
        assert!(
            ensure_loopback(&Url::parse(bad).unwrap()).is_err(),
            "{}",
            bad
        );
    }
    assert!(LocalModelServer::new("https://api.anthropic.com/v1", Duration::from_secs(5)).is_err());
}

#[tokio::test]
async fn confident_small_model_is_not_escalated() {
    let proposal = classifier(&[("small", SURE_REPLY), ("large", LARGE_REPLY)], true)
        .classify(&message())
        .await
        .unwrap();
    assert_eq!(proposal.bucket, Bucket::NeedsReply);
    assert_eq!(proposal.model_id.as_deref(), Some("small"));
    assert_eq!(proposal.source, ProposalSource::Model);
    assert_eq!(proposal.confidence, Some(0.9));
}

#[tokio::test]
async fn unsure_or_failed_small_model_escalates() {
    let unsure = classifier(&[("small", UNSURE_NOISE), ("large", LARGE_REPLY)], true);
    let proposal = unsure.classify(&message()).await.unwrap();
    assert_eq!(
        (proposal.bucket, proposal.model_id.as_deref()),
        (Bucket::NeedsReply, Some("large"))
    );

    let small_down = classifier(&[("large", LARGE_REPLY)], true);
    assert_eq!(
        small_down
            .classify(&message())
            .await
            .unwrap()
            .model_id
            .as_deref(),
        Some("large")
    );
}

#[tokio::test]
async fn falls_back_to_small_answer_when_large_model_fails() {
    let proposal = classifier(&[("small", UNSURE_NOISE)], true)
        .classify(&message())
        .await
        .unwrap();
    assert_eq!(
        (proposal.bucket, proposal.model_id.as_deref()),
        (Bucket::Noise, Some("small"))
    );
}

#[tokio::test]
async fn errors_when_no_model_answers() {
    assert!(classifier(&[], true).classify(&message()).await.is_err());
    assert!(classifier(&[], false).classify(&message()).await.is_err());
}

#[tokio::test]
async fn without_a_large_model_the_small_answer_stands() {
    let proposal = classifier(&[("small", UNSURE_NOISE)], false)
        .classify(&message())
        .await
        .unwrap();
    assert_eq!(proposal.model_id.as_deref(), Some("small"));
}
