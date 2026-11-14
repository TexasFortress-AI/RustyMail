// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Verification codes and resets go to Trash once their window has passed,
//! and the rule order around them.

use chrono::{Duration, TimeZone, Utc};
use rustymail::triage::codes::{expiring_code, is_due, stated_window};
use rustymail::triage::message::TriageMessage;
use rustymail::triage::model::parse_verdict;
use rustymail::triage::rules::{apply_rules, SenderRules};
use rustymail::triage::Bucket;

const DEFAULT_WINDOW_HOURS: i64 = 24;

fn window() -> Duration {
    Duration::hours(DEFAULT_WINDOW_HOURS)
}

fn message(extra_headers: &str, from: &str, subject: &str, body: &str) -> TriageMessage {
    let raw = format!(
        "{extra_headers}From: {from}\r\nTo: chris@example.test\r\nSubject: {subject}\r\n\
         Date: Fri, 13 Nov 2026 10:00:00 +0000\r\nMessage-ID: <c@example.test>\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}"
    );
    TriageMessage::from_raw(1, raw.as_bytes(), 500).unwrap()
}

fn arrival() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 13, 10, 0, 0).unwrap()
}

#[test]
fn code_in_subject_is_trash_after_the_default_window() {
    let m = message(
        "",
        "no-reply@service.test",
        "Your verification code",
        "Use 482913.",
    );
    let proposal = expiring_code(&m, window()).unwrap();
    assert_eq!(proposal.bucket, Bucket::Trash);
    assert_eq!(proposal.not_before, Some(arrival() + window()));
}

#[test]
fn stated_expiry_sets_the_window() {
    let m = message(
        "",
        "no-reply@bank.test",
        "Sign-in code",
        "Your code is 551203. It expires in 10 minutes.",
    );
    assert_eq!(stated_window(&m), Some(Duration::minutes(10)));
    let proposal = expiring_code(&m, window()).unwrap();
    assert_eq!(proposal.not_before, Some(arrival() + Duration::minutes(10)));

    let reset = message(
        "",
        "security@shop.test",
        "Reset your password",
        "This link is valid for 2 hours.",
    );
    assert_eq!(
        expiring_code(&reset, window()).unwrap().not_before,
        Some(arrival() + Duration::hours(2))
    );
}

#[test]
fn absurd_stated_windows_fall_back_to_the_default() {
    let m = message("", "a@x.test", "Your one-time code", "valid for 400 days");
    assert_eq!(stated_window(&m), None);
    assert_eq!(
        expiring_code(&m, window()).unwrap().not_before,
        Some(arrival() + window())
    );
}

#[test]
fn body_phrase_needs_a_numeric_code() {
    let with_code = message("", "a@x.test", "Welcome", "Your verification code is 7731.");
    assert!(expiring_code(&with_code, window()).is_some());
    let without = message(
        "",
        "a@x.test",
        "Welcome",
        "We will send a verification code later.",
    );
    assert!(expiring_code(&without, window()).is_none());
}

#[test]
fn security_alerts_and_ordinary_mail_are_not_codes() {
    for (subject, body) in [
        (
            "New sign-in to your account",
            "We noticed a new sign-in from Chrome.",
        ),
        ("Lunch on Friday?", "Are you free at noon?"),
        ("Your order has shipped", "Tracking number 123456789012."),
    ] {
        let m = message("", "a@x.test", subject, body);
        assert!(expiring_code(&m, window()).is_none(), "{}", subject);
    }
}

#[test]
fn due_only_once_the_window_has_passed() {
    let deadline = arrival() + window();
    assert!(!is_due(Some(deadline), deadline - Duration::seconds(1)));
    assert!(is_due(Some(deadline), deadline));
    assert!(is_due(None, arrival()));
}

#[test]
fn codes_beat_known_senders_and_bulk_headers() {
    let senders = SenderRules::from_json(r#"{"domains": {"github.com": "Alerts"}}"#).unwrap();
    let m = message(
        "List-Unsubscribe: <mailto:u@github.com>\r\n",
        "noreply@github.com",
        "[GitHub] Please verify your device",
        "Verification code: 123456",
    );
    let proposal = apply_rules(&m, &senders, window()).unwrap();
    assert_eq!(proposal.bucket, Bucket::Trash);
}

#[test]
fn authentication_failure_beats_codes_and_known_senders() {
    let senders = SenderRules::from_json(r#"{"domains": {"bank.test": "Alerts"}}"#).unwrap();
    let m = message(
        "Authentication-Results: mx.example.test; dmarc=fail\r\n",
        "security@bank.test",
        "Your one-time passcode",
        "Code 99887766",
    );
    assert_eq!(
        apply_rules(&m, &senders, window()).unwrap().bucket,
        Bucket::Spam
    );
}

#[test]
fn the_model_may_not_choose_trash() {
    let err =
        parse_verdict(r#"{"bucket": "Trash", "confidence": 0.99, "reason": "old"}"#).unwrap_err();
    assert!(err.contains("Trash"), "{}", err);
}

#[test]
fn reference_numbers_and_meeting_details_are_not_codes() {
    for (subject, body) in [
        (
            "Your booking confirmation code: K7XQ2P",
            "Flight 1234 departs at 10:00.",
        ),
        (
            "Team sync",
            "Join Zoom Meeting. Meeting ID: 812 3344 5566 Passcode: 123456",
        ),
        (
            "Your conference access code",
            "Use access code 4471 to dial in.",
        ),
        (
            "Your password was reset",
            "If this was not you, call us at 5551234.",
        ),
        (
            "Password reset successful",
            "Your password was changed at 10:04.",
        ),
    ] {
        let m = message("", "a@x.test", subject, body);
        assert!(expiring_code(&m, window()).is_none(), "{}", subject);
    }
}

#[test]
fn reset_requests_are_codes() {
    for subject in [
        "Password reset request",
        "Your password reset link",
        "Reset your password",
    ] {
        let m = message("", "a@x.test", subject, "Click the link within 1 hour.");
        assert_eq!(
            expiring_code(&m, window()).map(|p| p.not_before),
            Some(Some(arrival() + Duration::hours(1))),
            "{}",
            subject
        );
    }
}
