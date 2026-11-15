// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage settings from environment variables (see .env.example, TRIAGE_*).

use super::Bucket;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_FOLDER: &str = "INBOX";
const DEFAULT_ESCALATE_BELOW: f64 = 0.7;
const DEFAULT_MODEL_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_FETCH_BATCH: usize = 20;
/// A short snippet is enough to classify; full bodies never go to the model.
const DEFAULT_BODY_EXCERPT_CHARS: usize = 500;
/// How long a code or reset that states no expiry stays useful.
const DEFAULT_CODE_WINDOW_HOURS: i64 = 24;
/// Model proposals below this confidence stay in the INBOX on apply.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.8;
/// Model Needs Reply proposals only for mail newer than this; older stays put.
const DEFAULT_NEEDS_REPLY_DAYS: i64 = 30;
/// Buckets apply moves by default. Needs Reply is mail from real people, which
/// stays in the INBOX unless TRIAGE_APPLY_BUCKETS says otherwise.
const DEFAULT_APPLY_BUCKETS: [Bucket; 5] = [
    Bucket::Spam,
    Bucket::Noise,
    Bucket::Receipts,
    Bucket::Alerts,
    Bucket::Trash,
];

#[derive(Debug, Clone)]
pub struct TriageConfig {
    /// Accounts to triage when none are given on the command line.
    pub accounts: Vec<String>,
    pub folder: String,
    /// OpenAI-compatible API root of the local model server.
    pub model_url: Option<String>,
    pub small_model: Option<String>,
    pub large_model: Option<String>,
    pub escalate_below: f64,
    /// Decision-model server root (Ollama); with `decision_model` it replaces the chat model.
    pub decision_url: Option<String>,
    pub decision_model: Option<String>,
    pub needs_reply_max_age: chrono::Duration,
    pub model_timeout: Duration,
    pub fetch_batch: usize,
    pub excerpt_chars: usize,
    pub sender_rules_file: Option<PathBuf>,
    /// Per-account bucket-to-folder overrides.
    pub folder_map_file: Option<PathBuf>,
    pub code_window: chrono::Duration,
    pub min_confidence: f64,
    /// Buckets whose proposals apply moves; `Keep` is never moved.
    pub apply_buckets: Vec<Bucket>,
    /// Base URL for dashboard deep links in the report.
    pub dashboard_url: String,
}

impl TriageConfig {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Build from any variable lookup; blank values count as unset.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let value = |name: &str| {
            get(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        // The backend serves the dashboard on its own address.
        let dashboard_url = match value("TRIAGE_DASHBOARD_URL") {
            Some(url) => url,
            None => {
                let host = value("REST_HOST")
                    .ok_or("set TRIAGE_DASHBOARD_URL or REST_HOST for report links")?;
                let port = value("REST_PORT")
                    .ok_or("set TRIAGE_DASHBOARD_URL or REST_PORT for report links")?;
                format!("http://{}:{}", host, port)
            }
        };
        Ok(Self {
            accounts: value("TRIAGE_ACCOUNTS")
                .map(|list| {
                    list.split(',')
                        .map(|a| a.trim().to_ascii_lowercase())
                        .filter(|a| !a.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            folder: value("TRIAGE_FOLDER").unwrap_or_else(|| DEFAULT_FOLDER.to_string()),
            model_url: value("TRIAGE_MODEL_URL"),
            small_model: value("TRIAGE_MODEL"),
            large_model: value("TRIAGE_MODEL_LARGE"),
            escalate_below: parse_or(
                value("TRIAGE_ESCALATE_BELOW"),
                "TRIAGE_ESCALATE_BELOW",
                DEFAULT_ESCALATE_BELOW,
            )?,
            decision_url: value("TRIAGE_DECISION_URL"),
            decision_model: value("TRIAGE_DECISION_MODEL"),
            needs_reply_max_age: chrono::Duration::days(parse_or(
                value("TRIAGE_NEEDS_REPLY_DAYS"),
                "TRIAGE_NEEDS_REPLY_DAYS",
                DEFAULT_NEEDS_REPLY_DAYS,
            )?),
            model_timeout: Duration::from_secs(parse_or(
                value("TRIAGE_MODEL_TIMEOUT_SECONDS"),
                "TRIAGE_MODEL_TIMEOUT_SECONDS",
                DEFAULT_MODEL_TIMEOUT_SECONDS,
            )?),
            fetch_batch: parse_or(
                value("TRIAGE_FETCH_BATCH"),
                "TRIAGE_FETCH_BATCH",
                DEFAULT_FETCH_BATCH,
            )?
            .max(1),
            excerpt_chars: parse_or(
                value("TRIAGE_BODY_EXCERPT_CHARS"),
                "TRIAGE_BODY_EXCERPT_CHARS",
                DEFAULT_BODY_EXCERPT_CHARS,
            )?,
            sender_rules_file: value("TRIAGE_SENDER_RULES_FILE").map(PathBuf::from),
            folder_map_file: value("TRIAGE_FOLDER_MAP_FILE").map(PathBuf::from),
            code_window: chrono::Duration::hours(parse_or(
                value("TRIAGE_CODE_WINDOW_HOURS"),
                "TRIAGE_CODE_WINDOW_HOURS",
                DEFAULT_CODE_WINDOW_HOURS,
            )?),
            min_confidence: parse_or(
                value("TRIAGE_MIN_CONFIDENCE"),
                "TRIAGE_MIN_CONFIDENCE",
                DEFAULT_MIN_CONFIDENCE,
            )?,
            apply_buckets: match value("TRIAGE_APPLY_BUCKETS") {
                Some(list) => parse_buckets(&list)?,
                None => DEFAULT_APPLY_BUCKETS.to_vec(),
            },
            dashboard_url: dashboard_url.trim_end_matches('/').to_string(),
        })
    }

    /// Decision-model URL and name, when both are set.
    pub fn decision_settings(&self) -> Option<(&str, &str)> {
        match (&self.decision_url, &self.decision_model) {
            (Some(url), Some(model)) => Some((url, model)),
            _ => None,
        }
    }

    /// Model URL and small model name; required to classify, not to report.
    pub fn model_settings(&self) -> Result<(&str, &str), String> {
        match (&self.model_url, &self.small_model) {
            (Some(url), Some(model)) => Ok((url, model)),
            _ => Err("set TRIAGE_MODEL_URL and TRIAGE_MODEL to classify mail".to_string()),
        }
    }
}

/// Comma-separated bucket names; `Keep` is refused because it never moves.
fn parse_buckets(list: &str) -> Result<Vec<Bucket>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            Bucket::parse(name)
                .filter(|b| *b != Bucket::Keep)
                .ok_or_else(|| format!("TRIAGE_APPLY_BUCKETS has an invalid bucket: {}", name))
        })
        .collect()
}

fn parse_or<T: std::str::FromStr>(
    raw: Option<String>,
    name: &str,
    default: T,
) -> Result<T, String> {
    match raw {
        Some(text) => text
            .parse()
            .map_err(|_| format!("{} has an invalid value: {}", name, text)),
        None => Ok(default),
    }
}
