// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The larger model's second pass over answers the first model is unsure of.

use rustymail::triage::message::TriageMessage;
use rustymail::triage::model::ModelBackend;
use rustymail::triage::second_pass::SecondPass;
use rustymail::triage::{Bucket, Proposal, ProposalSource};
use std::sync::{Arc, Mutex};

/// Replies once with `reply` (or fails with it) and records what it was sent.
struct StubLarge {
    reply: Result<&'static str, &'static str>,
    sent: Arc<Mutex<Vec<String>>>,
}

impl ModelBackend for StubLarge {
    async fn complete(&self, _model: &str, _system: &str, user: &str) -> Result<String, String> {
        self.sent.lock().unwrap().push(user.to_string());
        self.reply.map(str::to_string).map_err(str::to_string)
    }
}

type Sent = Arc<Mutex<Vec<String>>>;

/// A second pass below 0.8, and what its model is sent.
fn second_pass(reply: Result<&'static str, &'static str>) -> (SecondPass<StubLarge>, Sent) {
    let sent = Sent::default();
    let stub = StubLarge {
        reply,
        sent: sent.clone(),
    };
    (SecondPass::new(stub, "large".to_string(), 0.8), sent)
}

fn first(bucket: Bucket, confidence: f64) -> Proposal {
    Proposal {
        bucket,
        confidence: Some(confidence),
        source: ProposalSource::Model,
        model_id: Some("first".to_string()),
        reason: "personal 0.20, asks 0.10, kind investor 0.70".to_string(),
        not_before: None,
        list_name: None,
        document: Some((Bucket::Legal, 0.6)),
    }
}

/// A body of 40 "word" tokens: 200 characters; the first model sees 50.
fn message() -> TriageMessage {
    let body = "word ".repeat(40) + "TAIL";
    let raw = format!("From: ir@fund.example\r\nSubject: Quarterly update\r\n\r\n{body}");
    TriageMessage::from_raw_with_fuller(1, raw.as_bytes(), 50, 1000).unwrap()
}

const SURE_ACCOUNTING: &str =
    r#"{"bucket": "Accounting", "confidence": 0.92, "reason": "investor update"}"#;

#[tokio::test]
async fn an_unsure_answer_is_replaced_by_the_larger_model() {
    let (pass, _) = second_pass(Ok(SURE_ACCOUNTING));
    let p = pass.review(&message(), first(Bucket::Keep, 0.7)).await;
    assert_eq!(p.bucket, Bucket::Accounting);
    assert_eq!(p.confidence, Some(0.92));
    assert_eq!(p.model_id.as_deref(), Some("large"));
    assert!(
        p.reason.contains("second pass; first said Keep 0.70"),
        "{}",
        p.reason
    );
    // What the first model knew about the document type is kept.
    assert_eq!(p.document, Some((Bucket::Legal, 0.6)));
}

#[tokio::test]
async fn the_larger_model_reads_the_longer_excerpt() {
    let message = message();
    assert!(!message.body_excerpt.contains("TAIL"));
    assert!(message.fuller_excerpt.contains("TAIL"));
    let (pass, sent) = second_pass(Ok(SURE_ACCOUNTING));
    pass.review(&message, first(Bucket::Keep, 0.5)).await;
    let large = sent.lock().unwrap().concat();
    assert!(large.contains("TAIL"), "{large}");
    assert!(
        large.starts_with("<<<EMAIL"),
        "the email stays between its markers"
    );
}

#[tokio::test]
async fn sure_answers_and_rules_are_not_asked_again() {
    let (pass, sent) = second_pass(Ok(SURE_ACCOUNTING));
    let sure = first(Bucket::Noise, 0.8);
    assert_eq!(pass.review(&message(), sure.clone()).await, sure);
    let rule = Proposal::from_rule(Bucket::Lists, "List-Id");
    assert_eq!(pass.review(&message(), rule.clone()).await, rule);
    assert!(sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failed_second_pass_keeps_the_first_answer() {
    let unsure = first(Bucket::Keep, 0.4);
    let (missing, _) = second_pass(Err("not_found: no router for requested model"));
    assert_eq!(missing.review(&message(), unsure.clone()).await, unsure);
    let (garbled, _) = second_pass(Ok("I think this is probably accounting"));
    assert_eq!(garbled.review(&message(), unsure.clone()).await, unsure);
}

#[test]
fn rules_and_the_first_model_see_the_usual_excerpt() {
    let raw = b"From: a@b.example\r\nSubject: s\r\n\r\none two three four five";
    let plain = TriageMessage::from_raw(1, raw, 9).unwrap();
    let with_fuller = TriageMessage::from_raw_with_fuller(1, raw, 9, 100).unwrap();
    assert_eq!(plain.body_excerpt, "one two t");
    assert_eq!(with_fuller.body_excerpt, plain.body_excerpt);
    assert_eq!(plain.fuller_excerpt, plain.body_excerpt);
    assert_eq!(with_fuller.fuller_excerpt, "one two three four five");
}

#[tokio::test]
async fn a_correspondents_mail_is_only_left_for_a_reply_or_filed_as_a_document() {
    let mut from_colleague = message();
    from_colleague.from_correspondent = true;
    let unsure = first(Bucket::Legal, 0.75);
    // "I resent the DocuSigns with the updated NCA": not an alert from a person.
    let (alerts, _) = second_pass(Ok(r#"{"bucket": "Alerts", "confidence": 0.9}"#));
    assert_eq!(alerts.review(&from_colleague, unsure.clone()).await, unsure);
    // A W-9 from a colleague is still filed as a document.
    let (pass, _) = second_pass(Ok(SURE_ACCOUNTING));
    let p = pass.review(&from_colleague, unsure).await;
    assert_eq!(p.bucket, Bucket::Accounting);
}
