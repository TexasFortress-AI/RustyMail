# Task ID: 42

**Title:** Add integration tests for AI subsystem configuration

**Status:** done

**Dependencies:** None

**Priority:** medium

**Description:** Create comprehensive integration tests that verify sampler configuration persistence, provider functionality, and model selection across service restarts to prevent configuration-related bugs.

**Details:**

Create integration tests in tests/ai_configuration_integration.rs that verify the entire AI configuration subsystem works correctly:

1. **Test Sampler Settings Application**:
   ```rust
   #[tokio::test]
   async fn test_sampler_config_applied_to_requests() {
       // Setup test database with custom sampler config
       let sampler_config = SamplerConfig {
           provider: "ollama",
           model_name: "llama3.3:70b",
           temperature: 0.3,
           top_p: 0.95,
           min_p: 0.05,
           repeat_penalty: 1.2,
           num_ctx: 4096,
           // ... other settings
       };
       
       // Save to database
       sampler_service.upsert_config(sampler_config).await?;
       
       // Create mock Ollama server that captures requests
       let mock_server = MockServer::start().await;
       Mock::given(method("POST"))
           .and(path("/api/chat"))
           .respond_with(ResponseTemplate::new(200)
               .set_body_json(json!({"response": "test"})))
           .mount(&mock_server)
           .await;
       
       // Make AI request through the system
       let ai_service = create_ai_service_with_url(&mock_server.uri());
       ai_service.complete(/* request */).await?;
       
       // Verify the request included our sampler settings
       let received_requests = mock_server.received_requests().await;
       let request_body: Value = serde_json::from_slice(&received_requests[0].body)?;
       
       assert_eq!(request_body["temperature"], 0.3);
       assert_eq!(request_body["top_p"], 0.95);
       assert_eq!(request_body["min_p"], 0.05);
       assert_eq!(request_body["repeat_penalty"], 1.2);
       assert_eq!(request_body["num_ctx"], 4096);
   }
   ```

2. **Test All Providers Can Process Requests**:
   ```rust
   #[tokio::test]
   async fn test_all_ui_providers_functional() {
       // Get list of providers shown in UI
       let ui_providers = vec!["ollama", "llamacpp", "lmstudio", "openai", "anthropic"];
       
       for provider in ui_providers {
           // Configure model for this provider
           let model_config = ModelConfiguration {
               role: "tool_calling",
               provider: provider.to_string(),
               model_name: get_test_model_for_provider(provider),
               // ...
           };
           
           model_service.set_model_config(model_config).await?;
           
           // Create mock server for provider
           let mock = create_provider_mock(provider).await;
           
           // Attempt to make a request
           let result = ai_service.complete(CompletionRequest {
               prompt: "Test prompt",
               // ...
           }).await;
           
           // Verify request was successful
           assert!(result.is_ok(), "Provider {} failed to process request: {:?}", 
                   provider, result.err());
           
           // Verify correct endpoint was called
           mock.assert();
       }
   }
   ```

3. **Test Configuration Persistence Across Restarts**:
   ```rust
   #[tokio::test]
   async fn test_config_persists_across_service_restart() {
       // Set up initial configurations
       let tool_model = ModelConfiguration {
           role: "tool_calling",
           provider: "ollama",
           model_name: "qwen2.5:7b",
       };
       let draft_model = ModelConfiguration {
           role: "drafting", 
           provider: "lmstudio",
           model_name: "llama3.3:70b",
       };
       
       // Save configurations
       model_service.set_model_config(tool_model.clone()).await?;
       model_service.set_model_config(draft_model.clone()).await?;
       
       // Save sampler configs
       let sampler_config = SamplerConfig {
           provider: "ollama",
           model_name: "qwen2.5:7b",
           temperature: 0.5,
           think_mode: true,
           // ...
       };
       sampler_service.upsert_config(sampler_config.clone()).await?;
       
       // Simulate service restart by dropping and recreating services
       drop(model_service);
       drop(sampler_service);
       drop(ai_service);
       
       // Recreate services (simulating restart)
       let model_service = ModelConfigService::new(db_pool.clone());
       let sampler_service = SamplerConfigService::new(db_pool.clone());
       let ai_service = create_ai_service(model_service.clone(), sampler_service.clone());
       
       // Verify configurations are still present
       let loaded_tool_model = model_service.get_model_config("tool_calling").await?;
       assert_eq!(loaded_tool_model.provider, "ollama");
       assert_eq!(loaded_tool_model.model_name, "qwen2.5:7b");
       
       let loaded_draft_model = model_service.get_model_config("drafting").await?;
       assert_eq!(loaded_draft_model.provider, "lmstudio");
       assert_eq!(loaded_draft_model.model_name, "llama3.3:70b");
       
       let loaded_sampler = sampler_service.get_config("ollama", "qwen2.5:7b").await?;
       assert_eq!(loaded_sampler.temperature, 0.5);
       assert_eq!(loaded_sampler.think_mode, true);
   }
   ```

4. **Test Model Selection Respected in AI Calls**:
   ```rust
   #[tokio::test]
   async fn test_model_selection_respected() {
       // Configure specific models for each role
       model_service.set_model_config(ModelConfiguration {
           role: "tool_calling",
           provider: "ollama",
           model_name: "mistral:7b",
       }).await?;
       
       model_service.set_model_config(ModelConfiguration {
           role: "drafting",
           provider: "ollama", 
           model_name: "llama3.3:70b",
       }).await?;
       
       // Set up mocks that verify correct model is used
       let tool_mock = Mock::given(method("POST"))
           .and(path("/api/chat"))
           .and(body_json_schema(json!({
               "model": {"const": "mistral:7b"}
           })))
           .respond_with(ResponseTemplate::new(200))
           .expect(1)
           .mount(&mock_server)
           .await;
       
       let draft_mock = Mock::given(method("POST"))
           .and(path("/api/chat"))
           .and(body_json_schema(json!({
               "model": {"const": "llama3.3:70b"}
           })))
           .respond_with(ResponseTemplate::new(200))
           .expect(1)
           .mount(&mock_server)
           .await;
       
       // Make tool calling request
       tool_executor.execute("List my emails").await?;
       
       // Make drafting request
       email_drafter.draft_email("Write a test email").await?;
       
       // Mocks will verify correct models were used
   }
   ```

5. **Test Edge Cases and Error Scenarios**:
   ```rust
   #[tokio::test]
   async fn test_missing_provider_configuration() {
       // Remove all configurations
       sqlx::query!("DELETE FROM ai_model_configurations").execute(&db_pool).await?;
       
       // Attempt to use AI service - should use defaults or fail gracefully
       let result = ai_service.complete(/* request */).await;
       
       // Verify appropriate error or default behavior
       match result {
           Ok(_) => {
               // Should have used default configuration
               let config = model_service.get_model_config("tool_calling").await?;
               assert_eq!(config.model_name, "qwen2.5:7b"); // Default from migration
           },
           Err(e) => {
               // Should be a clear configuration error
               assert!(e.to_string().contains("configuration"));
           }
       }
   }
   ```

6. **Test MCP Tool Integration**:
   ```rust
   #[tokio::test]
   async fn test_mcp_tools_use_configured_models() {
       // Configure models via MCP tools
       let set_tool_result = mcp_handler.handle_tool_call(
           "set_tool_calling_model",
           json!({"provider": "ollama", "model": "qwen2.5:7b"})
       ).await?;
       
       let set_draft_result = mcp_handler.handle_tool_call(
           "set_drafting_model",
           json!({"provider": "lmstudio", "model": "llama3.3:70b"})
       ).await?;
       
       // Verify configurations were saved
       let get_config_result = mcp_handler.handle_tool_call(
           "get_model_configurations",
           json!({})
       ).await?;
       
       let configs: Vec<ModelConfiguration> = serde_json::from_value(get_config_result)?;
       assert_eq!(configs.len(), 2);
       
       // Process email with configured models
       let process_result = mcp_handler.handle_tool_call(
           "process_email_instructions",
           json!({"instruction": "Reply to the latest email"})
       ).await?;
       
       // Verify correct models were used (check mock server logs)
   }
   ```

**Test Strategy:**

Run the integration test suite with the following verification steps:

1. **Environment Setup**:
   - Create a test database with all migrations applied
   - Set up mock servers for each AI provider (Ollama, LM Studio, etc.)
   - Configure test-specific environment variables for provider URLs

2. **Run Individual Test Categories**:
   ```bash
   # Test sampler configuration application
   cargo test test_sampler_config_applied_to_requests -- --nocapture
   
   # Test all providers
   cargo test test_all_ui_providers_functional -- --nocapture
   
   # Test persistence
   cargo test test_config_persists_across_service_restart -- --nocapture
   
   # Test model selection
   cargo test test_model_selection_respected -- --nocapture
   ```

3. **Verify Mock Server Interactions**:
   - Check that each test's mock server received the expected requests
   - Verify request bodies contain correct model names and sampler parameters
   - Ensure no unexpected API calls were made

4. **Database State Verification**:
   - After each test, query the database to verify configurations are correctly stored
   - Check both ai_model_configurations and ai_sampler_configs tables
   - Verify foreign key constraints are maintained

5. **Error Scenario Testing**:
   - Disconnect mock servers to test connection failures
   - Corrupt database entries to test validation
   - Remove configurations to test default fallbacks

6. **Performance Testing**:
   - Run tests with timing to ensure configuration lookups don't add significant latency
   - Test with multiple concurrent requests to verify thread safety

7. **Full Integration Test**:
   ```bash
   cargo test --test ai_configuration_integration -- --test-threads=1
   ```
   Run all tests sequentially to avoid database conflicts
