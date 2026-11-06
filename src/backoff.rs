// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Exponential backoff, so an unreachable mail host is retried less often and logs less.

use std::time::{Duration, Instant};

const DEFAULT_BASE_SECONDS: u64 = 300;
const DEFAULT_MAX_SECONDS: u64 = 3600;

/// Doubles the wait after each consecutive failure, up to a maximum.
#[derive(Debug, Clone, Copy)]
pub struct BackoffPolicy {
    base_seconds: u64,
    max_seconds: u64,
}

impl BackoffPolicy {
    pub fn new(base_seconds: u64, max_seconds: u64) -> Self {
        Self {
            base_seconds,
            max_seconds,
        }
    }

    /// Reads `FAILURE_BACKOFF_BASE_SECONDS` and `FAILURE_BACKOFF_MAX_SECONDS`.
    pub fn from_env() -> Self {
        let read = |name: &str, default: u64| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        };
        Self::new(
            read("FAILURE_BACKOFF_BASE_SECONDS", DEFAULT_BASE_SECONDS),
            read("FAILURE_BACKOFF_MAX_SECONDS", DEFAULT_MAX_SECONDS),
        )
    }

    pub fn delay_seconds(&self, consecutive_failures: u32) -> u64 {
        let doublings = consecutive_failures.saturating_sub(1).min(u64::BITS - 1);
        self.base_seconds
            .saturating_mul(1u64 << doublings)
            .min(self.max_seconds)
    }
}

/// In-memory backoff state for a long-running loop.
#[derive(Debug)]
pub struct FailureBackoff {
    policy: BackoffPolicy,
    consecutive_failures: u32,
    next_attempt: Option<Instant>,
}

impl FailureBackoff {
    pub fn new(policy: BackoffPolicy) -> Self {
        Self {
            policy,
            consecutive_failures: 0,
            next_attempt: None,
        }
    }

    pub fn is_due(&self, now: Instant) -> bool {
        self.next_attempt.is_none_or(|next| now >= next)
    }

    pub fn record_failure(&mut self, now: Instant) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let delay = self.policy.delay_seconds(self.consecutive_failures);
        self.next_attempt = Some(now + Duration::from_secs(delay));
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.next_attempt = None;
    }
}
