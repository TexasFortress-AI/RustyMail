// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Settled verdicts: mail triage filed as Spam stays in the spam folder, and
//! mail rescued from it is not proposed as Spam again; other verdicts pass.

use rustymail::triage::pinned::{stays_filed, stays_rescued};
use rustymail::triage::{Bucket, Proposal, ProposalSource};

fn model(bucket: Bucket) -> Proposal {
    Proposal {
        bucket,
        confidence: Some(0.95),
        source: ProposalSource::Model,
        model_id: Some("m".into()),
        reason: "stub".into(),
        not_before: None,
        list_name: None,
        document: None,
    }
}

#[test]
fn mail_the_owner_moved_to_the_inbox_is_not_filed_away_again() {
    let p = stays_rescued(model(Bucket::Spam));
    assert_eq!(p.bucket, Bucket::Keep);
    assert_eq!(
        p.reason,
        "moved to the INBOX by your review earlier; now says Spam (stub)"
    );
    // A team meeting recap the owner pulled out of Noise does not go back.
    assert_eq!(stays_rescued(model(Bucket::Noise)).bucket, Bucket::Keep);
    assert_eq!(stays_rescued(model(Bucket::Alerts)).bucket, Bucket::Keep);
    // Needs Reply keeps it in view, so it still applies.
    assert_eq!(
        stays_rescued(model(Bucket::NeedsReply)).bucket,
        Bucket::NeedsReply
    );
}

#[test]
fn mail_triage_filed_as_spam_stays_there() {
    let p = stays_filed(model(Bucket::Noise));
    assert_eq!(p.bucket, Bucket::Spam);
    assert!(
        p.reason.contains("model now says Noise (stub)"),
        "{}",
        p.reason
    );
    assert_eq!(stays_filed(model(Bucket::Spam)).reason, "stub");
}
