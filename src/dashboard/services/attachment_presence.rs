// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Whether a message's attachments can be served from disk.
//!
//! Sync records every attachment it sees as metadata only (an empty
//! `storage_path`); the file is written when the message is first fetched
//! with its attachments. Code that skips the IMAP fetch because rows exist
//! must check that the files exist, or it serves nothing.

use super::attachment_storage::AttachmentInfo;

/// True when there are attachments and every one has been saved to disk.
pub fn all_downloaded(attachments: &[AttachmentInfo]) -> bool {
    !attachments.is_empty() && attachments.iter().all(|a| !a.storage_path.is_empty())
}
