// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Runs a tool by name through the dispatcher that has it.
//!
//! Low-level tools run in `handlers::execute_mcp_tool_inner`; the tools only
//! the high-level set has (the agent, drafting, model settings, jobs) run in
//! `high_level_tools::execute_high_level_tool`. A tool in both sets runs the
//! low-level way, as `/api/dashboard/mcp/execute` does without a variant.
//! The agent and the chat assistant use this, so a high-level tool they are
//! offered actually runs instead of answering "not yet implemented".

use super::handlers::{execute_mcp_tool_inner, get_mcp_tools_jsonrpc_format};
use super::high_level_tools::execute_high_level_tool_without_agent;
use crate::dashboard::services::DashboardState;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;

static LOW_LEVEL_TOOLS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    get_mcp_tools_jsonrpc_format()
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
});

/// The `/api/dashboard/mcp/execute` variant that runs `tool`.
pub fn variant_of(tool: &str) -> &'static str {
    if LOW_LEVEL_TOOLS.contains(tool) {
        "low-level"
    } else {
        "high-level"
    }
}

/// Runs `tool` with `params` through the dispatcher that has it. The agent
/// (`process_email_instructions`) is not run from here: this is what the agent
/// itself uses, and tool_policy never offers it the agent.
pub async fn execute(state: &DashboardState, tool: &str, params: Value) -> Value {
    if variant_of(tool) == "low-level" {
        execute_mcp_tool_inner(state, tool, params).await
    } else {
        // Never the agent itself: the agent runs its tools through here.
        execute_high_level_tool_without_agent(state, tool, params).await
    }
}
