// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Which provider hosts a domain's mail, from its MX records.
//!
//! A custom domain (you@example.com) hosted by Google or Microsoft has no
//! autoconfig of its own, but its MX host (aspmx.l.google.com) names the
//! provider, whose settings Mozilla's ISPDB has under its own domain
//! (google.com). This is how Thunderbird finds them.

use hickory_resolver::lookup::Lookup;
use hickory_resolver::proto::rr::RData;

/// The most preferred mail exchanger in an MX lookup, without the trailing dot.
pub fn preferred_mail_host(lookup: &Lookup) -> Option<String> {
    lookup
        .answers()
        .iter()
        .filter_map(|record| match &record.data {
            RData::MX(mx) => Some(mx),
            _ => None,
        })
        .min_by_key(|mx| mx.preference)
        .map(|mx| {
            mx.exchange
                .to_string()
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
}

/// Domains to look up in the ISPDB for a mail host, most specific first:
/// `aspmx.l.google.com` gives `l.google.com`, then `google.com`. Never shorter
/// than two labels, and never the user's own domain, which was already tried.
pub fn provider_domains(mail_host: &str, own_domain: &str) -> Vec<String> {
    let labels: Vec<&str> = mail_host.split('.').filter(|l| !l.is_empty()).collect();
    (1..labels.len().saturating_sub(1))
        .map(|start| labels[start..].join("."))
        .filter(|candidate| !candidate.eq_ignore_ascii_case(own_domain))
        .collect()
}
