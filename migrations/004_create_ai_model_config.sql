-- AI Model Configurations Table
-- Stores AI model settings for different roles (tool-calling, drafting)

CREATE TABLE IF NOT EXISTS ai_model_configurations (
    role TEXT PRIMARY KEY,  -- 'tool_calling' or 'drafting'
    provider TEXT NOT NULL,  -- 'ollama', 'openai', 'anthropic', etc.
    model_name TEXT NOT NULL,  -- e.g., 'qwen2.5:7b', 'llama3.3:70b'
    base_url TEXT,  -- Provider API base URL (e.g., 'http://localhost:11434' for Ollama)
    api_key TEXT,  -- Optional API key for commercial providers
    additional_config TEXT,  -- JSON for provider-specific settings
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DROP TRIGGER IF EXISTS update_ai_model_config_timestamp;

CREATE TRIGGER update_ai_model_config_timestamp
    AFTER UPDATE ON ai_model_configurations
    BEGIN
        UPDATE ai_model_configurations SET updated_at = CURRENT_TIMESTAMP
        WHERE role = NEW.role;
    END;

-- No default configurations inserted - users must configure their AI models
-- via the WebUI Settings page or MCP tools before using AI features.
--
-- Recommended models (see .env.example for full documentation):
--   tool_calling: hf.co/unsloth/GLM-4.7-Flash-GGUF:q8_0 (via Ollama)
--   drafting:     gemma3:27b-it-q8_0 (via Ollama)
