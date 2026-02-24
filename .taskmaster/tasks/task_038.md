# Task ID: 38

**Title:** Wire sampler configuration from database to provider adapters

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Modify all AI provider adapters (OllamaAdapter, LlamaAdapter, LMStudioAdapter, etc.) to fetch and apply sampler configurations from the database instead of using hardcoded defaults, implementing a priority system: database > environment variables > code defaults.

**Details:**

Update all provider adapters to integrate with the SamplerConfigService and apply configurations dynamically:

1. **Update OllamaAdapter** (src/dashboard/services/ai/providers/ollama.rs):
   ```rust
   // Add sampler_config_service to the struct
   struct OllamaAdapter {
       base_url: String,
       sampler_config_service: Arc<SamplerConfigService>,
       // ... existing fields
   }
   
   // In the complete() method, before making the request:
   async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
       // Fetch sampler config from database
       let sampler_config = self.sampler_config_service
           .get_config_for_model("ollama", &request.model)
           .await?;
       
       // Build options with priority: DB > env > defaults
       let mut options = json!({});
       
       if let Some(config) = sampler_config {
           if let Some(temp) = config.temperature {
               options["temperature"] = json!(temp);
           }
           if let Some(top_p) = config.top_p {
               options["top_p"] = json!(top_p);
           }
           if let Some(top_k) = config.top_k {
               options["top_k"] = json!(top_k);
           }
           if let Some(repeat_penalty) = config.repeat_penalty {
               options["repeat_penalty"] = json!(repeat_penalty);
           }
           if let Some(num_ctx) = config.num_ctx {
               options["num_ctx"] = json!(num_ctx);
           }
           // Apply other sampler settings...
       } else {
           // Fall back to env vars or hardcoded defaults
           options["temperature"] = json!(env::var("OLLAMA_TEMPERATURE")
               .ok()
               .and_then(|v| v.parse::<f32>().ok())
               .unwrap_or(0.7));
           // ... other defaults
       }
       
       let ollama_request = json!({
           "model": request.model,
           "prompt": request.prompt,
           "options": options,
           "stream": request.stream.unwrap_or(false)
       });
       
       // Make the API request...
   }
   ```

2. **Update LlamaAdapter** (src/dashboard/services/ai/providers/llamacpp.rs):
   ```rust
   // Similar pattern but with llama.cpp specific parameters
   let sampler_config = self.sampler_config_service
       .get_config_for_model("llamacpp", &request.model)
       .await?;
   
   let mut body = json!({
       "prompt": request.prompt,
       "n_predict": request.max_tokens.unwrap_or(2048),
       "stream": request.stream.unwrap_or(false)
   });
   
   if let Some(config) = sampler_config {
       // Apply llama.cpp specific parameters
       if let Some(temp) = config.temperature {
           body["temperature"] = json!(temp);
       }
       if let Some(min_p) = config.min_p {
           body["min_p"] = json!(min_p);
       }
       if let Some(top_k) = config.top_k {
           body["top_k"] = json!(top_k);
       }
       // Handle stop sequences
       if let Some(stop) = config.stop_sequences {
           body["stop"] = json!(stop);
       }
   }
   ```

3. **Update LMStudioAdapter** (src/dashboard/services/ai/providers/lmstudio.rs):
   ```rust
   // LM Studio uses OpenAI-compatible format but supports additional samplers
   let sampler_config = self.sampler_config_service
       .get_config_for_model("lmstudio", &request.model)
       .await?;
   
   // Build request with sampler settings
   let mut lm_request = json!({
       "model": request.model,
       "messages": messages,
       "stream": request.stream.unwrap_or(false)
   });
   
   if let Some(config) = sampler_config {
       // Apply OpenAI-style parameters
       if let Some(temp) = config.temperature {
           lm_request["temperature"] = json!(temp);
       }
       if let Some(top_p) = config.top_p {
           lm_request["top_p"] = json!(top_p);
       }
       // LM Studio specific extensions
       if let Some(min_p) = config.min_p {
           lm_request["min_p"] = json!(min_p);
       }
       if let Some(repeat_penalty) = config.repeat_penalty {
           lm_request["frequency_penalty"] = json!(repeat_penalty);
       }
   }
   ```

4. **Update provider factory** to inject SamplerConfigService:
   ```rust
   // In src/dashboard/services/ai/provider_factory.rs
   pub fn create_provider(
       provider_type: &str,
       sampler_config_service: Arc<SamplerConfigService>,
       // ... other params
   ) -> Result<Box<dyn AIProvider>> {
       match provider_type {
           "ollama" => Ok(Box::new(OllamaAdapter::new(sampler_config_service)?)),
           "llamacpp" => Ok(Box::new(LlamaAdapter::new(sampler_config_service)?)),
           "lmstudio" => Ok(Box::new(LMStudioAdapter::new(sampler_config_service)?)),
           _ => Err(anyhow!("Unknown provider type: {}", provider_type))
       }
   }
   ```

5. **Add configuration caching** to reduce database queries:
   ```rust
   // In SamplerConfigService, add a cache layer
   struct SamplerConfigService {
       db_pool: Arc<DbPool>,
       cache: Arc<RwLock<HashMap<(String, String), CachedConfig>>>,
   }
   
   struct CachedConfig {
       config: Option<SamplerConfiguration>,
       expires_at: Instant,
   }
   
   impl SamplerConfigService {
       pub async fn get_config_for_model_cached(
           &self,
           provider: &str,
           model_name: &str
       ) -> Result<Option<SamplerConfiguration>> {
           let key = (provider.to_string(), model_name.to_string());
           
           // Check cache first
           {
               let cache = self.cache.read().await;
               if let Some(cached) = cache.get(&key) {
                   if cached.expires_at > Instant::now() {
                       return Ok(cached.config.clone());
                   }
               }
           }
           
           // Fetch from database
           let config = self.get_config_for_model(provider, model_name).await?;
           
           // Update cache
           {
               let mut cache = self.cache.write().await;
               cache.insert(key, CachedConfig {
                   config: config.clone(),
                   expires_at: Instant::now() + Duration::from_secs(300), // 5 min cache
               });
           }
           
           Ok(config)
       }
   }
   ```

6. **Handle provider-specific parameter mappings**:
   ```rust
   // Create a trait for parameter mapping
   trait SamplerParameterMapper {
       fn map_config_to_request(&self, config: &SamplerConfiguration) -> serde_json::Value;
   }
   
   // Implement for each provider with their specific parameter names
   impl SamplerParameterMapper for OllamaAdapter {
       fn map_config_to_request(&self, config: &SamplerConfiguration) -> serde_json::Value {
           let mut params = json!({});
           // Ollama uses "num_ctx" for context window
           if let Some(num_ctx) = config.num_ctx {
               params["num_ctx"] = json!(num_ctx);
           }
           // ... map other parameters
           params
       }
   }
   ```

**Test Strategy:**

1. **Integration Tests for Each Provider**:
   - Create test configurations in the database for each provider
   - Mock provider API endpoints to capture the actual requests being sent
   - Verify that sampler parameters from the database are correctly applied to requests
   - Test fallback to environment variables when no DB config exists
   - Test fallback to hardcoded defaults when neither DB nor env configs exist

2. **Priority Order Testing**:
   ```rust
   #[tokio::test]
   async fn test_config_priority_order() {
       // Set up: Create DB config with temperature=0.5
       let config = SamplerConfiguration {
           provider: "ollama".to_string(),
           model_name: "llama2:7b".to_string(),
           temperature: Some(0.5),
           // ...
       };
       sampler_service.save_config(config).await.unwrap();
       
       // Set environment variable to temperature=0.8
       env::set_var("OLLAMA_TEMPERATURE", "0.8");
       
       // Make completion request
       let response = ollama_adapter.complete(request).await.unwrap();
       
       // Verify DB value (0.5) was used, not env value (0.8)
       assert_eq!(captured_request["options"]["temperature"], 0.5);
   }
   ```

3. **Cache Performance Testing**:
   - Measure database query count before and after implementing cache
   - Verify cache expiration works correctly
   - Test cache invalidation when configurations are updated
   - Load test with multiple concurrent requests to ensure cache thread safety

4. **Provider-Specific Parameter Testing**:
   - Test Ollama-specific parameters (num_ctx, mirostat, etc.)
   - Test llama.cpp-specific parameters (min_p, top_k, grammar, etc.)
   - Test LM Studio OpenAI-compatible format with extensions
   - Verify stop sequences are correctly formatted for each provider

5. **Error Handling Tests**:
   - Test behavior when database is unavailable (should fall back gracefully)
   - Test invalid configuration values (e.g., temperature > 2.0)
   - Test missing required parameters
   - Verify error messages are helpful for debugging

6. **End-to-End Testing**:
   - Configure different sampler settings via the WebUI
   - Make actual completion requests to each provider
   - Verify response quality changes based on sampler settings
   - Test with real models to ensure parameters are having expected effects
