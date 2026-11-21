// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! get_attachment_content puts the attachment in `data`, the only part of a
//! tool reply MCP clients receive.

use rustymail::dashboard::api::attachment_content_reply::attachment_content_reply;

#[test]
fn the_attachment_is_in_the_data_an_mcp_client_receives() {
    let reply = attachment_content_reply(
        "get_attachment_content",
        "invoice.pdf",
        "application/pdf",
        b"%PDF-1.7",
        "<m1@example.test>",
    );
    assert_eq!(reply["success"], true);
    let data = &reply["data"];
    assert_eq!(data["filename"], "invoice.pdf");
    assert_eq!(data["content_type"], "application/pdf");
    assert_eq!(data["size_bytes"], 8);
    assert_eq!(data["content_base64"], "JVBERi0xLjc=");
    assert_eq!(data["message_id"], "<m1@example.test>");
}
