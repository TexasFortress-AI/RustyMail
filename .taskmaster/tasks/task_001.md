# Task ID: 1

**Title:** Create database migration for AI model configurations

**Status:** done

**Dependencies:** None

**Priority:** high

**Description:** Add ai_model_configurations table to store tool-calling and drafting model settings

**Details:**

Create migrations/004_create_ai_model_config.sql with table schema for role, provider, model_name, base_url, api_key, additional_config. Include default entries for qwen2.5:7b (tool-calling) and llama3.3:70b (drafting)

**Test Strategy:**

No test strategy provided.
