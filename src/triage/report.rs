// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Plain-text review of proposals and apply results, with dashboard deep links.
//! Printed to the terminal for Chris; never logged.

use super::apply::{ApplySummary, Skip};
use super::store::{BucketCount, StoredProposal};
use super::Bucket;
use std::fmt::Write;

const SUBJECT_CHARS: usize = 80;

/// Dashboard link that opens one message.
pub fn deep_link(dashboard_url: &str, proposal: &StoredProposal) -> String {
    format!(
        "{}/?account={}&folder={}&uid={}",
        dashboard_url,
        urlencoding::encode(&proposal.account_id),
        urlencoding::encode(&proposal.folder),
        proposal.uid
    )
}

/// Counts per bucket, then the pending rows grouped by account and bucket.
pub fn format_report(
    counts: &[BucketCount],
    proposals: &[StoredProposal],
    dashboard_url: &str,
) -> String {
    let mut out = format_counts(counts);
    if proposals.is_empty() {
        out.push_str("No pending triage proposals.\n");
        return out;
    }
    let shown: u64 = proposals.len() as u64;
    let pending: u64 = counts.iter().map(|c| c.pending).sum();
    if shown < pending {
        let _ = writeln!(out, "(showing the newest {} of {} pending)", shown, pending);
    }
    out.push_str(&format_rows(proposals, dashboard_url));
    out
}

/// One line per account: pending and applied counts per bucket.
pub fn format_counts(counts: &[BucketCount]) -> String {
    let mut accounts: Vec<&str> = counts.iter().map(|c| c.account_id.as_str()).collect();
    accounts.dedup();
    let mut out = String::new();
    for account in accounts {
        let mut parts = Vec::new();
        for bucket in Bucket::ALL {
            if let Some(c) = counts
                .iter()
                .find(|c| c.account_id == account && c.bucket == bucket.as_str())
            {
                parts.push(format!(
                    "{} {} pending/{} moved",
                    bucket, c.pending, c.applied
                ));
            }
        }
        let _ = writeln!(out, "{}: {}", account, parts.join(", "));
    }
    out
}

fn format_rows(proposals: &[StoredProposal], dashboard_url: &str) -> String {
    let mut accounts: Vec<&str> = proposals.iter().map(|p| p.account_id.as_str()).collect();
    accounts.sort_unstable();
    accounts.dedup();

    let mut out = String::new();
    for account in accounts {
        // The INBOX first, then each spam folder the review read.
        let mut folders: Vec<&str> = proposals
            .iter()
            .filter(|p| p.account_id == account)
            .map(|p| p.folder.as_str())
            .collect();
        folders.sort_unstable_by_key(|f| (!f.eq_ignore_ascii_case("INBOX"), *f));
        folders.dedup();
        for folder in folders {
            let mine: Vec<&StoredProposal> = proposals
                .iter()
                .filter(|p| p.account_id == account && p.folder == folder)
                .collect();
            if folder.eq_ignore_ascii_case("INBOX") {
                let _ = writeln!(out, "== {} ({} proposals)", account, mine.len());
            } else {
                let _ = writeln!(
                    out,
                    "== {} {}: spam-folder review ({} proposals; Spam stays, anything else is a rescue)",
                    account,
                    folder,
                    mine.len()
                );
            }
            out.push_str(&format_buckets(&mine, dashboard_url));
        }
    }
    out
}

fn format_buckets(mine: &[&StoredProposal], dashboard_url: &str) -> String {
    let mut out = String::new();
    for bucket in Bucket::ALL {
        let rows: Vec<&&StoredProposal> = mine
            .iter()
            .filter(|p| p.bucket == bucket.as_str())
            .collect();
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "  {} ({})", bucket, rows.len());
        for p in rows {
            let _ = writeln!(out, "    {}", describe(p));
            let _ = writeln!(out, "      {}", deep_link(dashboard_url, p));
        }
    }
    out
}

fn describe(p: &StoredProposal) -> String {
    let subject: String = p.subject.chars().take(SUBJECT_CHARS).collect();
    let evidence = match (&p.model_id, p.confidence) {
        (Some(model), Some(confidence)) => format!("{} {:.2}: {}", model, confidence, p.reason),
        _ => format!("{}: {}", p.source, p.reason),
    };
    let mut line = format!(
        "uid {} | {} | {} | [{}]",
        p.uid, p.from_address, subject, evidence
    );
    if let Some(list) = &p.list_name {
        let _ = write!(line, " | {} {}", p.bucket, list);
    }
    if let Some(time) = p.not_before {
        let _ = write!(line, " | due {}", time.format("%Y-%m-%d %H:%M UTC"));
    }
    if let Some(error) = &p.apply_error {
        let _ = write!(line, " | last apply failed: {}", error);
    }
    line
}

/// What an apply run did for one account.
pub fn format_apply_summary(summary: &ApplySummary) -> String {
    let mut out = format!(
        "{} {}: moved {} (verified), {} left {} but could not be found by Message-ID, {} failed\n",
        summary.account,
        summary.folder,
        summary.moved,
        summary.moved_unconfirmed,
        summary.folder,
        summary.failures.len()
    );
    for folder in &summary.created_folders {
        let _ = writeln!(out, "  created folder {}", folder);
    }
    if summary.untagged > 0 {
        let _ = writeln!(
            out,
            "  spam tag removed from the subject: {}",
            summary.untagged
        );
    }
    let mut skipped: Vec<(&Skip, &usize)> = summary.skipped.iter().collect();
    skipped.sort_by_key(|(skip, _)| format!("{:?}", skip));
    for (skip, count) in skipped {
        let _ = writeln!(out, "  left in place ({}): {}", skip_label(*skip), count);
    }
    for failure in &summary.failures {
        let _ = writeln!(
            out,
            "  FAILED uid {} -> {}: {}",
            failure.uid, failure.bucket, failure.error
        );
    }
    if let Some(error) = &summary.connection_lost {
        let _ = writeln!(
            out,
            "  STOPPED: connection lost ({}); run --apply again to continue",
            error
        );
    }
    out
}

fn skip_label(skip: Skip) -> &'static str {
    match skip {
        Skip::StaysInInbox => "Keep",
        Skip::StaysInSpam => "Spam (stays in the spam folder)",
        Skip::BucketNotApplied => "bucket not in TRIAGE_APPLY_BUCKETS",
        Skip::LowConfidence => "below minimum confidence",
        Skip::NotYetDue => "code or reset not expired yet",
        Skip::Stale => "UIDVALIDITY changed; re-run triage",
    }
}
