# Task ID: 34

**Title:** Create Rust service layer for sampler configurations

**Status:** done

**Dependencies:** 1 ✓, 2 ✓

**Priority:** high

**Description:** Implement a SamplerConfigService in Rust that manages sampler configurations in the database, providing methods to get/save configs and integrate with existing AI provider adapters to apply sampler settings dynamically.

**Details:**

Create src/dashboard/services/ai/sampler_config.rs with the following components:

1. Define SamplerConfiguration struct with fields:
   - id: i64
   - provider: String (ollama, llamacpp, etc.)
   - model_name: String
   - temperature: Option<f32>
   - top_p: Option<f32>
   - top_k: Option<i32>
   - repeat_penalty: Option<f32>
   - seed: Option<i64>
   - max_tokens: Option<i32>
   - stop_sequences: Option<Vec<String>>
   - created_at: DateTime
   - updated_at: DateTime

2. Implement SamplerConfigService with methods:
   - async fn get_config_for_model(provider: &str, model: &str) -> Result<Option<SamplerConfiguration>>
   - async fn save_config(config: SamplerConfiguration) -> Result<SamplerConfiguration>
   - async fn list_configs() -> Result<Vec<SamplerConfiguration>>
   - async fn get_default_config_for_provider(provider: &str) -> Result<SamplerConfiguration>
   - async fn delete_config(id: i64) -> Result<()>

3. Create database migration migrations/005_create_sampler_configs.sql:
   ```sql
   CREATE TABLE sampler_configurations (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       provider TEXT NOT NULL,
       model_name TEXT NOT NULL,
       temperature REAL,
       top_p REAL,
       top_k INTEGER,
       repeat_penalty REAL,
       seed INTEGER,
       max_tokens INTEGER,
       stop_sequences TEXT, -- JSON array
       created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
       updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
       UNIQUE(provider, model_name)
   );
   
   -- Insert default configurations
   INSERT INTO sampler_configurations (provider, model_name, temperature, top_p, top_k, max_tokens)
   VALUES 
   ('ollama', 'default', 0.7, 0.9, 40, 2048),
   ('llamacpp', 'default', 0.8, 0.95, 50, 4096);
   ```

4. Update existing AI provider adapters to use SamplerConfigService:
   - Modify OllamaAdapter::generate_response() to fetch sampler config before API call
   - Modify LlamaCppAdapter::generate_response() similarly
   - Apply sampler settings to the request payload dynamically
   - Fall back to provider defaults if no config found

5. Add sampler config application logic:
   ```rust
   // In adapter's generate_response method
   let sampler_config = self.sampler_service
       .get_config_for_model(&self.provider, &model_name)
       .await?
       .or_else(|| self.sampler_service.get_default_config_for_provider(&self.provider).await.ok())?;
   
   // Apply to request
   if let Some(temp) = sampler_config.temperature {
       request.temperature = Some(temp);
   }
   // ... apply other settings
   ```

6. Ensure thread-safe access to SamplerConfigService using Arc<SamplerConfigService> in adapters.

**Test Strategy:**

1. Unit tests for SamplerConfigService:
   - Test CRUD operations (create, read, update, delete configs)
   - Test unique constraint on (provider, model_name)
   - Test get_config_for_model returns correct config or None
   - Test default config retrieval for each provider
   - Test list_configs returns all configurations

2. Integration tests with database:
   - Create test database and run migration
   - Verify default configs are inserted
   - Test concurrent access patterns
   - Test config updates reflect in subsequent reads

3. Adapter integration tests:
   - Mock SamplerConfigService in OllamaAdapter tests
   - Verify generate_response applies sampler settings correctly
   - Test fallback to defaults when no config exists
   - Test that all sampler parameters are properly mapped to API request

4. Manual testing:
   - Create custom sampler config via direct DB insert
   - Call generate_response and verify API logs show correct parameters
   - Test with extreme values (temperature=0, temperature=2.0)
   - Verify stop sequences are properly serialized/deserialized

5. Performance testing:
   - Measure overhead of config lookup on each generate_response call
   - Consider caching strategy if lookup impacts performance
