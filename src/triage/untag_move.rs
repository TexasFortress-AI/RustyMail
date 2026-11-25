// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Rescuing a message whose subject carries the spam filter's tag. IMAP
//! cannot edit a stored message, so the rescue APPENDs a copy without the
//! tag (same flags, same arrival time) to the destination, confirms by
//! Message-ID that the copy arrived, and only then deletes the original (a
//! targeted UID delete) and confirms it left. If the copy is not found, the
//! original is left where it was.

use super::runner::Client;
use super::spam_tag::untag_message;
use crate::imap::error::ImapError;
use crate::imap::flag_changes::wire_flag;
use crate::imap::session::AsyncImapOps;
use crate::imap::verified_move::MoveVerifier;
use std::sync::Arc;

/// IMAP date-time, as INTERNALDATE takes it.
const INTERNAL_DATE_FORMAT: &str = "%d-%b-%Y %H:%M:%S %z";

/// Move `uid` from `from` to `to` as an untagged copy. Ok(None) when the
/// subject has no tag or the copy could not be identified afterwards (no
/// Message-ID): the caller then moves the message as it is. Ok(confirmed)
/// otherwise, as a verified move reports it.
pub async fn move_untagged(
    client: &Client,
    uid: u32,
    from: &str,
    to: &str,
) -> Result<Option<bool>, ImapError> {
    client.select_folder(from).await?;
    let Some(original) = client.fetch_emails(&[uid]).await?.into_iter().next() else {
        return Ok(None);
    };
    let Some(untagged) = original.body.as_deref().and_then(untag_message) else {
        return Ok(None);
    };
    let flags: Vec<String> = client
        .fetch_flags(&[uid])
        .await?
        .into_iter()
        .flat_map(|(_, names)| names)
        .filter_map(|name| wire_flag(&name))
        .collect();

    let verifier = MoveVerifier::new(client.session_arc() as Arc<dyn AsyncImapOps>);
    let pending = verifier.prepare(uid, from, to).await?;
    if !pending.identifiable() {
        return Ok(None);
    }
    match original.internal_date {
        Some(date) => {
            let date = date.format(INTERNAL_DATE_FORMAT).to_string();
            client.append_dated(to, &untagged, &flags, &date).await?
        }
        None => client.append(to, &untagged, &flags).await?,
    }
    if verifier.arrived(&pending, to).await? != Some(true) {
        return Err(ImapError::Other(format!(
            "the untagged copy of UID {} was not found in {}; the original stays in {}",
            uid, to, from
        )));
    }
    client.select_folder(from).await?;
    client.delete_messages(&[uid]).await?;
    verifier.confirm(&pending, from, to).await.map(Some)
}
