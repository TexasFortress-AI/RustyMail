// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! MCP clients, and RustyMail's own agent, build tool calls from these schemas.
//! Every handler reads `uids` as an array of numbers, so a schema that declares
//! a string makes a client that follows it fail with "Missing 'uids' parameter".

use rustymail::dashboard::api::handlers::get_mcp_tools_jsonrpc_format;
use rustymail::dashboard::api::high_level_tools::get_mcp_high_level_tools_jsonrpc_format;

#[test]
fn every_uids_parameter_is_declared_as_an_array_of_integers() {
    let tools = get_mcp_tools_jsonrpc_format()
        .into_iter()
        .chain(get_mcp_high_level_tools_jsonrpc_format());
    let mut checked = 0;
    for tool in tools {
        let Some(uids) = tool["inputSchema"]["properties"].get("uids") else {
            continue;
        };
        checked += 1;
        assert_eq!(uids["type"], "array", "{}: {uids}", tool["name"]);
        assert_eq!(uids["items"]["type"], "integer", "{}: {uids}", tool["name"]);
    }
    assert!(checked >= 7, "expected the uids tools, found {checked}");
}
