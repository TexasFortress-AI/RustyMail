// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Confirms a move really happened. IMAP servers answer OK to `UID MOVE` for a UID that
//! does not exist, so the server's response alone cannot be reported as success.
//!
//! The destination copy is found by comparing Message-IDs locally on UIDs at or above the
//! destination's pre-move UIDNEXT. `SEARCH HEADER Message-ID` is deliberately not used: some
//! production servers return no hits for it, and the ID is untrusted sender-supplied text.

use std::sync::Arc;

use log::warn;

use super::{error::ImapError, session::AsyncImapOps};

/// State captured before a move, compared against the mailbox afterwards.
pub struct PendingMove {
    uid: u32,
    message_id: Option<String>,
    destination_uid_next: Option<u32>,
}

/// Checks a single message's location before and after a move, over the given session.
pub struct MoveVerifier {
    session: Arc<dyn AsyncImapOps>,
}

impl MoveVerifier {
    pub fn new(session: Arc<dyn AsyncImapOps>) -> Self {
        Self { session }
    }

    /// Fails unless the UID is in `from_folder` and `to_folder` can be selected.
    pub async fn prepare(
        &self,
        uid: u32,
        from_folder: &str,
        to_folder: &str,
    ) -> Result<PendingMove, ImapError> {
        self.session.select_folder(from_folder).await?;
        if !self.selected_folder_contains(uid).await? {
            return Err(ImapError::MissingData(format!(
                "Message UID {} not found in {}",
                uid, from_folder
            )));
        }
        let message_id = self.message_id_or_none(uid).await;
        let destination = self.session.select_folder(to_folder).await?;
        Ok(PendingMove {
            uid,
            message_id,
            destination_uid_next: destination.uid_next,
        })
    }

    /// Returns whether the destination copy was positively identified. `false` means the source
    /// is gone but the copy could not be identified (e.g. the message has no Message-ID).
    pub async fn confirm(
        &self,
        pending: &PendingMove,
        from_folder: &str,
        to_folder: &str,
    ) -> Result<bool, ImapError> {
        self.session.select_folder(from_folder).await?;
        if self.selected_folder_contains(pending.uid).await? {
            return Err(ImapError::Other(format!(
                "Server reported success but UID {} is still in {}",
                pending.uid, from_folder
            )));
        }
        let (Some(message_id), Some(uid_next)) =
            (&pending.message_id, pending.destination_uid_next)
        else {
            return Ok(false);
        };

        self.session.select_folder(to_folder).await?;
        for uid in self.uids_from(uid_next).await? {
            if self.message_id_or_none(uid).await.as_ref() == Some(message_id) {
                return Ok(true);
            }
        }
        Err(ImapError::Other(format!(
            "UID {} left {} but was not found in {}",
            pending.uid, from_folder, to_folder
        )))
    }

    async fn selected_folder_contains(&self, uid: u32) -> Result<bool, ImapError> {
        let matches = self.session.search_emails(&format!("UID {}", uid)).await?;
        Ok(matches.contains(&uid))
    }

    /// `UID n:*` also returns the newest message when nothing is at or above `n`, so filter.
    async fn uids_from(&self, uid_next: u32) -> Result<Vec<u32>, ImapError> {
        let matches = self
            .session
            .search_emails(&format!("UID {}:*", uid_next))
            .await?;
        Ok(matches.into_iter().filter(|uid| *uid >= uid_next).collect())
    }

    /// An unreadable Message-ID only limits verification, so it must not fail the move.
    async fn message_id_or_none(&self, uid: u32) -> Option<String> {
        self.session
            .fetch_message_id(uid)
            .await
            .unwrap_or_else(|e| {
                warn!("Could not read Message-ID of UID {}: {}", uid, e);
                None
            })
    }
}
