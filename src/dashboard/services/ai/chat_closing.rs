// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The chat assistant's closing reply when its tool rounds run out.
//!
//! Each round, the model either answers or calls tools. A model that keeps
//! calling tools (retrying a call it got wrong, say) used to reach the limit
//! with nothing written, so the owner got an empty reply even though work was
//! done. Now the model is asked once more, without tools, to say what it did;
//! and if it still writes nothing, the reply is built from what was recorded.

use super::chat_tool_gate::PendingAction;

/// Environment variable for the most tool rounds in one chat turn.
pub const MAX_TOOL_ROUNDS_ENV: &str = "CHATBOT_MAX_TOOL_ROUNDS";
const DEFAULT_MAX_TOOL_ROUNDS: usize = 5;

/// What the model is asked when its rounds are used up.
pub const CLOSING_REQUEST: &str = "Stop calling tools now. In plain language, tell the owner \
what you did, what is waiting for their confirmation, and anything that failed, and answer \
their request as far as you can from what you have.";

/// The most tool rounds in one chat turn (at least 1).
pub fn max_tool_rounds() -> usize {
    std::env::var(MAX_TOOL_ROUNDS_ENV)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|&n: &usize| n >= 1)
        .unwrap_or(DEFAULT_MAX_TOOL_ROUNDS)
}

/// A tool the chat ran this turn and whether it succeeded.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRun {
    pub tool: String,
    pub succeeded: bool,
}

impl ToolRun {
    /// From the line fed back to the model ("TOOL_RESULT name: {...}" or "TOOL_ERROR ...").
    pub fn from_result_line(tool: &str, line: &str) -> Self {
        let succeeded = line.starts_with("TOOL_RESULT") && !line.contains("\"success\":false");
        Self {
            tool: tool.to_string(),
            succeeded,
        }
    }
}

/// The reply to show: the model's closing words without any tool calls in
/// them, or, if it wrote nothing else, a summary of what was recorded.
pub fn closing_reply(model_reply: Option<&str>, ran: &[ToolRun], held: &[PendingAction]) -> String {
    let words = model_reply.map(without_tool_calls).unwrap_or_default();
    if words.trim().is_empty() {
        summary(ran, held)
    } else {
        words.trim().to_string()
    }
}

fn without_tool_calls(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("TOOL_CALL:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn summary(ran: &[ToolRun], held: &[PendingAction]) -> String {
    let mut lines = vec!["I ran out of steps before writing a full answer.".to_string()];
    let done: Vec<&str> = ran
        .iter()
        .filter(|r| r.succeeded)
        .map(|r| r.tool.as_str())
        .collect();
    let failed: Vec<&str> = ran
        .iter()
        .filter(|r| !r.succeeded)
        .map(|r| r.tool.as_str())
        .collect();
    if !done.is_empty() {
        lines.push(format!("Done: {}.", done.join(", ")));
    }
    if !failed.is_empty() {
        lines.push(format!("Did not succeed: {}.", failed.join(", ")));
    }
    if !held.is_empty() {
        let tools: Vec<&str> = held.iter().map(|a| a.tool.as_str()).collect();
        lines.push(format!(
            "Waiting for your confirmation below: {}.",
            tools.join(", ")
        ));
    }
    if done.is_empty() && failed.is_empty() && held.is_empty() {
        lines.push("Nothing was done. Please try again or rephrase the request.".to_string());
    }
    lines.join(" ")
}
