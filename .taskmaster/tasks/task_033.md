# Task ID: 33

**Title:** Add sampler configuration schema to database

**Status:** done

**Dependencies:** 1 ✓

**Priority:** high

**Description:** Create a new database table 'ai_sampler_configs' to store per-model sampler settings including temperature, top_p, top_k, min_p, repeat_penalty, num_ctx (context window), think mode, stop sequences, and other provider-specific options.

**Details:**

Create a new database migration file migrations/005_create_ai_sampler_configs.sql with the following schema:

```sql
CREATE TABLE ai_sampler_configs (
    id SERIAL PRIMARY KEY,
    provider VARCHAR(255) NOT NULL,
    model_name VARCHAR(255) NOT NULL,
    temperature DECIMAL(3,2) DEFAULT 0.7,
    top_p DECIMAL(3,2) DEFAULT 1.0,
    top_k INTEGER DEFAULT NULL,
    min_p DECIMAL(4,3) DEFAULT 0.01,
    repeat_penalty DECIMAL(3,2) DEFAULT 1.0,
    num_ctx INTEGER DEFAULT 2048,
    think_mode BOOLEAN DEFAULT FALSE,
    stop_sequences TEXT[] DEFAULT '{}',
    provider_specific_options JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider, model_name),
    FOREIGN KEY (provider, model_name) REFERENCES ai_model_configurations(provider, model_name) ON DELETE CASCADE
);

-- Create trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_ai_sampler_configs_updated_at BEFORE UPDATE
    ON ai_sampler_configs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default configurations for known models
INSERT INTO ai_sampler_configs (provider, model_name, temperature, top_p, min_p, repeat_penalty, num_ctx, think_mode, stop_sequences, provider_specific_options)
VALUES 
    ('ollama', 'qwen2.5:7b', 0.7, 1.0, 0.01, 1.0, 8192, FALSE, '{}', '{}'),
    ('ollama', 'llama3.3:70b', 0.7, 1.0, 0.01, 1.0, 8192, FALSE, '{}', '{}'),
    ('openai', 'gpt-4', 0.7, 1.0, NULL, NULL, 8192, FALSE, '{}', '{"presence_penalty": 0, "frequency_penalty": 0}'),
    ('anthropic', 'claude-3-opus', 0.7, 1.0, NULL, NULL, 200000, FALSE, '{}', '{}'),
    ('glm', 'GLM-4.7-Flash', 0.7, 1.0, 0.01, 1.0, 51200, FALSE, '{}', '{}');
```

Key implementation considerations:
1. Use DECIMAL types for floating-point sampler values to ensure precision
2. Make top_k nullable since not all providers support it
3. Use TEXT[] for stop_sequences to support multiple stop strings
4. Use JSONB for provider_specific_options to allow flexible storage of provider-unique settings
5. Include composite foreign key to ai_model_configurations table
6. Add unique constraint on (provider, model_name) to ensure one config per model
7. Include sensible defaults for common models with their known optimal settings
8. Add timestamps for audit trail

**Test Strategy:**

1. Run the migration and verify table creation:
   ```sql
   \d ai_sampler_configs
   ```
   Confirm all columns exist with correct types and constraints

2. Test foreign key constraint:
   - Try inserting a config for a non-existent model and verify it fails
   - Insert a valid model in ai_model_configurations first, then add its sampler config

3. Test unique constraint:
   - Try inserting duplicate (provider, model_name) combinations and verify it fails

4. Verify default values:
   - Insert a row with minimal data and check that defaults are applied correctly

5. Test the update trigger:
   - Update a row and verify updated_at changes while created_at remains the same

6. Query the default configurations:
   ```sql
   SELECT * FROM ai_sampler_configs ORDER BY provider, model_name;
   ```
   Verify all 5 default model configurations are present with correct values

7. Test JSONB operations on provider_specific_options:
   ```sql
   UPDATE ai_sampler_configs 
   SET provider_specific_options = '{"custom_param": "value"}'::jsonb 
   WHERE provider = 'ollama' AND model_name = 'qwen2.5:7b';
   ```

8. Test cascade delete:
   - Delete a model from ai_model_configurations and verify its sampler config is also deleted
