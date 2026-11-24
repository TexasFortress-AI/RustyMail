// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The tool list in the chat assistant's system prompt.
//!
//! The chat calls tools by writing `TOOL_CALL: name {json}`, so the model can
//! only use the parameter names it is shown. Each tool is listed with its
//! parameters and their descriptions (which mark the required ones); without
//! them the model guessed names, got an error and spent rounds retrying.

use serde_json::Value;

/// The "AVAILABLE TOOLS" section for `tools` (dashboard tool listings, with a
/// `parameters` map of name to description; an MCP `inputSchema` also works).
pub fn describe_tools(tools: &[Value]) -> String {
    let mut prompt = format!("\n\nAVAILABLE TOOLS ({} total):\n", tools.len());
    prompt.push_str("You have access to the following tools. To use a tool, respond with:\n");
    prompt.push_str("TOOL_CALL: tool_name {\"param1\": \"value1\", \"param2\": \"value2\"}\n");
    prompt.push_str("Use exactly the parameter names listed under each tool.\n\n");
    for (i, tool) in tools.iter().enumerate() {
        let text = |key: &str, default: &'static str| {
            tool.get(key)
                .and_then(Value::as_str)
                .unwrap_or(default)
                .to_string()
        };
        prompt.push_str(&format!(
            "{}. **{}**: {}\n",
            i + 1,
            text("name", "unknown"),
            text("description", "No description")
        ));
        for (name, description) in parameters_of(tool) {
            prompt.push_str(&format!("   - {name}: {description}\n"));
        }
    }
    prompt
}

/// (name, description) of each parameter, sorted by name.
fn parameters_of(tool: &Value) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = match tool.get("parameters") {
        Some(Value::Object(map)) => map
            .iter()
            .map(|(name, description)| {
                let description = description
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| {
                        description
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                (name.clone(), description)
            })
            .collect(),
        _ => schema_parameters(tool),
    };
    found.sort();
    found
}

/// Parameters from an MCP `inputSchema` (`properties` + `required`).
fn schema_parameters(tool: &Value) -> Vec<(String, String)> {
    let Some(schema) = tool.get("inputSchema") else {
        return Vec::new();
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| {
            props
                .iter()
                .map(|(name, prop)| {
                    let mut description = prop
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if required.contains(&name.as_str()) && !description.starts_with("REQUIRED") {
                        description = format!("REQUIRED. {description}");
                    }
                    (name.clone(), description)
                })
                .collect()
        })
        .unwrap_or_default()
}
