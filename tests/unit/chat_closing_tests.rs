// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The chat assistant's reply is never empty when its tool rounds run out.

use rustymail::dashboard::services::ai::chat_closing::{
    closing_reply, max_tool_rounds, ToolRun, MAX_TOOL_ROUNDS_ENV,
};
use rustymail::dashboard::services::ai::chat_tool_gate::PendingAction;
use serde_json::json;
use serial_test::serial;

fn ran(tool: &str, line: &str) -> ToolRun {
    ToolRun::from_result_line(tool, line)
}

#[test]
fn the_model_s_closing_words_are_used_without_tool_calls() {
    let reply = "I saved the draft to Drafts.\nTOOL_CALL: draft_email {\"to\": \"x\"}";
    assert_eq!(
        closing_reply(Some(reply), &[], &[]),
        "I saved the draft to Drafts."
    );
}

#[test]
fn with_no_words_from_the_model_the_reply_says_what_happened() {
    let runs = vec![
        ran("draft_email", "TOOL_ERROR draft_email: missing 'context'"),
        ran(
            "draft_email",
            "TOOL_RESULT draft_email: {\"success\":true,\"data\":{}}",
        ),
    ];
    let held = vec![PendingAction {
        id: "1".into(),
        tool: "send_email".into(),
        variant: "low-level".into(),
        parameters: json!({}),
        affected: vec![],
    }];
    for nothing in [None, Some(""), Some("TOOL_CALL: draft_email {}")] {
        let text = closing_reply(nothing, &runs, &held);
        assert!(text.contains("Done: draft_email."), "{text}");
        assert!(text.contains("Did not succeed: draft_email."), "{text}");
        assert!(
            text.contains("Waiting for your confirmation below: send_email."),
            "{text}"
        );
    }
    let empty = closing_reply(None, &[], &[]);
    assert!(empty.contains("Nothing was done"), "{empty}");
}

#[test]
fn a_tool_that_reported_failure_counts_as_not_succeeded() {
    assert!(!ran("x", "TOOL_RESULT x: {\"success\":false,\"error\":\"nope\"}").succeeded);
    assert!(ran("x", "TOOL_RESULT x: {\"success\":true}").succeeded);
    assert!(!ran("x", "TOOL_ERROR x: boom").succeeded);
}

#[test]
#[serial]
fn the_round_limit_comes_from_the_environment() {
    std::env::set_var(MAX_TOOL_ROUNDS_ENV, "8");
    assert_eq!(max_tool_rounds(), 8);
    for bad in ["0", "lots"] {
        std::env::set_var(MAX_TOOL_ROUNDS_ENV, bad);
        assert_eq!(max_tool_rounds(), 5, "{bad}");
    }
    std::env::remove_var(MAX_TOOL_ROUNDS_ENV);
    assert_eq!(max_tool_rounds(), 5);
}
