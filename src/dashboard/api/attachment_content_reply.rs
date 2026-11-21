// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The success reply of the get_attachment_content tool.
//!
//! MCP clients receive only a tool reply's `data` (see api/mcp_http.rs), so
//! the attachment goes there; without it a client got `null` for every
//! attachment it fetched.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};

/// `{"success": true, "data": {filename, content_type, size_bytes,
/// content_base64, message_id}, "tool": tool_name}`
pub fn attachment_content_reply(
    tool_name: &str,
    filename: &str,
    content_type: &str,
    content: &[u8],
    message_id: &str,
) -> Value {
    json!({
        "success": true,
        "data": {
            "filename": filename,
            "content_type": content_type,
            "size_bytes": content.len(),
            "content_base64": BASE64.encode(content),
            "message_id": message_id,
        },
        "tool": tool_name
    })
}
