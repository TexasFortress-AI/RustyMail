// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Verification codes, sign-in links and password resets are useless once
//! their window has passed. They are proposed for Trash with a `not_before`
//! time: apply leaves them alone until the window is over.

use super::message::TriageMessage;
use super::{Bucket, Proposal};
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use std::sync::LazyLock;

/// Phrases that mark a one-time code, link or reset. Deliberately absent:
/// security notices ("new sign-in to your account", "your password was
/// reset") are Alerts, and bare "passcode", "confirmation code" and
/// "access code" also name meeting passcodes, booking references and dial-in
/// codes, which must never be binned. "Reset your password" and "security
/// code" are in `SUBJECT_ONLY_PHRASES`.
static CODE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(verification code|verify your (email|e-mail|account|identity)|confirm your (email|e-mail) address|one[- ]time (code|passcode|password|pin|link)|\botp\b|log[- ]?in code|sign[- ]?in code|sign[- ]?in link|magic link|password reset (request|link|code)|two[- ]factor (code|authentication code)|2fa code|authentication code)",
    )
    .expect("valid code phrase pattern")
});

/// Phrases that mark a code or reset only in the subject. In a body, "reset
/// your password" is mostly the advice in a security notice ("if this wasn't
/// you, please reset your password"), and "security code" is often a card's
/// CSC in a payment notice.
static SUBJECT_ONLY_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(reset your password|security code)\b")
        .expect("valid subject-only phrase pattern")
});

/// A helpdesk ticket or case number in the subject marks a support
/// conversation, which may be about a code but never is one.
static SUPPORT_TICKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(ticket|case|request)\s*#\s*\d+").expect("valid support ticket pattern")
});

/// A subject announcing a security event: someone tried to get in, or the
/// account changed. The message may carry a code, but the record of the
/// event matters after the code expires, so it is an alert, not a code.
static SECURITY_EVENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(attempts?|attempted|changes?|changed|new (device|sign[- ]?in|log[- ]?in))\b",
    )
    .expect("valid security event pattern")
});

/// A subject that describes verifying rather than asking for it: "use the
/// app to verify your identity" announces a feature; a code mail says
/// "Verify your email" or names the code.
static DESCRIBES_VERIFYING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bto (verify|confirm|reset)\b").expect("valid describing pattern")
});

/// A candidate code token: digits, or capitals and digits ("D7BJGIU2KK").
/// `is_code_token` decides which candidates are codes.
static CODE_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z0-9]{4,12}\b").expect("valid code token pattern"));

/// Four-digit numbers in this range are read as years, not codes.
const YEARS: std::ops::RangeInclusive<u16> = 1900..=2099;

/// How far, in bytes, a code may sit from its phrase in the body. Further
/// away, a number is a year, an address or a price, not the code.
const CODE_NEAR_BYTES: usize = 80;

/// "expires in 10 minutes", "valid for 2 hours", "within 24 hours", "for 3 days".
static STATED_WINDOW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:expires?|expiring|valid|good|active|use it|within|for the next)\b[^.\d]{0,20}?(\d{1,3})\s*(minutes?|mins?|hours?|hrs?|days?)\b",
    )
    .expect("valid stated window pattern")
});

/// A stated window longer than this is ignored in favour of the default.
const MAX_STATED_WINDOW_DAYS: i64 = 30;

/// Trash proposal for a code/reset message, applicable once its window has
/// passed. `default_window` applies when the message states none.
pub fn expiring_code(message: &TriageMessage, default_window: Duration) -> Option<Proposal> {
    if !is_code_message(message) {
        return None;
    }
    let window = stated_window(message).unwrap_or(default_window);
    let received = message.received_at.unwrap_or_else(Utc::now);
    let mut proposal = Proposal::from_rule(
        Bucket::Trash,
        format!(
            "one-time code or reset, expires {} after arrival",
            describe(window)
        ),
    );
    proposal.not_before = Some(received + window);
    Some(proposal)
}

/// A code phrase in the subject, or in the body next to a code, but not in a
/// support conversation, a security event, or a subject describing verifying.
fn is_code_message(message: &TriageMessage) -> bool {
    let subject = &message.subject;
    if SUPPORT_TICKET.is_match(subject)
        || SECURITY_EVENT.is_match(subject)
        || DESCRIBES_VERIFYING.is_match(subject)
    {
        return false;
    }
    CODE_PHRASES.is_match(&message.subject)
        || SUBJECT_ONLY_PHRASES.is_match(&message.subject)
        || code_next_to_phrase(&message.body_excerpt)
}

/// A code phrase in `body` with a code token within `CODE_NEAR_BYTES` of it.
fn code_next_to_phrase(body: &str) -> bool {
    let codes: Vec<_> = CODE_TOKEN
        .find_iter(body)
        .filter(|token| is_code_token(token.as_str()))
        .collect();
    CODE_PHRASES.find_iter(body).any(|phrase| {
        codes.iter().any(|code| {
            code.start() <= phrase.end() + CODE_NEAR_BYTES
                && phrase.start() <= code.end() + CODE_NEAR_BYTES
        })
    })
}

/// 4-8 digits, or 6-12 capitals and digits with at least one digit. Longer
/// numbers are tracking and account numbers; all-capital words are words; a
/// year is the date of the message, not a code.
fn is_code_token(token: &str) -> bool {
    let digits = token.bytes().filter(u8::is_ascii_digit).count();
    if digits == token.len() {
        (4..=8).contains(&digits) && !is_year(token)
    } else {
        digits > 0 && token.len() >= 6
    }
}

/// A four-digit year in the range dates in mail use.
fn is_year(token: &str) -> bool {
    token.len() == 4 && token.parse::<u16>().is_ok_and(|year| YEARS.contains(&year))
}

/// The longest window the message states, if any.
pub fn stated_window(message: &TriageMessage) -> Option<Duration> {
    let text = format!("{} {}", message.subject, message.body_excerpt);
    STATED_WINDOW
        .captures_iter(&text)
        .filter_map(|caps| {
            let amount: i64 = caps[1].parse().ok()?;
            let unit = caps[2].to_ascii_lowercase();
            let window = if unit.starts_with("min") {
                Duration::minutes(amount)
            } else if unit.starts_with('h') {
                Duration::hours(amount)
            } else {
                Duration::days(amount)
            };
            (amount > 0 && window <= Duration::days(MAX_STATED_WINDOW_DAYS)).then_some(window)
        })
        .max()
}

fn describe(window: Duration) -> String {
    if window.num_minutes() < 120 {
        format!("{} min", window.num_minutes())
    } else if window.num_hours() < 48 {
        format!("{} h", window.num_hours())
    } else {
        format!("{} days", window.num_days())
    }
}

/// Whether a proposal may be applied now.
pub fn is_due(not_before: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    not_before.is_none_or(|time| time <= now)
}
