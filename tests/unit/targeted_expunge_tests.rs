// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Choosing an expunge that cannot remove messages outside the targets.

use rustymail::imap::targeted_expunge::{plan_targeted_expunge, ExpungePlan};
use std::collections::HashSet;

#[test]
fn uidplus_always_expunges_by_uid() {
    let others: HashSet<u32> = [5, 6].into_iter().collect();
    assert_eq!(
        plan_targeted_expunge(true, &others, &[1]),
        ExpungePlan::UidExpunge
    );
}

#[test]
fn without_uidplus_plain_expunge_only_when_targets_are_the_only_deleted() {
    let only_targets: HashSet<u32> = [1, 2].into_iter().collect();
    assert_eq!(
        plan_targeted_expunge(false, &only_targets, &[1, 2]),
        ExpungePlan::PlainExpunge
    );
    assert_eq!(
        plan_targeted_expunge(false, &HashSet::new(), &[1]),
        ExpungePlan::PlainExpunge
    );
}

#[test]
fn without_uidplus_other_deleted_messages_block_the_expunge() {
    let mixed: HashSet<u32> = [1, 7, 8].into_iter().collect();
    assert_eq!(
        plan_targeted_expunge(false, &mixed, &[1]),
        ExpungePlan::LeaveFlagged { other_deleted: 2 }
    );
}
