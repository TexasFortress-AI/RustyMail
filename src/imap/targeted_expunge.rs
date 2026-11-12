// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Permanently removes only the messages an operation targeted.
//!
//! A plain `EXPUNGE` removes every message in the selected folder that has
//! the `\Deleted` flag, including ones the user (or another client) marked
//! but never meant to purge yet. Moves and deletes therefore use
//! `UID EXPUNGE` (RFC 4315, UIDPLUS), which removes only the given UIDs.
//! Without UIDPLUS, a plain `EXPUNGE` runs only if no other message in the
//! folder is marked `\Deleted`; otherwise the targets are left flagged.

use std::collections::HashSet;

use futures_util::stream::TryStreamExt;
use log::warn;

use crate::imap::error::ImapError;
use crate::imap::session::TlsImapSession;

/// The capability that provides `UID EXPUNGE`.
const UIDPLUS_CAPABILITY: &str = "UIDPLUS";

/// How the targeted messages will be removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpungePlan {
    /// `UID EXPUNGE` of exactly the targets.
    UidExpunge,
    /// Plain `EXPUNGE`: the targets are the only `\Deleted` messages.
    PlainExpunge,
    /// Not expunged: other messages in the folder are also marked
    /// `\Deleted`, and a plain `EXPUNGE` would remove them too.
    LeaveFlagged { other_deleted: usize },
}

/// Chooses a removal that cannot touch messages outside `targets`.
/// `deleted_in_folder` is only consulted when UIDPLUS is unavailable.
pub fn plan_targeted_expunge(
    has_uidplus: bool,
    deleted_in_folder: &HashSet<u32>,
    targets: &[u32],
) -> ExpungePlan {
    if has_uidplus {
        return ExpungePlan::UidExpunge;
    }
    let targets: HashSet<u32> = targets.iter().copied().collect();
    let other_deleted = deleted_in_folder.difference(&targets).count();
    if other_deleted == 0 {
        ExpungePlan::PlainExpunge
    } else {
        ExpungePlan::LeaveFlagged { other_deleted }
    }
}

/// Expunges only `uids` (already marked `\Deleted`) from the selected folder.
pub async fn expunge_only(
    session: &mut TlsImapSession,
    uids: &[u32],
) -> Result<ExpungePlan, ImapError> {
    if uids.is_empty() {
        return Ok(ExpungePlan::UidExpunge);
    }
    let has_uidplus = session
        .capabilities()
        .await
        .map_err(ImapError::from)?
        .has_str(UIDPLUS_CAPABILITY);
    let deleted_in_folder = if has_uidplus {
        HashSet::new()
    } else {
        session
            .uid_search("DELETED")
            .await
            .map_err(ImapError::from)?
    };
    let plan = plan_targeted_expunge(has_uidplus, &deleted_in_folder, uids);
    match &plan {
        ExpungePlan::UidExpunge => {
            let uid_set = uid_set(uids);
            let stream = session
                .uid_expunge(&uid_set)
                .await
                .map_err(|e| ImapError::Other(format!("Failed to UID EXPUNGE: {}", e)))?;
            stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| ImapError::Other(format!("Failed to process UID EXPUNGE: {}", e)))?;
        }
        ExpungePlan::PlainExpunge => {
            let stream = session
                .expunge()
                .await
                .map_err(|e| ImapError::Other(format!("Failed to expunge: {}", e)))?;
            stream
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| ImapError::Other(format!("Failed to process expunge: {}", e)))?;
        }
        ExpungePlan::LeaveFlagged { other_deleted } => warn!(
            "Server lacks UIDPLUS and {} other message(s) are marked \\Deleted; \
             leaving {} message(s) flagged instead of expunging the whole folder",
            other_deleted,
            uids.len()
        ),
    }
    Ok(plan)
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(|uid| uid.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
