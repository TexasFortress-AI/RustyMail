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
use super::{Bucket, ProposalSource};
use crate::dashboard::services::cache_prune::remove_cached_message;
use crate::imap::atomic::AtomicImapOperations;
use crate::imap::error::ImapError;
use crate::imap::session::AsyncImapOps;
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
    /// Set when the server connection was lost and the run stopped early;
    /// the moves not yet attempted stay pending for the next run.
    pub connection_lost: Option<String>,
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

    let mut destinations = FolderResolver::new(client, account_id, folders).await?;
    for (proposal, bucket) in due {
        let resolved = destinations
            .folder_for(bucket, proposal.list_name.as_deref())
            .await;
        let mut lost_connection = false;
        let outcome = match resolved {
            Ok(target) => match verified_move(client, proposal.uid, folder, &target).await {
                Ok(confirmed) => Ok((target, confirmed)),
                Err(e) => {
                    lost_connection = matches!(e, ImapError::Connection(_));
                    Err(e.to_string())
                }
            },
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
