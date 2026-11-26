// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Verdicts the owner already settled. Mail triage filed as Spam stays in the
//! spam folder, and mail the owner moved into the INBOX (out of the spam
//! folder or Noise) stays there, whatever today's model or rules say: only
//! the owner's correction changes either.
//! The reason names today's verdict so the owner can still correct it.

use super::{Bucket, Proposal};

/// A message triage filed as Spam earlier: it stays in the spam folder.
pub fn stays_filed(proposal: Proposal) -> Proposal {
    if proposal.bucket == Bucket::Spam {
        return proposal;
    }
    Proposal {
        reason: format!(
            "stays in spam, filed by triage earlier; model now says {} ({})",
            proposal.bucket, proposal.reason
        ),
        bucket: Bucket::Spam,
        ..proposal
    }
}

/// A message moved into the INBOX by the owner's review: it is not filed
/// away again. Needs Reply still applies (it keeps the message in view).
pub fn stays_rescued(proposal: Proposal) -> Proposal {
    if matches!(proposal.bucket, Bucket::Keep | Bucket::NeedsReply) {
        return proposal;
    }
    Proposal {
        reason: format!(
            "moved to the INBOX by your review earlier; now says {} ({})",
            proposal.bucket, proposal.reason
        ),
        bucket: Bucket::Keep,
        ..proposal
    }
}
