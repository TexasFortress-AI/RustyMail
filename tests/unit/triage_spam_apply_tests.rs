// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Deciding moves out of the spam folder: Spam stays; anything else goes
//! where an INBOX run would have left it (Keep, or a bucket that is not
//! applied, means the INBOX); the usual checks still hold.

use chrono::Utc;
use rustymail::triage::apply::{decide_in, ApplyOptions, Skip};
use rustymail::triage::store::StoredProposal;
use rustymail::triage::Bucket;

fn proposal(bucket: &str, confidence: f64) -> StoredProposal {
    StoredProposal {
        id: 1,
        account_id: "a@x.test".into(),
        folder: "Junk".into(),
        uidvalidity: 5,
        uid: 10,
        message_id: Some("<m@x.test>".into()),
        from_address: "n@x.test".into(),
        subject: "s".into(),
        bucket: bucket.into(),
        confidence: Some(confidence),
        source: "model".into(),
        model_id: None,
        reason: "r".into(),
        not_before: None,
        list_name: None,
        correction: None,
        apply_error: None,
        created_at: "2026-11-24 00:00:00".into(),
    }
}

fn in_spam(p: &StoredProposal) -> Result<Bucket, Skip> {
    let options = ApplyOptions {
        min_confidence: 0.8,
        buckets: vec![Bucket::Spam, Bucket::Noise],
        limit: None,
    };
    decide_in(p, 5, Utc::now(), &options, Bucket::Spam)
}

#[test]
fn spam_stays_and_everything_else_is_rescued_where_it_belongs() {
    assert_eq!(in_spam(&proposal("Spam", 0.99)), Err(Skip::StaysInSpam));
    assert_eq!(in_spam(&proposal("Noise", 0.9)), Ok(Bucket::Noise));
    assert_eq!(in_spam(&proposal("Keep", 0.9)), Ok(Bucket::Keep));
    // Needs Reply is not applied here: it would have stayed in the INBOX.
    assert_eq!(in_spam(&proposal("Needs Reply", 0.9)), Ok(Bucket::Keep));
    // A bucket the row does not name stays where it is.
    assert_eq!(in_spam(&proposal("Bogus", 0.9)), Err(Skip::StaysInSpam));
}

#[test]
fn the_owner_decides_and_a_doubtful_model_does_not_rescue() {
    assert_eq!(in_spam(&proposal("Noise", 0.5)), Err(Skip::LowConfidence));
    let mut reviewed = proposal("Spam", 0.99);
    reviewed.correction = Some("Keep".into());
    assert_eq!(in_spam(&reviewed), Ok(Bucket::Keep));
    let mut confirmed = proposal("Noise", 0.9);
    confirmed.correction = Some("Spam".into());
    assert_eq!(in_spam(&confirmed), Err(Skip::StaysInSpam));
    let mut stale = proposal("Noise", 0.9);
    stale.uidvalidity = 4;
    assert_eq!(in_spam(&stale), Err(Skip::Stale));
}
