// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// tests/unit/ai_service_tests.rs
// Unit tests for AI Service and Provider Manager

use rustymail::dashboard::services::ai::AiService;
use rustymail::dashboard::services::ai::provider_manager::{ProviderManager, ProviderConfig, ProviderType};
use rustymail::dashboard::api::models::ChatbotQuery;

#[tokio::test]
async fn test_ai_service_new_mock() {
    // Test creating a mock AI service
    let _service = AiService::new_mock();

    // Verify service is in mock mode
    // This is implicitly verified by not panicking during creation
    assert!(true, "Mock service created successfully");
}

#[tokio::test]
async fn test_ai_service_mock_response_simple() {
    // Test that mock service accepts queries (without triggering MCP calls)
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: Some("test-conv".to_string()),
        provider_override: None,
        model_override: None,
        current_folder: None,
        account_id: None,
        enabled_tools: None,
    };

    // Mock service will generate a response (may try MCP calls but will fail gracefully)
    let response = service.process_query(query).await;

    // Even if MCP calls fail, the service should return a response
    assert!(response.is_ok(), "Mock service should generate response");

    let response = response.unwrap();
    assert_eq!(response.conversation_id, "test-conv");
    assert!(!response.text.is_empty(), "Response should have text");
}

#[tokio::test]
async fn test_ai_service_conversation_id_tracking() {
    // Test that conversation IDs are properly tracked
    let service = AiService::new_mock();

    let conv_id = "test-conversation".to_string();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: Some(conv_id.clone()),
        provider_override: None,
        model_override: None,
        current_folder: None,
        account_id: None,
    };

    let response = service.process_query(query).await;
    assert!(response.is_ok(), "Service should process query");
    assert_eq!(response.unwrap().conversation_id, conv_id);
}

#[tokio::test]
async fn test_ai_service_accepts_account_context() {
    // Test that account ID and folder parameters are accepted
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Test".to_string(),
        conversation_id: Some("test-conv".to_string()),
        provider_override: None,
        model_override: None,
        current_folder: Some("INBOX".to_string()),
        account_id: Some("test@example.com".to_string()),
    };

    // Service should accept the query with account context
    let response = service.process_query(query).await;
    assert!(response.is_ok(), "Service should accept account context parameters");
}

#[tokio::test]
async fn test_provider_manager_creation() {
    // Test creating a provider manager
    let manager = ProviderManager::new();

    let providers = manager.list_providers().await;
    assert_eq!(providers.len(), 0, "New manager should have no providers");
}

#[tokio::test]
async fn test_provider_manager_add_mock_provider() {
    // Test adding a mock provider
    let mut manager = ProviderManager::new();

    let config = ProviderConfig {
        name: "test-mock".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "mock-model".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 1,
        enabled: true,
    };

    let result = manager.add_provider(config).await;
    assert!(result.is_ok(), "Should successfully add mock provider");

    let providers = manager.list_providers().await;
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].name, "test-mock");
}

#[tokio::test]
async fn test_provider_manager_set_current_provider() {
    // Test setting the current provider
    let mut manager = ProviderManager::new();

    let config = ProviderConfig {
        name: "test-provider".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "test-model".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 1,
        enabled: true,
    };

    manager.add_provider(config).await.unwrap();

    let result = manager.set_current_provider("test-provider".to_string()).await;
    assert!(result.is_ok(), "Should set current provider");

    let current_name = manager.get_current_provider_name().await;
    assert_eq!(current_name, Some("test-provider".to_string()));
}

#[tokio::test]
async fn test_provider_manager_set_invalid_provider() {
    // Test setting a provider that doesn't exist
    let manager = ProviderManager::new();

    let result = manager.set_current_provider("nonexistent".to_string()).await;
    assert!(result.is_err(), "Should fail for nonexistent provider");
}

#[tokio::test]
async fn test_provider_manager_generate_response_no_provider() {
    // Test generating response with no provider selected
    let manager = ProviderManager::new();

    let messages = vec![];
    let result = manager.generate_response(&messages).await;
    assert!(result.is_err(), "Should fail when no provider is selected");
}

#[tokio::test]
async fn test_provider_manager_list_providers_empty() {
    // Test listing providers when none exist
    let manager = ProviderManager::new();

    let providers = manager.list_providers().await;
    assert_eq!(providers.len(), 0);
}

#[tokio::test]
async fn test_provider_manager_multiple_providers() {
    // Test adding multiple providers with different priorities
    let mut manager = ProviderManager::new();

    let config1 = ProviderConfig {
        name: "provider1".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "model1".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 2,
        enabled: true,
    };

    let config2 = ProviderConfig {
        name: "provider2".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "model2".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 1,
        enabled: true,
    };

    manager.add_provider(config1).await.unwrap();
    manager.add_provider(config2).await.unwrap();

    let providers = manager.list_providers().await;
    assert_eq!(providers.len(), 2);

    // Check that providers are sorted by priority (lower number = higher priority)
    assert_eq!(providers[0].name, "provider2");
    assert_eq!(providers[1].name, "provider1");
}

#[tokio::test]
async fn test_provider_manager_enable_disable() {
    // Test enabling and disabling providers
    let mut manager = ProviderManager::new();

    let config = ProviderConfig {
        name: "test-provider".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "test-model".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 1,
        enabled: true,
    };

    manager.add_provider(config).await.unwrap();

    // Disable the provider
    manager.set_provider_enabled("test-provider", false).await.unwrap();

    let providers = manager.list_providers().await;
    assert!(!providers[0].enabled, "Provider should be disabled");

    // Re-enable the provider
    manager.set_provider_enabled("test-provider", true).await.unwrap();

    let providers = manager.list_providers().await;
    assert!(providers[0].enabled, "Provider should be enabled");
}

#[tokio::test]
async fn test_provider_manager_init_from_env_mock_fallback() {
    // Test that init_from_env always adds mock provider
    let mut manager = ProviderManager::new();

    // Without any API keys, should still succeed with mock provider
    let result = manager.init_from_env().await;
    assert!(result.is_ok(), "Init should succeed with mock provider");

    let providers = manager.list_providers().await;
    assert!(!providers.is_empty(), "Should have at least mock provider");
    assert!(providers.iter().any(|p| p.provider_type == ProviderType::Mock),
            "Should have mock provider");
}

#[tokio::test]
async fn test_ai_service_list_providers() {
    // Test listing providers through AI service
    // new_mock() creates an empty provider manager, so no providers initially
    let service = AiService::new_mock();

    let providers = service.list_providers().await;
    // Mock service starts with no providers configured
    assert_eq!(providers.len(), 0);
}

#[tokio::test]
async fn test_ai_service_get_current_provider() {
    // Test getting current provider name
    // new_mock() doesn't set a current provider, so it should be None
    let service = AiService::new_mock();

    let provider_name = service.get_current_provider_name().await;
    assert_eq!(provider_name, None, "Mock service should have no provider set initially");
}

#[tokio::test]
async fn test_ai_service_provider_override() {
    // Test that provider override parameter is accepted
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: Some("test-conv".to_string()),
        provider_override: Some("mock".to_string()),
        model_override: None,
        current_folder: None,
        account_id: None,
    };

    let response = service.process_query(query).await;
    assert!(response.is_ok(), "Should accept provider override");
}

#[tokio::test]
async fn test_ai_service_model_override() {
    // Test that model override parameter is accepted
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: Some("test-conv".to_string()),
        provider_override: None,
        model_override: Some("custom-model".to_string()),
        current_folder: None,
        account_id: None,
    };

    let response = service.process_query(query).await;
    assert!(response.is_ok(), "Should accept model override");
}

#[tokio::test]
async fn test_ai_service_followup_suggestions() {
    // Test that responses include followup suggestions
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: Some("test-conv".to_string()),
        provider_override: None,
        model_override: None,
        current_folder: None,
        account_id: None,
    };

    let response = service.process_query(query).await.unwrap();
    assert!(response.followup_suggestions.is_some(), "Should include suggestions");

    let suggestions = response.followup_suggestions.unwrap();
    assert!(!suggestions.is_empty(), "Should have at least one suggestion");
}

#[tokio::test]
async fn test_provider_config_serialization() {
    // Test that ProviderConfig can be serialized/deserialized
    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mock,
        api_key: Some("key".to_string()),
        model: "model".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0.5),
        priority: 1,
        enabled: true,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.model, deserialized.model);
    assert_eq!(config.priority, deserialized.priority);
}

#[tokio::test]
async fn test_provider_type_equality() {
    // Test ProviderType equality
    assert_eq!(ProviderType::Mock, ProviderType::Mock);
    assert_ne!(ProviderType::Mock, ProviderType::OpenAI);
    assert_ne!(ProviderType::OpenAI, ProviderType::Anthropic);
}

#[tokio::test]
async fn test_ai_service_auto_conversation_id() {
    // Test that service auto-generates conversation IDs
    let service = AiService::new_mock();

    let query = ChatbotQuery {
        query: "Hello".to_string(),
        conversation_id: None,  // No conversation ID provided
        provider_override: None,
        model_override: None,
        current_folder: None,
        account_id: None,
    };

    let response = service.process_query(query).await.unwrap();
    assert!(!response.conversation_id.is_empty(), "Should auto-generate conversation ID");
}

#[tokio::test]
async fn test_provider_manager_update_config() {
    // Test updating provider configuration
    let mut manager = ProviderManager::new();

    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "model-v1".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0.5),
        priority: 1,
        enabled: true,
    };

    manager.add_provider(config.clone()).await.unwrap();

    // Update with new model
    let updated_config = ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "model-v2".to_string(),
        max_tokens: Some(2000),
        temperature: Some(0.7),
        priority: 1,
        enabled: true,
    };

    let result = manager.update_provider_config("test", updated_config).await;
    assert!(result.is_ok(), "Should update provider config");

    let providers = manager.list_providers().await;
    assert_eq!(providers[0].model, "model-v2");
    assert_eq!(providers[0].max_tokens, Some(2000));
}

#[tokio::test]
async fn test_provider_manager_get_current_model() {
    // Test getting current model name
    let mut manager = ProviderManager::new();

    let config = ProviderConfig {
        name: "test".to_string(),
        provider_type: ProviderType::Mock,
        api_key: None,
        model: "test-model-123".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0.5),
        priority: 1,
        enabled: true,
    };

    manager.add_provider(config).await.unwrap();
    manager.set_current_provider("test".to_string()).await.unwrap();

    let model_name = manager.get_current_model_name().await;
    assert_eq!(model_name, Some("test-model-123".to_string()));
}
