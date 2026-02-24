# Task ID: 39

**Title:** Persist Email Assistant provider/model selection to database

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Save TopBar provider selection from in-memory ProviderManager to ai_model_configurations table with role='chatbot' and load on service startup to persist user's Email Assistant model choice across restarts.

**Details:**

Modify the Email Assistant to persist provider/model selection to the database instead of only storing in-memory:

1. **Update ai_model_configurations table usage**:
   - Use role='chatbot' for Email Assistant provider/model selection
   - Store provider name, model name, and any provider-specific settings
   - Ensure compatibility with existing 'tool_calling' and 'drafting' roles

2. **Modify ProviderManager** (src/dashboard/services/ai/provider_manager.rs):
   ```rust
   // Add model_config_service dependency
   struct ProviderManager {
       providers: Arc<RwLock<HashMap<String, Box<dyn AIProvider>>>>,
       current_provider: Arc<RwLock<String>>,
       model_config_service: Arc<ModelConfigService>,
   }
   
   // Update set_current_provider to persist to database
   pub async fn set_current_provider(&self, provider_name: String, model_name: String) -> Result<()> {
       // Update in-memory state
       let mut current = self.current_provider.write().await;
       *current = provider_name.clone();
       
       // Persist to database
       let config = ModelConfiguration {
           role: "chatbot".to_string(),
           provider: provider_name,
           model_name,
           base_url: None, // Provider-specific, set if needed
           api_key: None,  // Provider-specific, set if needed
           additional_config: None,
       };
       
       self.model_config_service.set_model_config(config).await?;
       Ok(())
   }
   ```

3. **Add startup initialization**:
   ```rust
   // In ProviderManager::new() or init()
   pub async fn init(&self) -> Result<()> {
       // Load saved chatbot configuration
       if let Some(config) = self.model_config_service
           .get_model_config("chatbot").await? {
           
           // Set current provider from database
           let mut current = self.current_provider.write().await;
           *current = config.provider.clone();
           
           // Optionally validate provider exists
           let providers = self.providers.read().await;
           if !providers.contains_key(&config.provider) {
               warn!("Saved provider {} not available, using default", config.provider);
               *current = "ollama".to_string(); // Or other default
           }
       }
       Ok(())
   }
   ```

4. **Update TopBar component integration**:
   - Ensure TopBar calls the updated set_current_provider method
   - Pass both provider and model name when selection changes
   - Handle any UI state updates after persistence

5. **Migration considerations**:
   - Check if 'chatbot' role already exists in ai_model_configurations
   - Handle upgrade path for users with existing in-memory selections
   - Provide sensible defaults if no configuration exists

6. **Error handling**:
   - Gracefully handle database write failures (continue with in-memory)
   - Log persistence errors without breaking provider switching
   - Implement retry logic for transient database errors

**Test Strategy:**

1. **Unit tests for persistence logic**:
   - Mock ModelConfigService and verify set_model_config is called with correct parameters
   - Test that set_current_provider updates both in-memory and database state
   - Verify error handling when database write fails

2. **Integration tests for startup loading**:
   - Insert test configuration with role='chatbot' into database
   - Initialize ProviderManager and verify it loads the saved provider
   - Test fallback behavior when saved provider doesn't exist
   - Verify no configuration scenario uses appropriate defaults

3. **End-to-end testing**:
   - Start service and select a provider/model in TopBar
   - Restart service and verify selection is preserved
   - Test switching between different providers and models
   - Verify other roles ('tool_calling', 'drafting') are not affected

4. **Database verification**:
   ```sql
   -- Check chatbot configuration is saved
   SELECT * FROM ai_model_configurations WHERE role = 'chatbot';
   
   -- Verify only one chatbot configuration exists
   SELECT COUNT(*) FROM ai_model_configurations WHERE role = 'chatbot';
   ```

5. **Concurrency testing**:
   - Simulate multiple provider switches in quick succession
   - Verify final database state matches last selection
   - Test simultaneous reads during provider switch
