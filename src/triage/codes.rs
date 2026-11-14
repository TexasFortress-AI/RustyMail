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

/// Phrases that mark a one-time code, link or reset. Security notices such as
/// "new sign-in to your account" are deliberately absent: those are Alerts.
static CODE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(verification code|verify your (email|e-mail|account|identity)|confirm your (email|e-mail) address|one[- ]time (code|passcode|password|pin|link)|\botp\b|security code|log[- ]?in code|sign[- ]?in code|sign[- ]?in link|magic link|passcode|password reset|reset your password|two[- ]factor (code|authentication code)|2fa code|authentication code|confirmation code|access code)",
    )
    .expect("valid code phrase pattern")
});

/// A 4-8 digit code on its own, as these messages carry.
static CODE_DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4,8}\b").expect("valid code digits pattern"));

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

/// A code phrase in the subject, or in the body next to a numeric code.
fn is_code_message(message: &TriageMessage) -> bool {
    CODE_PHRASES.is_match(&message.subject)
        || (CODE_PHRASES.is_match(&message.body_excerpt)
            && CODE_DIGITS.is_match(&message.body_excerpt))
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
