# Task ID: 36

**Title:** Add sensible default sampler presets for common models

**Status:** done

**Dependencies:** 33 ✓, 34 ✓

**Priority:** medium

**Description:** Research and implement default sampler configurations for popular models used with local inference, creating presets for GLM-4.7-Flash, Qwen, Llama, Mistral models and generic defaults for cloud providers, storing them as seed data in the database with documentation.

**Details:**

Create a database migration to seed the ai_sampler_configs table with well-researched default configurations:

1. **Create migration file** migrations/006_seed_default_sampler_configs.sql:

```sql
-- GLM-4.7-Flash defaults (optimized for fast, coherent responses)
INSERT INTO ai_sampler_configs (provider, model_name, temperature, top_p, min_p, repeat_penalty, num_ctx, think_mode, created_at, updated_at)
VALUES 
    ('ollama', 'glm4:7b-flash', 0.7, 1.0, 0.01, 1.0, 51200, false, NOW(), NOW()),
    -- Temperature 0.7: Balanced creativity without excessive randomness
    -- top_p 1.0: No nucleus sampling restriction, model can use full vocabulary
    -- min_p 0.01: Filters out tokens with <1% probability to reduce noise
    -- repeat_penalty 1.0: No penalty, GLM models handle repetition well internally
    -- num_ctx 51200: Large context window for complex email threads
    -- think_mode false: Flash variant optimized for speed, not deep reasoning

    -- Qwen models (optimized for instruction following)
    ('ollama', 'qwen2.5:7b', 0.7, 0.95, 0.05, 1.1, 32768, false, NOW(), NOW()),
    ('ollama', 'qwen2.5:14b', 0.7, 0.95, 0.05, 1.1, 32768, false, NOW(), NOW()),
    ('ollama', 'qwen2.5:32b', 0.7, 0.95, 0.05, 1.1, 32768, false, NOW(), NOW()),
    -- Temperature 0.7: Standard for instruction-following tasks
    -- top_p 0.95: Slight nucleus sampling for more focused outputs
    -- min_p 0.05: Higher threshold to ensure quality token selection
    -- repeat_penalty 1.1: Slight penalty to encourage variety
    -- num_ctx 32768: Qwen's native context size
    -- think_mode false: Qwen models don't support CoT prompting

    -- Llama 3.x models (optimized for general purpose)
    ('ollama', 'llama3.2:3b', 0.8, 0.9, 0.05, 1.15, 8192, false, NOW(), NOW()),
    ('ollama', 'llama3.2:7b', 0.8, 0.9, 0.05, 1.15, 8192, false, NOW(), NOW()),
    ('ollama', 'llama3.1:70b', 0.7, 0.9, 0.05, 1.1, 131072, false, NOW(), NOW()),
    -- Temperature 0.8/0.7: Higher for smaller models, lower for larger
    -- top_p 0.9: Moderate nucleus sampling for coherence
    -- min_p 0.05: Standard quality threshold
    -- repeat_penalty 1.15/1.1: Higher for smaller models to reduce loops
    -- num_ctx: Model-specific limits
    
    -- Mistral models (optimized for reasoning)
    ('ollama', 'mistral:7b', 0.7, 0.95, 0.05, 1.1, 32768, false, NOW(), NOW()),
    ('ollama', 'mixtral:8x7b', 0.7, 0.95, 0.05, 1.1, 32768, false, NOW(), NOW()),
    ('ollama', 'mistral-large:123b', 0.6, 0.95, 0.05, 1.05, 32768, false, NOW(), NOW()),
    -- Temperature 0.6-0.7: Lower for larger models
    -- top_p 0.95: Consistent nucleus sampling
    -- repeat_penalty: Lower for larger models
    
    -- Generic cloud provider defaults
    ('openai', 'gpt-4-turbo', 0.7, 1.0, NULL, NULL, 128000, false, NOW(), NOW()),
    ('openai', 'gpt-4o', 0.7, 1.0, NULL, NULL, 128000, false, NOW(), NOW()),
    ('openai', 'gpt-3.5-turbo', 0.7, 1.0, NULL, NULL, 16384, false, NOW(), NOW()),
    -- OpenAI models: Only temperature and top_p supported
    -- num_ctx matches model's context window
    
    ('anthropic', 'claude-3-opus', 0.7, 1.0, NULL, NULL, 200000, false, NOW(), NOW()),
    ('anthropic', 'claude-3-sonnet', 0.7, 1.0, NULL, NULL, 200000, false, NOW(), NOW()),
    ('anthropic', 'claude-3-haiku', 0.7, 1.0, NULL, NULL, 200000, false, NOW(), NOW()),
    -- Anthropic: Similar to OpenAI, large context windows
    
    -- LlamaCpp defaults (for self-hosted models)
    ('llamacpp', 'default', 0.7, 0.95, 0.05, 1.1, 4096, false, NOW(), NOW());
    -- Conservative defaults for unknown models
```

2. **Add rollback migration** for reversibility:
```sql
-- Rollback: Remove only the seeded defaults, preserve user configurations
DELETE FROM ai_sampler_configs 
WHERE created_at = updated_at 
AND model_name IN (
    'glm4:7b-flash', 'qwen2.5:7b', 'qwen2.5:14b', 'qwen2.5:32b',
    'llama3.2:3b', 'llama3.2:7b', 'llama3.1:70b',
    'mistral:7b', 'mixtral:8x7b', 'mistral-large:123b',
    'gpt-4-turbo', 'gpt-4o', 'gpt-3.5-turbo',
    'claude-3-opus', 'claude-3-sonnet', 'claude-3-haiku',
    'default'
);
```

3. **Create documentation file** docs/sampler-presets.md explaining the rationale:
```markdown
# Default Sampler Configurations

## Overview
This document explains the default sampler settings for various AI models.

## Parameter Explanations

### Temperature (0.0 - 2.0)
- Controls randomness in token selection
- 0.0: Deterministic (always picks most likely token)
- 0.7-0.8: Balanced creativity for general tasks
- 1.0+: High creativity, may reduce coherence

### Top-p (0.0 - 1.0)
- Nucleus sampling: only consider tokens whose cumulative probability < top_p
- 1.0: Consider all tokens
- 0.9-0.95: Moderate filtering of unlikely tokens
- <0.9: More focused, deterministic outputs

### Min-p (0.0 - 1.0)
- Minimum probability threshold for token consideration
- Filters out tokens with probability < min_p
- 0.01-0.05: Standard range for quality filtering

### Repeat Penalty (1.0 - 2.0)
- Penalizes tokens that have appeared recently
- 1.0: No penalty
- 1.1-1.15: Light penalty for variety
- 1.5+: Strong penalty, may harm coherence

## Model-Specific Rationales
[Include detailed explanations for each model family]
```

4. **Update SamplerConfigService** to include a method for resetting to defaults:
```rust
impl SamplerConfigService {
    pub async fn reset_to_default(&self, provider: &str, model_name: &str) -> Result<()> {
        // Query the seeded defaults where created_at = updated_at
        // This identifies unchanged default configurations
    }
}
```

**Test Strategy:**

1. **Migration Testing**:
   - Run migration: `diesel migration run`
   - Verify all default configurations are inserted:
     ```sql
     SELECT provider, model_name, temperature, top_p, min_p, repeat_penalty, num_ctx 
     FROM ai_sampler_configs 
     ORDER BY provider, model_name;
     ```
   - Confirm 17 rows inserted with correct values
   - Test rollback removes only seeded data

2. **Configuration Validation**:
   - For each model, verify sampler settings are within valid ranges
   - Test that NULL values are properly set for cloud providers (min_p, repeat_penalty)
   - Verify num_ctx values match documented model limits

3. **Integration Testing**:
   - Start application and verify SamplerConfigService loads defaults
   - Test get_config_for_model() returns correct preset for each model
   - Verify WebUI displays presets correctly in dropdown
   - Test that user modifications don't affect default lookup

4. **Model Testing** (manual):
   - For each model type, generate a test email draft
   - Verify output quality matches expectations:
     - GLM-4.7-Flash: Fast, coherent responses
     - Qwen: Good instruction following
     - Llama: Balanced general purpose output
     - Mistral: Strong reasoning capability
   - Compare outputs with/without presets to validate improvements

5. **Documentation Verification**:
   - Review docs/sampler-presets.md for accuracy
   - Ensure all parameter choices are justified
   - Verify model-specific notes are comprehensive
