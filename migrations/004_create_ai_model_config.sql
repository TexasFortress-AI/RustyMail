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

CREATE TRIGGER update_ai_model_config_timestamp
    AFTER UPDATE ON ai_model_configurations
    BEGIN
        UPDATE ai_model_configurations SET updated_at = CURRENT_TIMESTAMP
        WHERE role = NEW.role;
    END;

-- Insert default configurations
-- Tool-calling model: lightweight model for routing and executing workflows
INSERT OR IGNORE INTO ai_model_configurations (role, provider, model_name, base_url)
VALUES ('tool_calling', 'ollama', 'qwen2.5:7b', 'http://localhost:11434');

-- Drafting model: larger model for generating email text
INSERT OR IGNORE INTO ai_model_configurations (role, provider, model_name, base_url)
VALUES ('drafting', 'ollama', 'llama3.3:70b', 'http://localhost:11434');
