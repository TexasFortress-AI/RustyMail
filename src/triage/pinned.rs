// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Verdicts the owner already settled. Mail triage filed as Spam stays in the
//! spam folder, and mail rescued from it is never proposed as Spam again,
//! whatever today's model says: only the owner's correction changes either.
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

/// A message rescued from the spam folder earlier: it is not sent back.
pub fn stays_rescued(proposal: Proposal) -> Proposal {
    if proposal.bucket != Bucket::Spam {
        return proposal;
    }
    Proposal {
        reason: format!(
            "rescued from spam earlier; model now says Spam ({})",
            proposal.reason
        ),
        bucket: Bucket::Keep,
        ..proposal
    }
}
