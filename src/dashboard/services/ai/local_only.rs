// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Email content is processed only by models running on this machine.
//!
//! Prompts sent to a model include email content, so every path that can send
//! one (chat, drafting, tool calling) checks here first. Cloud providers are
//! refused, and local providers must point at a loopback address.

use super::provider_manager::ProviderType;
use url::{Host, Url};

/// Providers that run on this machine. `mock` makes no network calls.
pub const LOCAL_PROVIDERS: &[&str] = &["ollama", "llamacpp", "lmstudio", "mock"];

/// Environment variable holding a local provider's base URL.
pub fn base_url_env_var(provider: &str) -> Option<&'static str> {
    match provider {
        "ollama" => Some("OLLAMA_BASE_URL"),
        "llamacpp" => Some("LLAMACPP_BASE_URL"),
        "lmstudio" => Some("LMSTUDIO_BASE_URL"),
        _ => None,
    }
}

/// Provider name for a local provider type; None for cloud types.
pub fn provider_key(provider_type: &ProviderType) -> Option<&'static str> {
    match provider_type {
        ProviderType::Ollama => Some("ollama"),
        ProviderType::LlamaCpp => Some("llamacpp"),
        ProviderType::LmStudio => Some("lmstudio"),
        ProviderType::Mock => Some("mock"),
        _ => None,
    }
}

/// True for localhost, 127.0.0.0/8 and ::1.
pub fn is_loopback_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    match parsed.host() {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Allow `provider` only if it is local and its base URL is on this machine.
/// `configured_base_url` takes precedence over the provider's env variable.
pub fn ensure_local(provider: &str, configured_base_url: Option<&str>) -> Result<(), String> {
    if !LOCAL_PROVIDERS.contains(&provider) {
        return Err(format!(
            "AI provider '{}' is not allowed: email is only processed by local models ({})",
            provider,
            LOCAL_PROVIDERS.join(", ")
        ));
    }
    let Some(var) = base_url_env_var(provider) else {
        return Ok(());
    };
    let base_url = configured_base_url
        .map(str::to_string)
        .or_else(|| std::env::var(var).ok());
    match base_url {
        Some(url) if is_loopback_url(&url) => Ok(()),
        Some(url) => Err(format!(
            "{} base URL {} is not on this machine; email is only processed by local models",
            provider, url
        )),
        None => Err(format!("{} needs {} set to a local address", provider, var)),
    }
}

/// `ensure_local` for a provider type; cloud types are always refused.
pub fn ensure_local_type(provider_type: &ProviderType) -> Result<(), String> {
    match provider_key(provider_type) {
        Some(provider) => ensure_local(provider, None),
        None => Err(format!(
            "AI provider type {:?} is not allowed: email is only processed by local models",
            provider_type
        )),
    }
}
