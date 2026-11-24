// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The agent and the chat assistant reach high-level-only tools (drafting,
//! model settings, jobs) through the dispatcher that has them; before, they
//! went to the low-level one and got "not yet implemented".

use super::mcp_http::{cleanup_test_db, create_test_dashboard_state, setup_test_env};
use rustymail::dashboard::api::handlers::execute_mcp_tool_inner;
use rustymail::dashboard::api::tool_dispatch::{execute, variant_of};
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn high_level_tools_run_through_the_dispatcher_that_has_them() {
    setup_test_env();
    let test_name = "tool_dispatch";
    let state = create_test_dashboard_state(test_name).await;

    // The old route: the low-level dispatcher does not have it.
    let old = execute_mcp_tool_inner(&state, "get_model_configurations", json!({})).await;
    assert!(old.to_string().contains("not yet implemented"), "{old}");

    let new = execute(&state, "get_model_configurations", json!({})).await;
    assert_eq!(new["success"], true, "{new}");

    // Drafting reaches its handler (which reports a missing argument, not "not implemented").
    let draft = execute(&state, "draft_email", json!({})).await;
    assert!(
        !draft.to_string().contains("not yet implemented"),
        "{draft}"
    );
    assert!(!draft.to_string().contains("Unknown tool"), "{draft}");

    // The agent's own dispatch can never start another agent.
    let agent = execute(
        &state,
        "process_email_instructions",
        json!({"instruction": "hi"}),
    )
    .await;
    assert_eq!(agent["success"], false, "{agent}");

    assert_eq!(variant_of("list_accounts"), "low-level");
    assert_eq!(variant_of("draft_reply"), "high-level");
    assert_eq!(variant_of("process_email_instructions"), "high-level");
    cleanup_test_db(test_name);
}
