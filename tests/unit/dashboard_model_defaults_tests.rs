// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! One configured local model: it survives startup, unset roles use it, and
//! the Ollama keep-alive is sent only when configured.

use rustymail::dashboard::services::ai::model_config::default_model_config;
use rustymail::dashboard::services::ai::provider::ollama::{add_keep_alive, keep_alive_setting};
use rustymail::dashboard::services::ai::provider_manager::startup_model;
use serde_json::json;

#[test]
fn a_configured_model_is_kept_at_startup() {
    let listed = vec!["newest-pull:latest".to_string(), "gemma4:26b".to_string()];
    // Configured, listed or not: kept (a proxy may not list every model it serves).
    assert_eq!(startup_model("gemma4:26b", &listed), None);
    assert_eq!(startup_model("ms/gemma4-26b-a4b-it-q4_k_m", &listed), None);
    // Nothing configured: the server's first model.
    assert_eq!(
        startup_model("", &listed),
        Some("newest-pull:latest".to_string())
    );
    assert_eq!(startup_model("", &[]), None);
}

#[test]
fn unset_roles_use_the_configured_ollama_model() {
    let config = default_model_config("tool_calling", None, Some(" gemma4:26b ".into())).unwrap();
    assert_eq!(config.role, "tool_calling");
    assert_eq!(config.provider, "ollama");
    assert_eq!(config.model_name, "gemma4:26b");
    assert_eq!(config.base_url, None, "uses OLLAMA_BASE_URL");
    assert!(default_model_config("drafting", None, None).is_err());
    assert!(default_model_config("drafting", None, Some("  ".into())).is_err());
}

#[test]
fn keep_alive_is_sent_only_when_configured() {
    let body = json!({"model": "m", "stream": false});
    assert_eq!(
        keep_alive_setting(Some(" 30m ".into())),
        Some("30m".to_string())
    );
    assert_eq!(keep_alive_setting(Some("".into())), None);
    assert_eq!(keep_alive_setting(None), None);
    let with = add_keep_alive(body.clone(), Some("30m".into()));
    assert_eq!(with["keep_alive"], "30m");
    assert_eq!(add_keep_alive(body.clone(), None), body);
}

#[test]
fn ollama_chat_menus_list_only_models_that_can_chat() {
    use rustymail::dashboard::services::ai::provider::can_chat;
    use rustymail::dashboard::services::ai::provider::ollama::chat_model_names;
    // The shape of Ollama 0.40's /api/tags.
    let tags = json!({"models": [
        {"name": "gemma4:26b-a4b-it-q4_K_M", "model": "gemma4:26b-a4b-it-q4_K_M",
         "capabilities": ["completion", "vision", "tools", "thinking"]},
        {"name": "autotrust-jev:9b", "model": "autotrust-jev:9b", "capabilities": ["decision"]},
        {"name": "older-ollama:latest", "model": "older-ollama:latest"},
    ]});
    assert_eq!(
        chat_model_names(&tags).unwrap(),
        vec!["gemma4:26b-a4b-it-q4_K_M", "older-ollama:latest"]
    );
    assert!(can_chat(None), "no capabilities reported: assume it chats");
    assert!(!can_chat(Some(&["embedding".to_string()])));
}
