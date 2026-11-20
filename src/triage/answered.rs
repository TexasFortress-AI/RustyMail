// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Needs Reply holds only mail still waiting on the owner. A message counts
//! as answered when the server marks it `\Answered` (mail clients set it when
//! you reply) or when sent mail names it in In-Reply-To or References (a reply
//! from a phone or another app that sets no flag). Answered mail is never
//! proposed for Needs Reply, and answered mail already in the Needs Reply
//! folder goes back to the INBOX.

use super::apply::verified_move;
use super::folders::{bucket_namespace, Destination, FolderMap};
use super::message::TriageMessage;
use super::people::KnownPeople;
use super::runner::Client;
use super::Bucket;
use crate::imap::session::AsyncImapOps;
use crate::imap::special_folders::inbox_namespace;
use log::{info, warn};
use std::collections::HashSet;

/// `\Answered` as `fetch_flags` reports it (the Debug form of the flag).
pub const ANSWERED_FLAG: &str = "Answered";

/// Characters of body text read when parsing a message only for its ID.
const ID_ONLY_EXCERPT_CHARS: usize = 0;

/// UIDs among `uids` (in the selected folder) the server marks `\Answered`.
/// A server whose flags cannot be read yields none: unknown is not answered.
pub async fn answered_uids(client: &Client, uids: &[u32]) -> HashSet<u32> {
    if uids.is_empty() {
        return HashSet::new();
    }
    match client.fetch_flags(uids).await {
        Ok(flags) => flags
            .into_iter()
            .filter(|(_, flags)| flags.iter().any(|f| f == ANSWERED_FLAG))
            .map(|(uid, _)| uid)
            .collect(),
        Err(e) => {
            warn!("Cannot read flags; treating messages as unanswered: {}", e);
            HashSet::new()
        }
    }
}

/// Move answered mail out of the account's Needs Reply folder into `inbox`.
/// Returns how many moved; an account without the folder has nothing to do.
pub async fn release_answered(
    client: &Client,
    account_id: &str,
    folders: &FolderMap,
    known: &KnownPeople,
    inbox: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let Some(folder) = needs_reply_folder(client, account_id, folders).await? else {
        return Ok(0);
    };
    client.select_folder(&folder).await?;
    let mut uids = client.search_emails("ALL").await?;
    uids.sort_unstable();
    // Decide every message before moving any: a verified move leaves the
    // destination selected, so reads after it would hit the wrong folder.
    let flagged = answered_uids(client, &uids).await;
    let mut answered = Vec::new();
    for uid in uids {
        if flagged.contains(&uid) || replied_in_sent_mail(client, uid, known).await {
            answered.push(uid);
        }
    }
    let mut moved = 0;
    for uid in answered {
        match verified_move(client, uid, &folder, inbox).await {
            Ok(_) => moved += 1,
            Err(e) => warn!("{}: answered uid {} not moved: {}", account_id, uid, e),
        }
    }
    if moved > 0 {
        info!(
            "{}: {} answered message(s) back to {}",
            account_id, moved, inbox
        );
    }
    Ok(moved)
}

/// The existing Needs Reply folder, never created here.
async fn needs_reply_folder(
    client: &Client,
    account_id: &str,
    folders: &FolderMap,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let existing = client.list_folders().await?;
    let tree = client.session().list_folders_hierarchical().await?;
    let namespace = bucket_namespace(&existing, &inbox_namespace(&tree));
    Ok(
        match folders.resolve(account_id, Bucket::NeedsReply, &existing, None, &namespace) {
            Destination::Existing(name) => Some(name),
            _ => None,
        },
    )
}

/// Whether sent mail replies to this message, by its Message-ID.
async fn replied_in_sent_mail(client: &Client, uid: u32, known: &KnownPeople) -> bool {
    let Ok(raw) = client.fetch_raw_message(uid).await else {
        return false;
    };
    TriageMessage::from_raw(uid, &raw, ID_ONLY_EXCERPT_CHARS)
        .is_some_and(|message| known.replied_to(message.message_id.as_deref()))
}
