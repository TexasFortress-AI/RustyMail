// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which messages a triage run looks at, and where the watermark moves.

use super::store::Watermark;
use chrono::{Duration, NaiveDate};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanPlan {
    /// No usable watermark and no backfill: start at the current end of the
    /// folder so only mail arriving from now on is triaged.
    Initialize,
    /// UID SEARCH with these criteria, keeping only UIDs above `above`.
    Search { criteria: String, above: u32 },
}

/// A stored watermark only counts while UIDVALIDITY is unchanged.
pub fn plan_scan(
    stored: Option<Watermark>,
    uidvalidity: u32,
    since_days: Option<u32>,
    today: NaiveDate,
) -> ScanPlan {
    let current = stored.filter(|w| w.uidvalidity == uidvalidity);
    match (since_days, current) {
        (Some(days), _) => ScanPlan::Search {
            criteria: format!(
                "SINCE {}",
                (today - Duration::days(i64::from(days))).format("%d-%b-%Y")
            ),
            above: 0,
        },
        (None, Some(w)) => ScanPlan::Search {
            criteria: format!("UID {}:*", w.last_uid.saturating_add(1)),
            above: w.last_uid,
        },
        (None, None) => ScanPlan::Initialize,
    }
}

/// UIDs to triage, oldest first. `UID n:*` also returns the newest message
/// when it is below n, hence the `above` filter.
pub fn select_uids(
    found: &[u32],
    above: u32,
    already_proposed: &HashSet<u32>,
    limit: Option<usize>,
) -> Vec<u32> {
    let mut uids: Vec<u32> = found
        .iter()
        .copied()
        .filter(|uid| *uid > above && !already_proposed.contains(uid))
        .collect();
    uids.sort_unstable();
    uids.dedup();
    if let Some(limit) = limit {
        uids.truncate(limit);
    }
    uids
}

/// The watermark never moves backwards.
pub fn advance(current_last_uid: Option<u32>, last_done: Option<u32>) -> Option<u32> {
    match (current_last_uid, last_done) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}
