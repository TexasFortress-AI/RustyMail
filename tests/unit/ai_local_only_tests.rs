// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Email content may only reach models on this machine.

use rustymail::dashboard::services::ai::local_only::{
    ensure_local, ensure_local_type, is_loopback_url,
};
use rustymail::dashboard::services::ai::model_config::{
    get_model_config, set_model_config, ModelConfiguration,
};
use rustymail::dashboard::services::ai::provider_manager::{
    ProviderConfig, ProviderManager, ProviderType,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

const CLOUD_TYPES: [ProviderType; 10] = [
    ProviderType::OpenAI,
    ProviderType::OpenRouter,
    ProviderType::Morpheus,
    ProviderType::Anthropic,
    ProviderType::DeepSeek,
    ProviderType::XAI,
    ProviderType::Gemini,
    ProviderType::Mistral,
    ProviderType::Together,
    ProviderType::Azure,
];

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

fn model_config(provider: &str, base_url: &str) -> ModelConfiguration {
    ModelConfiguration {
        role: "drafting".to_string(),
        provider: provider.to_string(),
        model_name: "some-model".to_string(),
        base_url: Some(base_url.to_string()),
        api_key: None,
        additional_config: None,
    }
}

#[test]
fn loopback_urls_only() {
    for url in [
        "http://127.0.0.1:11434",
        "http://localhost:5846/v1",
        "http://[::1]:1234",
        "http://127.8.9.1",
    ] {
        assert!(is_loopback_url(url), "{}", url);
    }
    for url in [
        "https://api.openai.com/v1",
        "https://api.mor.org/api/v1",
        "http://192.168.1.20:11434",
        "http://localhost.attacker.example",
        "not a url",
    ] {
        assert!(!is_loopback_url(url), "{}", url);
    }
}

#[test]
fn cloud_providers_are_refused() {
    for name in [
        "openai",
        "anthropic",
        "openrouter",
        "morpheus",
        "gemini",
        "deepseek",
    ] {
        assert!(
            ensure_local(name, Some("http://127.0.0.1:1")).is_err(),
            "{}",
            name
        );
    }
    for provider_type in CLOUD_TYPES {
        assert!(
            ensure_local_type(&provider_type).is_err(),
            "{:?}",
            provider_type
        );
    }
    assert!(ensure_local_type(&ProviderType::Mock).is_ok());
}

#[test]
fn local_providers_must_point_at_this_machine() {
    assert!(ensure_local("ollama", Some("http://127.0.0.1:11434")).is_ok());
    assert!(ensure_local("lmstudio", Some("http://localhost:5846")).is_ok());
    let remote = ensure_local("ollama", Some("http://10.0.0.5:11434")).unwrap_err();
    assert!(remote.contains("10.0.0.5"), "{}", remote);
}

#[tokio::test]
async fn provider_manager_will_not_register_cloud_providers() {
    let mut manager = ProviderManager::new();
    for provider_type in CLOUD_TYPES {
        let result = manager
            .add_provider(ProviderConfig {
                name: format!("{:?}", provider_type).to_lowercase(),
                provider_type: provider_type.clone(),
                api_key: Some("sk-test".to_string()),
                model: "cloud-model".to_string(),
                max_tokens: None,
                temperature: None,
                priority: 1,
                enabled: true,
            })
            .await;
        assert!(result.is_err(), "{:?} was registered", provider_type);
    }
    assert!(manager.list_providers().await.is_empty());
}

#[tokio::test]
async fn drafting_and_tool_models_cannot_be_cloud() {
    let pool = pool().await;
    assert!(
        set_model_config(&pool, &model_config("openai", "https://api.openai.com/v1"))
            .await
            .is_err()
    );
    assert!(
        set_model_config(&pool, &model_config("ollama", "https://ollama.example.com"))
            .await
            .is_err()
    );
    set_model_config(&pool, &model_config("ollama", "http://127.0.0.1:11434"))
        .await
        .unwrap();
    assert_eq!(
        get_model_config(&pool, "drafting").await.unwrap().provider,
        "ollama"
    );
}

#[tokio::test]
async fn cloud_config_saved_before_the_rule_is_not_used() {
    let pool = pool().await;
    sqlx::query(
        "INSERT INTO ai_model_configurations (role, provider, model_name, base_url)
         VALUES ('tool_calling', 'openai', 'gpt-4o', 'https://api.openai.com/v1')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(get_model_config(&pool, "tool_calling").await.is_err());
}
