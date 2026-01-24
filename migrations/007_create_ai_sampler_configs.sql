-- AI Sampler Configurations Table
-- Stores per-model sampler settings for local LLM inference (Ollama, llama.cpp, LM Studio)
--
-- Configuration priority (highest to lowest):
-- 1. Database record for specific (provider, model_name) - set via Web UI
-- 2. Environment variables (SAMPLER_DEFAULT_*) - deployment-time defaults
-- 3. Code defaults in Rust adapters - last resort fallback
--
-- NOTE: No seed data is inserted by this migration.
-- Default configs are created at runtime from environment variables.

CREATE TABLE ai_sampler_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Model identification (provider + model_name is unique)
    provider TEXT NOT NULL,           -- 'ollama', 'llamacpp', 'lmstudio', etc.
    model_name TEXT NOT NULL,         -- e.g., 'qwen2.5:7b', 'GLM-4.7-Flash:Q8_K_XL'

    -- Core sampler parameters
    temperature REAL,                 -- 0.0 = deterministic, higher = more random
    top_p REAL,                       -- Nucleus sampling (1.0 = disabled)
    top_k INTEGER,                    -- Top-k sampling (NULL = disabled)
    min_p REAL,                       -- Min-p sampling (llama.cpp exclusive)
    repeat_penalty REAL,              -- 1.0 = disabled

    -- Context and generation limits
    num_ctx INTEGER,                  -- Context window size in tokens
    max_tokens INTEGER,               -- Max tokens to generate (NULL = no limit)

    -- Thinking mode control (for GLM-4, Qwen, etc.)
    think_mode INTEGER DEFAULT 0,     -- 0 = disabled, 1 = enabled (SQLite uses INTEGER for bool)

    -- Stop sequences (JSON array of strings)
    stop_sequences TEXT DEFAULT '[]',

    -- Provider-specific options (JSON object)
    provider_options TEXT DEFAULT '{}',

    -- Metadata
    description TEXT,                 -- Optional description of this config
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- Ensure one config per provider+model combination
    UNIQUE(provider, model_name)
);

-- Index for fast lookups by provider
CREATE INDEX idx_sampler_configs_provider ON ai_sampler_configs(provider);
CREATE INDEX idx_sampler_configs_lookup ON ai_sampler_configs(provider, model_name);

-- Trigger to update timestamp on modification
CREATE TRIGGER update_ai_sampler_configs_timestamp
    AFTER UPDATE ON ai_sampler_configs
    BEGIN
        UPDATE ai_sampler_configs SET updated_at = CURRENT_TIMESTAMP
        WHERE id = NEW.id;
    END;
