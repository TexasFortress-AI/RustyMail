// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Triage settings from environment variables (see .env.example, TRIAGE_*).

use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_FOLDER: &str = "INBOX";
const DEFAULT_ESCALATE_BELOW: f64 = 0.7;
const DEFAULT_MODEL_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_FETCH_BATCH: usize = 20;
const DEFAULT_BODY_EXCERPT_CHARS: usize = 1500;

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
    pub model_timeout: Duration,
    pub fetch_batch: usize,
    pub excerpt_chars: usize,
    pub sender_rules_file: Option<PathBuf>,
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
        let dashboard_url = match value("TRIAGE_DASHBOARD_URL") {
            Some(url) => url,
            None => {
                let port = value("DASHBOARD_PORT")
                    .ok_or("set TRIAGE_DASHBOARD_URL or DASHBOARD_PORT for report links")?;
                format!("http://127.0.0.1:{}", port)
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
            dashboard_url: dashboard_url.trim_end_matches('/').to_string(),
        })
    }

    /// Model URL and small model name; required to classify, not to report.
    pub fn model_settings(&self) -> Result<(&str, &str), String> {
        match (&self.model_url, &self.small_model) {
            (Some(url), Some(model)) => Ok((url, model)),
            _ => Err("set TRIAGE_MODEL_URL and TRIAGE_MODEL to classify mail".to_string()),
        }
    }
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
