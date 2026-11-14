// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Plain-text review of pending proposals, with dashboard deep links.

use super::store::StoredProposal;
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

/// Group by account, then by bucket in a fixed order.
pub fn format_report(proposals: &[StoredProposal], dashboard_url: &str) -> String {
    if proposals.is_empty() {
        return "No pending triage proposals.\n".to_string();
    }
    let mut accounts: Vec<&str> = proposals.iter().map(|p| p.account_id.as_str()).collect();
    accounts.sort_unstable();
    accounts.dedup();

    let mut out = String::new();
    for account in accounts {
        let mine: Vec<&StoredProposal> = proposals
            .iter()
            .filter(|p| p.account_id == account)
            .collect();
        let _ = writeln!(out, "== {} ({} proposals)", account, mine.len());
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
    }
    out
}

fn describe(p: &StoredProposal) -> String {
    let subject: String = p.subject.chars().take(SUBJECT_CHARS).collect();
    let evidence = match (&p.model_id, p.confidence) {
        (Some(model), Some(confidence)) => format!("{} {:.2}: {}", model, confidence, p.reason),
        _ => format!("{}: {}", p.source, p.reason),
    };
    format!(
        "uid {} | {} | {} | [{}]",
        p.uid, p.from_address, subject, evidence
    )
}
