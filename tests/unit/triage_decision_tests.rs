// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The decision-model classifier: parsing answers, combining them into a
//! bucket, and the request it sends.

use rustymail::triage::decision::{
    combine, parse_answers, questions, Decision, DecisionBackend, DecisionClassifier,
    LocalDecisionServer,
};
use rustymail::triage::message::TriageMessage;
use rustymail::triage::{Bucket, ProposalSource};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;

/// Answers in the shape Ollama's /v1/systemone returns them.
fn answers(personal: f64, asks: f64, kind: &str, kind_probability: f64) -> Value {
    json!({
        "personal": {"type": "noul", "noul": personal},
        "asks": {"type": "noul", "noul": asks},
        "kind": {
            "type": "choice",
            "choice": kind,
            "probabilities": {kind: kind_probability},
            "confidence": 0.5
        }
    })
}

fn decision(personal: f64, asks: f64, kind: &str, kind_probability: f64) -> Decision {
    parse_answers(&answers(personal, asks, kind, kind_probability)).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_person_asking_something_needs_a_reply() {
    let (bucket, confidence) = combine(&decision(0.9, 0.8, "spam", 0.9)).unwrap();
    assert_eq!(bucket, Bucket::NeedsReply);
    assert!(close(confidence, 0.72));
}

#[test]
fn a_person_only_informing_stays_in_the_inbox() {
    let (bucket, confidence) = combine(&decision(0.9, 0.2, "newsletter", 0.9)).unwrap();
    assert_eq!(bucket, Bucket::Keep);
    assert!(close(confidence, 0.72));
}

#[test]
fn automated_mail_takes_the_bucket_of_its_kind() {
    for (kind, bucket) in [
        ("accounting", Bucket::Accounting),
        ("legal", Bucket::Legal),
        ("credit", Bucket::Credit),
        ("alert", Bucket::Alerts),
        ("newsletter", Bucket::Noise),
        ("spam", Bucket::Spam),
    ] {
        let (got, confidence) = combine(&decision(0.1, 0.9, kind, 0.8)).unwrap();
        assert_eq!(got, bucket, "{kind}");
        assert!(close(confidence, 0.72), "{kind}");
    }
}

#[test]
fn doubt_about_who_wrote_it_lowers_confidence() {
    // 45% personal, 90% spam: Spam, but too unsure to clear the 0.8 apply bar.
    let (bucket, confidence) = combine(&decision(0.45, 0.1, "spam", 0.9)).unwrap();
    assert_eq!(bucket, Bucket::Spam);
    assert!(confidence < 0.5);
}

#[test]
fn an_unknown_kind_is_an_error() {
    assert!(combine(&decision(0.1, 0.1, "invoice", 0.9)).is_err());
}

#[test]
fn missing_or_malformed_answers_are_errors() {
    let mut no_asks = answers(0.9, 0.9, "alert", 0.9);
    no_asks.as_object_mut().unwrap().remove("asks");
    assert!(parse_answers(&no_asks).is_err());

    let mut no_probability = answers(0.1, 0.1, "alert", 0.9);
    no_probability["kind"]["probabilities"] = json!({"receipt": 0.9});
    assert!(parse_answers(&no_probability).is_err());

    assert!(parse_answers(&json!({})).is_err());
}

#[test]
fn probabilities_are_clamped() {
    let d = decision(1.7, -0.2, "alert", 3.0);
    assert_eq!((d.personal, d.asks, d.kind_probability), (1.0, 0.0, 1.0));
}

#[test]
fn every_kind_question_maps_to_a_bucket() {
    let q = questions();
    assert_eq!(q["personal"]["type"], "noul");
    assert_eq!(q["asks"]["type"], "noul");
    assert_eq!(q["kind"]["type"], "choice");
    for kind in q["kind"]["criteria"].as_object().unwrap().keys() {
        assert!(combine(&decision(0.0, 0.0, kind, 1.0)).is_ok(), "{kind}");
    }
}

#[test]
fn investor_updates_and_tax_forms_are_accounting() {
    let accounting = questions()["kind"]["criteria"]["accounting"]
        .as_str()
        .unwrap()
        .to_string();
    for phrase in [
        "investors or shareholders",
        "capital call",
        "W-2",
        "1099",
        "K-1",
    ] {
        assert!(accounting.contains(phrase), "{phrase}");
    }
}

#[test]
fn only_loopback_decision_servers_are_accepted() {
    let timeout = Duration::from_secs(1);
    assert!(LocalDecisionServer::new("http://127.0.0.1:11434", timeout).is_ok());
    assert!(LocalDecisionServer::new("http://localhost:11434", timeout).is_ok());
    assert!(LocalDecisionServer::new("https://api.example.com", timeout).is_err());
}

/// Records what was sent and replies with fixed answers.
struct StubDecider {
    reply: Result<Value, String>,
    sent: Mutex<Vec<(String, String, Value)>>,
}

impl DecisionBackend for StubDecider {
    async fn decide(&self, model: &str, state: &str, questions: &Value) -> Result<Value, String> {
        self.sent
            .lock()
            .unwrap()
            .push((model.into(), state.into(), questions.clone()));
        self.reply.clone()
    }
}

fn message() -> TriageMessage {
    let raw = "From: Pat <pat@example.test>\r\nTo: you@example.test\r\nSubject: Lunch?\r\n\
               Content-Type: text/plain\r\n\r\nAre you free on Friday?\r\n";
    TriageMessage::from_raw(9, raw.as_bytes(), 500).unwrap()
}

#[tokio::test]
async fn classify_sends_the_message_summary_and_reports_the_probabilities() {
    let stub = StubDecider {
        reply: Ok(answers(0.9, 0.8, "spam", 0.6)),
        sent: Mutex::new(Vec::new()),
    };
    let classifier = DecisionClassifier::new(stub, "decider".into());
    let proposal = classifier.classify(&message()).await.unwrap();

    assert_eq!(proposal.bucket, Bucket::NeedsReply);
    assert_eq!(proposal.source, ProposalSource::Model);
    assert_eq!(proposal.model_id.as_deref(), Some("decider"));
    assert_eq!(proposal.reason, "personal 0.90, asks 0.80, kind spam 0.60");
    assert!(close(proposal.confidence.unwrap(), 0.72));
}

#[tokio::test]
async fn classify_passes_backend_errors_through() {
    let stub = StubDecider {
        reply: Err("decision server returned 500".into()),
        sent: Mutex::new(Vec::new()),
    };
    let classifier = DecisionClassifier::new(stub, "decider".into());
    let error = classifier.classify(&message()).await.unwrap_err();
    assert!(error.contains("500"));
}
