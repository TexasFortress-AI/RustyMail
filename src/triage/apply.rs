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
use super::folders::{special_use, Destination, FolderMap};
use super::runner::Client;
use super::store::{StoredProposal, TriageStore};
use super::{Bucket, ProposalSource};
use crate::dashboard::services::cache_prune::remove_cached_message;
use crate::imap::atomic::AtomicImapOperations;
use crate::imap::error::ImapError;
use crate::imap::session::AsyncImapOps;
use crate::imap::special_folders::marked_folder;
use crate::imap::types::Folder;
use crate::imap::verified_move::MoveVerifier;
use chrono::{DateTime, Utc};
use log::{info, warn};
use std::collections::hash_map::Entry;
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
    /// The bucket is not in TRIAGE_APPLY_BUCKETS.
    BucketNotApplied,
    LowConfidence,
    /// A code or reset whose window has not passed yet.
    NotYetDue,
    /// UIDVALIDITY changed, so the stored UID may name another message.
    Stale,
}

/// Whether to move this proposal now, and to which bucket.
pub fn decide(
    proposal: &StoredProposal,
    uidvalidity: u32,
    now: DateTime<Utc>,
    options: &ApplyOptions,
) -> Result<Bucket, Skip> {
    let bucket = Bucket::parse(&proposal.bucket)
        .filter(|b| *b != Bucket::Keep)
        .ok_or(Skip::StaysInInbox)?;
    if proposal.uidvalidity != uidvalidity {
        return Err(Skip::Stale);
    }
    if !options.buckets.contains(&bucket) {
        return Err(Skip::BucketNotApplied);
    }
    let from_model = proposal.source == ProposalSource::Model.as_str();
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
    /// Verified: gone from the source and found in the destination.
    pub moved: usize,
    /// Gone from the source, but the destination copy could not be identified
    /// (the message has no Message-ID). Counted separately so it is visible.
    pub moved_unconfirmed: usize,
    pub failures: Vec<ApplyFailure>,
    pub skipped: HashMap<Skip, usize>,
    pub created_folders: Vec<String>,
}

/// Apply one account's pending proposals for `folder`.
pub async fn apply_account(
    store: &TriageStore,
    client: &Client,
    account_id: &str,
    folder: &str,
    folders: &FolderMap,
    options: &ApplyOptions,
) -> Result<ApplySummary, Box<dyn std::error::Error>> {
    let mut summary = ApplySummary {
        account: account_id.to_string(),
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
        match decide(&proposal, uidvalidity, now, options) {
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

    let mut existing = client.list_folders().await?;
    let tree = client.session().list_folders_hierarchical().await?;
    let mut destinations: HashMap<Bucket, Result<String, String>> = HashMap::new();
    for (proposal, bucket) in due {
        let resolved = match destinations.entry(bucket) {
            Entry::Occupied(known) => known.into_mut(),
            Entry::Vacant(slot) => slot.insert(
                destination(
                    client,
                    account_id,
                    bucket,
                    folders,
                    &mut existing,
                    &tree,
                    &mut summary,
                )
                .await,
            ),
        };
        let outcome = match resolved {
            Ok(target) => verified_move(client, proposal.uid, folder, target)
                .await
                .map(|confirmed| (target.clone(), confirmed))
                .map_err(|e| e.to_string()),
            Err(reason) => Err(reason.clone()),
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
                summary.failures.push(ApplyFailure {
                    uid: proposal.uid,
                    bucket: bucket.to_string(),
                    error,
                });
            }
        }
    }
    info!(
        "{}: moved {}, unconfirmed {}, failed {}",
        account_id,
        summary.moved,
        summary.moved_unconfirmed,
        summary.failures.len()
    );
    Ok(summary)
}

/// Resolve a bucket's folder, creating it when allowed (never a trash folder).
async fn destination(
    client: &Client,
    account_id: &str,
    bucket: Bucket,
    folders: &FolderMap,
    existing: &mut Vec<String>,
    tree: &[Folder],
    summary: &mut ApplySummary,
) -> Result<String, String> {
    let marked = special_use(bucket).and_then(|kind| marked_folder(tree, kind));
    match folders.resolve(account_id, bucket, existing, marked.as_deref()) {
        Destination::Existing(name) => Ok(name),
        Destination::Unavailable(reason) => Err(reason),
        Destination::Create(name) => {
            client
                .create_folder(&name)
                .await
                .map_err(|e| format!("cannot create folder {}: {}", name, e))?;
            info!("{}: created folder {}", account_id, name);
            existing.push(name.clone());
            summary.created_folders.push(name.clone());
            Ok(name)
        }
    }
}

/// Move one message and check it left the source and reached the destination.
/// Returns whether the destination copy was positively identified.
async fn verified_move(client: &Client, uid: u32, from: &str, to: &str) -> Result<bool, ImapError> {
    let session = client.session_arc();
    let verifier = MoveVerifier::new(session.clone() as Arc<dyn AsyncImapOps>);
    let pending = verifier.prepare(uid, from, to).await?;
    AtomicImapOperations::new((*session).clone())
        .atomic_move(uid, from, to)
        .await?;
    verifier.confirm(&pending, from, to).await
}
