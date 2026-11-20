// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Cheap rules that decide a bucket from headers alone, before any model call.
//!
//! Order: authentication failures (Spam), expiring codes and resets (Trash,
//! once expired), the owner's known senders, then debt-collector notices
//! (Credit). Anything else goes to the
//! model. Authentication comes first because a sender that fails it is not
//! who its From header claims.
//!
//! Bulk-mail headers are not decisive: newsletters carry them, but so do
//! signature requests, statements and chat notifications. They only decide
//! after the model has answered; see `bulk_mail_default`.

use super::codes::expiring_code;
use super::message::TriageMessage;
use super::{Bucket, Proposal};
use chrono::Duration;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

/// A DMARC result whose From domain publishes policy `none`: the owner asks
/// receivers not to act on failures, and its own mail sent through third
/// parties often fails alignment. Written `policy.dmarc=none` or `(p=NONE ...)`.
static DMARC_POLICY_NONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:policy\.dmarc|p)=none\b").expect("valid DMARC policy pattern")
});

/// Known senders mapped to buckets, loaded from a JSON file:
/// `{"addresses": {"orders@shop.example": "Accounting"}, "domains": {"github.com": "Alerts"},
///   "subjects": [{"domain": "bank.example", "subject": "^Your statement", "bucket": "Accounting"}]}`.
/// A domain also matches its subdomains. Subject rules, for senders whose mail
/// splits between buckets, are case-insensitive patterns and are tried first.
#[derive(Debug, Default)]
pub struct SenderRules {
    addresses: HashMap<String, Bucket>,
    domains: HashMap<String, Bucket>,
    subjects: Vec<SubjectRule>,
}

/// One sender domain's mail whose subject matches `subject`.
#[derive(Debug)]
struct SubjectRule {
    domain: String,
    subject: Regex,
    bucket: Bucket,
}

#[derive(Deserialize)]
struct SenderRulesFile {
    #[serde(default)]
    addresses: HashMap<String, String>,
    #[serde(default)]
    domains: HashMap<String, String>,
    #[serde(default)]
    subjects: Vec<SubjectRuleEntry>,
}

#[derive(Deserialize)]
struct SubjectRuleEntry {
    domain: String,
    subject: String,
    bucket: String,
}

impl SenderRules {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: SenderRulesFile =
            serde_json::from_str(text).map_err(|e| format!("invalid sender rules: {}", e))?;
        Ok(Self {
            addresses: parse_entries(file.addresses)?,
            domains: parse_entries(file.domains)?,
            subjects: file
                .subjects
                .into_iter()
                .map(SubjectRule::parse)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        Self::from_json(&text)
    }

    fn lookup(&self, message: &TriageMessage) -> Option<Proposal> {
        if let Some(rule) = self.subjects.iter().find(|rule| rule.matches(message)) {
            let reason = format!("known subject from {}", rule.domain);
            return Some(Proposal::from_rule(rule.bucket, reason));
        }
        if let Some(bucket) = self.addresses.get(&message.from_address) {
            let reason = format!("known sender {}", message.from_address);
            return Some(Proposal::from_rule(*bucket, reason));
        }
        let mut domain = message.from_domain()?;
        loop {
            if let Some(bucket) = self.domains.get(domain) {
                return Some(Proposal::from_rule(
                    *bucket,
                    format!("known domain {}", domain),
                ));
            }
            domain = domain.split_once('.')?.1;
        }
    }
}

impl SubjectRule {
    fn parse(entry: SubjectRuleEntry) -> Result<Self, String> {
        let bucket = Bucket::parse(&entry.bucket)
            .ok_or_else(|| format!("unknown bucket '{}' for '{}'", entry.bucket, entry.domain))?;
        let subject = Regex::new(&format!("(?i){}", entry.subject))
            .map_err(|e| format!("invalid subject pattern '{}': {}", entry.subject, e))?;
        Ok(Self {
            domain: entry.domain.trim().to_ascii_lowercase(),
            subject,
            bucket,
        })
    }

    /// The sender's domain is `domain` or a subdomain of it, and the subject matches.
    fn matches(&self, message: &TriageMessage) -> bool {
        let Some(sender) = message.from_domain() else {
            return false;
        };
        let in_domain = sender == self.domain || sender.ends_with(&format!(".{}", self.domain));
        in_domain && self.subject.is_match(&message.subject)
    }
}

fn parse_entries(entries: HashMap<String, String>) -> Result<HashMap<String, Bucket>, String> {
    entries
        .into_iter()
        .map(|(key, value)| {
            let bucket = Bucket::parse(&value)
                .ok_or_else(|| format!("unknown bucket '{}' for '{}'", value, key))?;
            Ok((key.trim().to_ascii_lowercase(), bucket))
        })
        .collect()
}

/// The bucket the rules decide for this message, or None to ask the model.
/// `code_window` is how long a code or reset stays useful when it says nothing.
pub fn apply_rules(
    message: &TriageMessage,
    senders: &SenderRules,
    code_window: Duration,
) -> Option<Proposal> {
    authentication_failure(message)
        .or_else(|| expiring_code(message, code_window))
        .or_else(|| senders.lookup(message))
        .or_else(|| debt_collection(message))
}

/// The disclosure US law requires on a debt collector's messages (FDCPA
/// section 807(11), Regulation F). Mortgage servicers' statements say "an
/// attempt to collect a debt" instead, and those are Accounting, not Credit.
const DEBT_COLLECTOR_DISCLOSURE: &str = "this communication is from a debt collector";

/// Whether a body carries the debt-collector disclosure (case and line
/// breaks ignored). Checked on the whole body: collectors put it at the end.
pub fn carries_debt_collector_disclosure(body: &str) -> bool {
    body.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .contains(DEBT_COLLECTOR_DISCLOSURE)
}

fn debt_collection(message: &TriageMessage) -> Option<Proposal> {
    message
        .debt_collector_notice
        .then(|| Proposal::from_rule(Bucket::Credit, "debt collector disclosure"))
}

/// Bulk mail is Noise unless the model says it is accounting, legal, credit or
/// an alert.
pub fn bulk_mail_default(message: &TriageMessage, model: Proposal) -> Proposal {
    match bulk_mail(message) {
        Some(noise)
            if !matches!(
                model.bucket,
                Bucket::Accounting | Bucket::Legal | Bucket::Credit | Bucket::Alerts
            ) =>
        {
            noise
        }
        _ => model,
    }
}

/// Uses only the topmost Authentication-Results header: the one added by our
/// own receiving server. Lower ones can be forged by the sender.
fn authentication_failure(message: &TriageMessage) -> Option<Proposal> {
    let results = message
        .header("authentication-results")?
        .to_ascii_lowercase();
    let reason = if results.contains("dmarc=fail") && !DMARC_POLICY_NONE.is_match(&results) {
        "DMARC fail in Authentication-Results"
    } else if results.contains("spf=fail") && !results.contains("dkim=pass") {
        "SPF fail without a DKIM pass in Authentication-Results"
    } else if results.contains("dkim=fail")
        && !results.contains("dkim=pass")
        && !results.contains("spf=pass")
    {
        "DKIM fail without an SPF or DKIM pass in Authentication-Results"
    } else {
        return None;
    };
    Some(Proposal::from_rule(Bucket::Spam, reason))
}

fn bulk_mail(message: &TriageMessage) -> Option<Proposal> {
    if message.has_header("list-unsubscribe") {
        return Some(Proposal::from_rule(
            Bucket::Noise,
            "List-Unsubscribe header",
        ));
    }
    let precedence = message.header("precedence")?.trim().to_ascii_lowercase();
    match precedence.as_str() {
        "bulk" | "list" | "junk" => Some(Proposal::from_rule(
            Bucket::Noise,
            format!("Precedence: {}", precedence),
        )),
        _ => None,
    }
}
