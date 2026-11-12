// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Where the server's API key comes from.
//!
//! `RUSTYMAIL_API_KEY` from the environment wins (CI, Linux, explicit
//! launchers). When it is unset, the server reads the macOS Keychain item
//! `apikey:RUSTYMAIL_API_KEY` (the convention `mcp-keychain-run` uses), so
//! the key need not be written to `.env`. The value is placed in this
//! process's environment only; it is never logged.

use super::api_key::API_KEY_ENV;
use log::{info, warn};

/// Overrides the Keychain service name (default `apikey:RUSTYMAIL_API_KEY`).
pub const KEYCHAIN_SERVICE_ENV: &str = "RUSTYMAIL_API_KEY_KEYCHAIN_SERVICE";
#[cfg(target_os = "macos")]
const DEFAULT_KEYCHAIN_SERVICE: &str = "apikey:RUSTYMAIL_API_KEY";
#[cfg(target_os = "macos")]
const SECURITY_TOOL: &str = "/usr/bin/security";

/// Where the key was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    Keychain,
    Missing,
}

/// The environment value if it is set and non-empty, otherwise the Keychain's.
pub fn resolve_api_key(
    environment: Option<String>,
    read_keychain: impl FnOnce() -> Option<String>,
) -> (Option<String>, ApiKeySource) {
    if let Some(value) = environment.filter(|value| !value.trim().is_empty()) {
        return (Some(value), ApiKeySource::Environment);
    }
    match read_keychain().filter(|value| !value.trim().is_empty()) {
        Some(value) => (Some(value), ApiKeySource::Keychain),
        None => (None, ApiKeySource::Missing),
    }
}

/// Reads the key from the login Keychain with `/usr/bin/security`.
#[cfg(target_os = "macos")]
fn read_keychain() -> Option<String> {
    let service = std::env::var(KEYCHAIN_SERVICE_ENV)
        .unwrap_or_else(|_| DEFAULT_KEYCHAIN_SERVICE.to_string());
    let account = std::env::var("USER").ok()?;
    let output = std::process::Command::new(SECURITY_TOOL)
        .args([
            "find-generic-password",
            "-a",
            &account,
            "-s",
            &service,
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        warn!("No Keychain item '{}' for account '{}'", service, account);
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim_end_matches('\n').to_string())
}

#[cfg(not(target_os = "macos"))]
fn read_keychain() -> Option<String> {
    None
}

/// Call once at startup, before any thread reads the environment: fills in
/// `RUSTYMAIL_API_KEY` from the Keychain when the environment lacks it.
pub fn load_api_key_into_environment() -> ApiKeySource {
    let (value, source) = resolve_api_key(std::env::var(API_KEY_ENV).ok(), read_keychain);
    match (value, source) {
        (Some(value), ApiKeySource::Keychain) => {
            std::env::set_var(API_KEY_ENV, value);
            info!("API key loaded from the macOS Keychain");
        }
        (_, ApiKeySource::Environment) => info!("API key taken from the environment"),
        _ => warn!(
            "No API key: set {} or add the Keychain item; every authenticated request will be refused",
            API_KEY_ENV
        ),
    }
    source
}
