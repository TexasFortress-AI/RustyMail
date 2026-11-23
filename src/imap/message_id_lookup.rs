// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Find a message by its Message-ID in the selected folder.
//!
//! `SEARCH HEADER Message-ID` cannot be trusted: some servers (the mail.*
//! hosts this project uses) answer it with no hits for a message that is
//! there. This reads each message's Message-ID from its header block
//! (`BODY.PEEK[HEADER]`, which neither marks it read nor trips the ENVELOPE
//! parser) instead, so it is meant for small folders such as the Outbox.

use super::client::ImapClient;
use super::error::ImapError;
use super::session::{AsyncImapOps, AsyncImapSessionWrapper};

/// UIDs in the selected folder whose Message-ID is `message_id`
/// (angle brackets optional on either side).
pub async fn uids_with_message_id(
    client: &ImapClient<AsyncImapSessionWrapper>,
    message_id: &str,
) -> Result<Vec<u32>, ImapError> {
    let wanted = bare(message_id);
    let session = client.session_arc();
    let mut matches = Vec::new();
    for uid in client.search_emails("ALL").await? {
        if let Some(found) = session.fetch_message_id(uid).await? {
            if bare(&found) == wanted {
                matches.push(uid);
            }
        }
    }
    Ok(matches)
}

fn bare(message_id: &str) -> &str {
    message_id
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
}
