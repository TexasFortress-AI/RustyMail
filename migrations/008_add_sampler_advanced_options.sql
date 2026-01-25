-- Add advanced sampler options to ai_sampler_configs table
-- Migration 008: Add typical_p (top-n-sigma) and system_prompt fields

-- typical_p: Tail-free sampling (top-nσ) - filters out low probability tokens
-- Based on the second derivative of token probability distribution
-- Range: 0.0 to 1.0 (1.0 = disabled)
ALTER TABLE ai_sampler_configs ADD COLUMN typical_p REAL;

-- system_prompt: Custom system prompt for this model configuration
-- Overrides the default system prompt when set
-- Allows per-model customization of AI behavior
ALTER TABLE ai_sampler_configs ADD COLUMN system_prompt TEXT;
