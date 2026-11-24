// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! What happens to a tool call the chat assistant's model makes.
//!
//! - A tool it was not offered is refused.
//! - A tool it may run (tool_policy) runs.
//! - Any other tool it was offered (send, delete, move...) is not run: it
//!   becomes a pending action, shown to the owner exactly as the server
//!   recorded it, with the sender and subject of the messages it would touch
//!   looked up by the server, not described by the model. Only the owner's
//!   click runs it; a typed "yes" goes through the model and only makes
//!   another pending action.

use super::tool_policy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::future::Future;

/// How many affected messages are looked up to describe a pending action.
const DESCRIBED_MESSAGES: usize = 10;

/// A call the model made that only the owner can run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingAction {
    pub id: String,
    pub tool: String,
    /// The `/api/dashboard/mcp/execute` variant that runs it.
    pub variant: String,
    /// Exactly what runs if the owner confirms.
    pub parameters: Value,
    /// The messages it acts on, as "uid N: sender, subject", from the server.
    pub affected: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum ChatToolOutcome {
    /// The tool ran; the line to feed back to the model.
    Ran(String),
    /// Held for the owner; the line to feed back to the model.
    AwaitingOwner(PendingAction, String),
    /// Refused; the line to feed back to the model.
    Refused(String),
}

/// Handles one tool call. `run` executes a tool (the chat's MCP call); it is
/// used for allowed tools and for read-only lookups of affected messages.
pub async fn handle_call<F, Fut>(
    tool: &str,
    parameters: Value,
    offered: &HashSet<String>,
    low_level_tools: &HashSet<String>,
    run: F,
) -> ChatToolOutcome
where
    F: Fn(String, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    if !offered.contains(tool) {
        return ChatToolOutcome::Refused(format!(
            "TOOL_ERROR {tool}: not available in this conversation"
        ));
    }
    if tool_policy::model_may_run(tool) {
        return match run(tool.to_string(), parameters).await {
            Ok(result) => ChatToolOutcome::Ran(format!(
                "TOOL_RESULT {}: {}",
                tool,
                serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
            )),
            Err(e) => ChatToolOutcome::Ran(format!("TOOL_ERROR {tool}: {e}")),
        };
    }
    let affected = describe_affected(&parameters, &run).await;
    let action = PendingAction {
        id: uuid::Uuid::new_v4().to_string(),
        tool: tool.to_string(),
        variant: if low_level_tools.contains(tool) {
            "low-level"
        } else {
            "high-level"
        }
        .to_string(),
        parameters,
        affected,
    };
    let line = format!(
        "PENDING_OWNER_CONFIRMATION {tool}: not run. It is shown to the owner with a Confirm \
         button; only the owner can run it. Tell the owner what it would do."
    );
    ChatToolOutcome::AwaitingOwner(action, line)
}

/// "uid N: sender, subject" for the messages a call names, looked up with
/// get_email_by_uid (read-only).
async fn describe_affected<F, Fut>(parameters: &Value, run: &F) -> Vec<String>
where
    F: Fn(String, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    let folder = parameters
        .get("folder")
        .or_else(|| parameters.get("source_folder"))
        .and_then(Value::as_str);
    let (Some(folder), Some(account)) = (folder, parameters.get("account_id")) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for uid in uids_in(parameters).into_iter().take(DESCRIBED_MESSAGES) {
        let lookup = json!({"account_id": account, "folder": folder, "uid": uid});
        let line = match run("get_email_by_uid".to_string(), lookup).await {
            Ok(result) => describe_message(uid, &result),
            Err(_) => format!("uid {uid}: (could not be looked up)"),
        };
        lines.push(line);
    }
    lines
}

/// The UIDs a call names: `uid`, or `uids` as an array or comma-separated.
pub fn uids_in(parameters: &Value) -> Vec<u32> {
    if let Some(uid) = parameters.get("uid").and_then(Value::as_u64) {
        return vec![uid as u32];
    }
    match parameters.get("uids") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_u64)
            .map(|u| u as u32)
            .collect(),
        Some(Value::String(list)) => list
            .split(',')
            .filter_map(|u| u.trim().parse().ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// "uid N: sender, subject" from a get_email_by_uid result.
pub fn describe_message(uid: u32, result: &Value) -> String {
    let email = result.get("data").unwrap_or(result);
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| email.get(*k).and_then(Value::as_str))
            .unwrap_or("?")
            .to_string()
    };
    format!(
        "uid {uid}: {}, \"{}\"",
        text(&["from_address", "from"]),
        text(&["subject"])
    )
}
