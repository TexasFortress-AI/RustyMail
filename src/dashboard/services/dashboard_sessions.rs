// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Browser sessions for the dashboard.
//!
//! The browser never holds the API key. It presents the key once to log in
//! and gets back a random session token in an HttpOnly cookie. Only a
//! SHA-256 digest of each token is kept, in memory, so a server restart
//! ends every session.

use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Environment variable: how long a dashboard login lasts, in hours.
pub const SESSION_TTL_HOURS_ENV: &str = "DASHBOARD_SESSION_TTL_HOURS";
const DEFAULT_SESSION_TTL_HOURS: u64 = 12;
const SECONDS_PER_HOUR: u64 = 3600;
const TOKEN_BYTES: usize = 32;

type TokenDigest = [u8; 32];

/// The set of live dashboard sessions, shared by every server worker.
#[derive(Debug)]
pub struct DashboardSessions {
    ttl: Duration,
    expiry_by_digest: Mutex<HashMap<TokenDigest, Instant>>,
}

impl DashboardSessions {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            expiry_by_digest: Mutex::new(HashMap::new()),
        }
    }

    /// Uses `DASHBOARD_SESSION_TTL_HOURS`, or 12 hours when unset or invalid.
    pub fn from_env() -> Self {
        let hours = std::env::var(SESSION_TTL_HOURS_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|hours| *hours > 0)
            .unwrap_or(DEFAULT_SESSION_TTL_HOURS);
        Self::new(Duration::from_secs(hours * SECONDS_PER_HOUR))
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Starts a session and returns its token (64 hex characters).
    pub fn create(&self) -> String {
        let mut bytes = [0u8; TOKEN_BYTES];
        OsRng.fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        let now = Instant::now();
        let mut sessions = self.expiry_by_digest.lock().unwrap();
        sessions.retain(|_, expiry| *expiry > now);
        sessions.insert(digest(&token), now + self.ttl);
        token
    }

    /// True while the token's session exists and has not expired.
    pub fn is_valid(&self, token: &str) -> bool {
        let key = digest(token);
        let mut sessions = self.expiry_by_digest.lock().unwrap();
        match sessions.get(&key) {
            Some(expiry) if *expiry > Instant::now() => true,
            Some(_) => {
                sessions.remove(&key);
                false
            }
            None => false,
        }
    }

    /// Ends the token's session (logout). Unknown tokens are ignored.
    pub fn revoke(&self, token: &str) {
        self.expiry_by_digest.lock().unwrap().remove(&digest(token));
    }
}

fn digest(token: &str) -> TokenDigest {
    Sha256::digest(token.as_bytes()).into()
}
