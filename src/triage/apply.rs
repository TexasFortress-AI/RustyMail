// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Apply stored proposals: move each message to its bucket's folder and
//! verify the move before recording it.
//!
//! Servers answer OK to a move that did nothing (on Oct 2 one reported
//! success and never moved), so every move goes through `MoveVerifier`: the
//! UID must be in the source beforehand, gone afterwards, and the message must
//! be found in the destination by Message-ID. Nothing is ever deleted or
//! expunged folder-wide; Trash is only a move into an existing trash folder.

use super::codes::is_due;
use super::destinations::FolderResolver;
use super::folders::FolderMap;
use super::runner::Client;
use super::store::{StoredProposal, TriageStore};
use super::untag_move::move_untagged;
use super::{Bucket, ProposalSource};
use crate::dashboard::services::cache_prune::remove_cached_message;
use crate::imap::atomic::AtomicImapOperations;
use crate::imap::error::ImapError;
use crate::imap::session::AsyncImapOps;
use crate::imap::types::FlagOperation;
use crate::imap::verified_move::MoveVerifier;
use chrono::{DateTime, Utc};
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    /// Model proposals below this confidence stay put. Rule proposals have no
    /// confidence and always qualify: the rules are deterministic.
    pub min_confidence: f64,
    pub buckets: Vec<Bucket>,
    /// Move at most this many messages per account.
    pub limit: Option<usize>,
}

/// Why a proposal was not moved this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Skip {
    /// Keep, or a bucket the stored row does not name.
    StaysInInbox,
    /// Spam, in the spam-folder review: it is where it belongs.
    StaysInSpam,
    /// The bucket is not in TRIAGE_APPLY_BUCKETS.
    BucketNotApplied,
    LowConfidence,
    /// A code or reset whose window has not passed yet.
    NotYetDue,
    /// UIDVALIDITY changed, so the stored UID may name another message.
    Stale,
}

/// Whether to move an INBOX proposal now, and to which bucket.
pub fn decide(
    proposal: &StoredProposal,
    uidvalidity: u32,
    now: DateTime<Utc>,
    options: &ApplyOptions,
) -> Result<Bucket, Skip> {
    decide_in(proposal, uidvalidity, now, options, Bucket::Keep)
}

/// Whether to move a proposal now, and to which bucket, in a folder where
/// `stays` is the bucket that belongs there: Keep for the INBOX, Spam for the
/// spam folder. In the spam folder, Keep, or a bucket that is not applied,
/// means the INBOX (where an INBOX run would have left it).
pub fn decide_in(
    proposal: &StoredProposal,
    uidvalidity: u32,
    now: DateTime<Utc>,
    options: &ApplyOptions,
    stays: Bucket,
) -> Result<Bucket, Skip> {
    // The owner's correction, when there is one, is the decision.
    let chosen = proposal.correction.as_deref().unwrap_or(&proposal.bucket);
    let in_spam_folder = stays == Bucket::Spam;
    let stay = if in_spam_folder {
        Skip::StaysInSpam
    } else {
        Skip::StaysInInbox
    };
    let bucket = match Bucket::parse(chosen) {
        // Where it belongs, or a bucket the stored row does not name.
        Some(b) if b == stays => return Err(stay),
        None => return Err(stay),
        Some(b) if in_spam_folder && !options.buckets.contains(&b) => Bucket::Keep,
        Some(b) => b,
    };
    if proposal.uidvalidity != uidvalidity {
        return Err(Skip::Stale);
    }
    if bucket != Bucket::Keep && !options.buckets.contains(&bucket) {
        return Err(Skip::BucketNotApplied);
    }
    // A model's doubt doesn't hold back a bucket the owner chose.
    let from_model =
        proposal.source == ProposalSource::Model.as_str() && proposal.correction.is_none();
    if from_model && proposal.confidence.unwrap_or(0.0) < options.min_confidence {
        return Err(Skip::LowConfidence);
    }
    if !is_due(proposal.not_before, now) {
        return Err(Skip::NotYetDue);
    }
    Ok(bucket)
}

/// A move that did not happen, for the report.
#[derive(Debug, Clone)]
pub struct ApplyFailure {
    pub uid: u32,
    pub bucket: String,
    pub error: String,
}

#[derive(Debug, Default)]
pub struct ApplySummary {
    pub account: String,
    /// The folder the messages were moved out of.
    pub folder: String,
    /// Verified: gone from the source and found in the destination.
    pub moved: usize,
    /// Gone from the source, but the destination copy could not be identified
    /// (the message has no Message-ID). Counted separately so it is visible.
    pub moved_unconfirmed: usize,
    pub failures: Vec<ApplyFailure>,
    pub skipped: HashMap<Skip, usize>,
    pub created_folders: Vec<String>,
    /// Rescues whose subject lost the spam filter's tag (see untag_move).
    pub untagged: usize,
    /// Set when the server connection was lost and the run stopped early;
    /// the moves not yet attempted stay pending for the next run.
    pub connection_lost: Option<String>,
}

/// Keywords mail servers and clients use to mark junk, and not-junk. A rescue
/// clears the first and sets the second, so neither the server nor a client
/// such as Thunderbird files the message back into spam.
const JUNK_KEYWORDS: [&str; 2] = ["$Junk", "Junk"];
const NOT_JUNK_KEYWORDS: [&str; 2] = ["$NotJunk", "NonJunk"];

/// Which folder is being applied, and so which bucket stays put there.
#[derive(Debug, Clone, Copy)]
pub enum ApplyIn<'a> {
    /// The INBOX: Keep stays.
    Inbox,
    /// The spam folder: Spam stays, and Keep goes to this INBOX.
    SpamFolder { inbox: &'a str },
}

/// Apply one account's pending proposals for its INBOX `folder`.
pub async fn apply_account(
    store: &TriageStore,
    client: &Client,
    account_id: &str,
    folder: &str,
    folders: &FolderMap,
    options: &ApplyOptions,
) -> Result<ApplySummary, Box<dyn std::error::Error>> {
    apply_folder(
        store,
        client,
        account_id,
        folder,
        ApplyIn::Inbox,
        folders,
        options,
    )
    .await
}

/// Apply one account's pending proposals for `folder`.
pub async fn apply_folder(
    store: &TriageStore,
    client: &Client,
    account_id: &str,
    folder: &str,
    role: ApplyIn<'_>,
    folders: &FolderMap,
    options: &ApplyOptions,
) -> Result<ApplySummary, Box<dyn std::error::Error>> {
    let stays = match role {
        ApplyIn::Inbox => Bucket::Keep,
        ApplyIn::SpamFolder { .. } => Bucket::Spam,
    };
    let mut summary = ApplySummary {
        account: account_id.to_string(),
        folder: folder.to_string(),
        ..Default::default()
    };
    let uidvalidity = client
        .select_folder(folder)
        .await?
        .uid_validity
        .ok_or("server did not report UIDVALIDITY")?;
    let now = Utc::now();
    let mut due = Vec::new();
    for proposal in store.proposals_to_apply(account_id, folder).await? {
        match decide_in(&proposal, uidvalidity, now, options, stays) {
            Ok(bucket) => due.push((proposal, bucket)),
            Err(skip) => *summary.skipped.entry(skip).or_default() += 1,
        }
    }
    if let Some(limit) = options.limit {
        due.truncate(limit);
    }
    if due.is_empty() {
        return Ok(summary);
    }
    if let ApplyIn::SpamFolder { .. } = role {
        let uids: Vec<u32> = due.iter().map(|(p, _)| p.uid).collect();
        clear_junk_marks(client, account_id, &uids).await;
    }

    let mut destinations = FolderResolver::new(client, account_id, folders).await?;
    for (proposal, bucket) in due {
        let resolved = match role {
            ApplyIn::SpamFolder { inbox } if bucket == Bucket::Keep => Ok(inbox.to_string()),
            _ => {
                destinations
                    .folder_for(bucket, proposal.list_name.as_deref())
                    .await
            }
        };
        let mut lost_connection = false;
        let outcome = match resolved {
            Ok(target) => {
                match move_one(client, role, proposal.uid, folder, &target, &mut summary).await {
                    Ok(confirmed) => Ok((target, confirmed)),
                    Err(e) => {
                        lost_connection = matches!(e, ImapError::Connection(_));
                        Err(e.to_string())
                    }
                }
            }
            Err(reason) => Err(reason),
        };
        match outcome {
            Ok((target, confirmed)) => {
                store.mark_applied(proposal.id, &target).await?;
                // The cache would otherwise show it in the INBOX until a sync prunes it.
                if let Err(e) =
                    remove_cached_message(store.pool(), account_id, folder, proposal.uid).await
                {
                    warn!(
                        "{} uid {}: moved, but not removed from the cache: {}",
                        account_id, proposal.uid, e
                    );
                }
                if confirmed {
                    summary.moved += 1;
                } else {
                    summary.moved_unconfirmed += 1;
                }
            }
            Err(error) => {
                warn!(
                    "{} uid {} -> {}: move failed",
                    account_id, proposal.uid, bucket
                );
                store.mark_failed(proposal.id, &error).await?;
                if lost_connection {
                    warn!("{}: connection lost, stopping: {}", account_id, error);
                    summary.connection_lost = Some(error.clone());
                }
                summary.failures.push(ApplyFailure {
                    uid: proposal.uid,
                    bucket: bucket.to_string(),
                    error,
                });
                if lost_connection {
                    break;
                }
            }
        }
    }
    summary.created_folders = destinations.created;
    info!(
        "{}: moved {}, unconfirmed {}, failed {}",
        account_id,
        summary.moved,
        summary.moved_unconfirmed,
        summary.failures.len()
    );
    Ok(summary)
}

/// Move one message: in the INBOX a verified move; out of the spam folder an
/// untagged copy when its subject carries the spam filter's tag.
async fn move_one(
    client: &Client,
    role: ApplyIn<'_>,
    uid: u32,
    from: &str,
    to: &str,
    summary: &mut ApplySummary,
) -> Result<bool, ImapError> {
    if let ApplyIn::SpamFolder { .. } = role {
        if let Some(confirmed) = move_untagged(client, uid, from, to).await? {
            summary.untagged += 1;
            return Ok(confirmed);
        }
    }
    verified_move(client, uid, from, to).await
}

/// Clear the junk keywords on messages about to leave the selected spam
/// folder and mark them not-junk. Best effort: a server that refuses custom
/// keywords still gets the move, and the refusal is logged.
async fn clear_junk_marks(client: &Client, account_id: &str, uids: &[u32]) {
    let junk: Vec<String> = JUNK_KEYWORDS.iter().map(|k| k.to_string()).collect();
    let not_junk: Vec<String> = NOT_JUNK_KEYWORDS.iter().map(|k| k.to_string()).collect();
    if let Err(e) = client.store_flags(uids, FlagOperation::Remove, &junk).await {
        warn!("{}: could not clear junk keywords: {}", account_id, e);
    }
    if let Err(e) = client
        .store_flags(uids, FlagOperation::Add, &not_junk)
        .await
    {
        warn!("{}: could not set not-junk keywords: {}", account_id, e);
    }
}

/// Move one message and check it left the source and reached the destination.
/// Returns whether the destination copy was positively identified.
pub(crate) async fn verified_move(
    client: &Client,
    uid: u32,
    from: &str,
    to: &str,
) -> Result<bool, ImapError> {
    let session = client.session_arc();
    let verifier = MoveVerifier::new(session.clone() as Arc<dyn AsyncImapOps>);
    let pending = verifier.prepare(uid, from, to).await?;
    AtomicImapOperations::new((*session).clone())
        .atomic_move(uid, from, to)
        .await?;
    verifier.confirm(&pending, from, to).await
}
