// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The current flags of a folder's messages, for keeping the cache's
//! read/unread (and other) flags in step with the server.
//!
//! The message sync fetches bodies without FLAGS (some servers send FLAGS the
//! parser rejects), and flags change on messages already cached whenever the
//! owner reads, answers or flags mail in another client. So flags are fetched
//! on their own, FLAGS only:
//!
//! - With CONDSTORE (RFC 7162), `SELECT ... (CONDSTORE)` reports the folder's
//!   HIGHESTMODSEQ. If it equals the value from the last refresh, nothing
//!   changed and nothing is fetched; otherwise only messages changed since
//!   then are fetched (`CHANGEDSINCE`).
//! - Without it, every message's flags are fetched.
//!
//! A FLAGS reply the client cannot parse breaks the connection's response
//! stream (it surfaces as a connection error, with the bad bytes still
//! buffered), so nothing is retried on it: the error is returned, the caller
//! keeps its stored HIGHESTMODSEQ, and should not reuse the connection.
//!
//! Flags are named as everywhere else in RustyMail (`format!("{:?}", flag)`):
//! "Seen", "Answered", "Flagged", "Deleted", "Draft", `Custom("$Junk")`.

use super::error::ImapError;
use super::session::TlsImapSession;
use async_imap::types::Fetch;
use futures_util::stream::TryStreamExt;
use std::collections::HashSet;

const CONDSTORE_CAPABILITY: &str = "CONDSTORE";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FlagChanges {
    /// The folder's HIGHESTMODSEQ when the server supports CONDSTORE: store it
    /// once `flags` are saved, and pass it back as `since` next time.
    pub highest_modseq: Option<u64>,
    /// (UID, flags) of every message whose flags may have changed: all of the
    /// folder's messages when no usable `since` was given.
    pub flags: Vec<(u32, Vec<String>)>,
}

/// Selects `folder` and returns the flags changed since `since` (a
/// HIGHESTMODSEQ from an earlier call), or all flags when `since` is None
/// or the server has no CONDSTORE.
pub async fn fetch_flag_changes(
    session: &mut TlsImapSession,
    folder: &str,
    since: Option<u64>,
) -> Result<FlagChanges, ImapError> {
    let condstore = session
        .capabilities()
        .await
        .map_err(ImapError::from)?
        .has_str(CONDSTORE_CAPABILITY);
    let mailbox = if condstore {
        session.select_condstore(folder).await
    } else {
        session.select(folder).await
    }
    .map_err(ImapError::from)?;
    // A mailbox without HIGHESTMODSEQ (NOMODSEQ) is treated as no CONDSTORE.
    let highest_modseq = mailbox.highest_modseq.filter(|_| condstore);
    let unchanged = FlagChanges {
        highest_modseq,
        flags: Vec::new(),
    };
    if mailbox.exists == 0 {
        return Ok(unchanged);
    }
    let query = match (highest_modseq, since) {
        (Some(now), Some(then)) if now == then => return Ok(unchanged),
        (Some(_), Some(then)) => format!("(UID FLAGS) (CHANGEDSINCE {then})"),
        _ => "(UID FLAGS)".to_string(),
    };
    let flags = fetch(session, "1:*", &query).await?;
    Ok(FlagChanges {
        highest_modseq,
        flags,
    })
}

async fn fetch(
    session: &mut TlsImapSession,
    uid_set: &str,
    query: &str,
) -> Result<Vec<(u32, Vec<String>)>, ImapError> {
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_set, query)
        .await
        .map_err(ImapError::from)?
        .try_collect()
        .await
        .map_err(ImapError::from)?;
    Ok(fetches
        .iter()
        .filter_map(|fetch| Some((fetch.uid?, flag_names(fetch))))
        .collect())
}

/// A message's flags, named as RustyMail stores them, without duplicates.
pub fn flag_names(fetch: &Fetch) -> Vec<String> {
    let mut seen = HashSet::new();
    fetch
        .flags()
        .map(|flag| format!("{:?}", flag))
        .filter(|name| seen.insert(name.clone()))
        .collect()
}
