// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Mailing lists and code-repository notifications, recognised by their
//! standard headers on any mail server, each get a folder of their own.
//!
//! - A repository: `List-Id` (RFC 2919) under a code host's domain, such as
//!   `<RustyMail.TexasFortress-AI.github.com>`. Checked first, because a code
//!   host marks only some of its notifications with a `List-Post`.
//! - A mailing list: a `List-Id` and a `List-Post` (RFC 2369) with a `mailto:`
//!   address, as Mailman, Google Groups and groups.io send. Newsletters carry
//!   a `List-Id` but no address to post to, and stay with the classifier, as
//!   does an organisation's own group posted to by its members (a company's
//!   all-staff list), which is mail for the INBOX.
//!
//! The folder name is the repository (`RustyMail`, `llama.cpp` as
//! `llama-cpp`) or the list id's first label (`zeromq-dev`). The sender writes that header, so the name is cut to safe
//! characters and a length cap before it can become a folder.

use super::message::TriageMessage;
use super::{Bucket, Proposal};

/// Longest list or repo name used as a folder name.
const MAX_LIST_NAME_CHARS: usize = 64;

/// Decides which messages are list posts or repo notifications.
#[derive(Debug, Clone, Default)]
pub struct ListRules {
    /// Domains whose `List-Id`s name code repositories, e.g. `github.com`.
    repo_domains: Vec<String>,
}

impl ListRules {
    pub fn new(repo_domains: Vec<String>) -> Self {
        let repo_domains = repo_domains
            .into_iter()
            .map(|domain| domain.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|domain| !domain.is_empty())
            .collect();
        Self { repo_domains }
    }

    /// A `Repos` or `Lists` proposal naming the folder, or None.
    pub fn proposal(&self, message: &TriageMessage) -> Option<Proposal> {
        let list_id = list_id(message)?;
        let (bucket, name, reason) = if let Some(repo) = self.repo_name(&list_id) {
            (
                Bucket::Repos,
                repo,
                "code repository notification (List-Id)",
            )
        } else if has_post_address(message) && !from_own_organisation(message, &list_id) {
            let first_label = list_id.split('.').next()?;
            (
                Bucket::Lists,
                first_label,
                "mailing list post (List-Id and List-Post)",
            )
        } else {
            return None;
        };
        let mut proposal = Proposal::from_rule(bucket, reason);
        proposal.list_name = Some(folder_name(name)?);
        Some(proposal)
    }

    /// The repository in a code host's `List-Id`, `<repo>.<owner>.<domain>`.
    /// The repo name may itself hold dots (`llama.cpp.owner.github.com`).
    fn repo_name<'l>(&self, list_id: &'l str) -> Option<&'l str> {
        let lower = list_id.to_ascii_lowercase();
        let domain = self
            .repo_domains
            .iter()
            .find(|domain| lower.ends_with(&format!(".{}", domain)))?;
        let without_domain = &list_id[..list_id.len() - domain.len() - 1];
        Some(
            without_domain
                .rsplit_once('.')
                .map_or(without_domain, |(repo, _owner)| repo),
        )
    }
}

/// The identifier inside the angle brackets of `List-Id`, which may follow a
/// description: `ZeroMQ development list <zeromq-dev.lists.zeromq.org>`.
pub fn list_id(message: &TriageMessage) -> Option<String> {
    let value = message.header("list-id")?;
    let inner = match (value.rfind('<'), value.rfind('>')) {
        (Some(start), Some(end)) if start < end => &value[start + 1..end],
        _ => value,
    };
    let inner = inner.trim();
    (!inner.is_empty()).then(|| inner.to_string())
}

/// `List-Post` offers an address to post to (not `NO`, not only a web form).
fn has_post_address(message: &TriageMessage) -> bool {
    message
        .header("list-post")
        .is_some_and(|value| value.to_ascii_lowercase().contains("mailto:"))
}

/// The sender writes from the list's own domain: an organisation addressing
/// its members through its group (`billy@company.example` to
/// `all-users.company.example`). That is mail to the recipient as a member,
/// for the INBOX, not a discussion they follow. Exact match only: a project
/// list on `lists.zeromq.org` posted to from `zeromq.org` is still a list.
fn from_own_organisation(message: &TriageMessage, list_id: &str) -> bool {
    let list_domain = list_id.split_once('.').map(|(_, domain)| domain);
    match (list_domain, message.from_domain()) {
        (Some(list), Some(sender)) => list.eq_ignore_ascii_case(sender),
        _ => false,
    }
}

/// A list or repo name reduced to letters, digits, `-` and `_`. Dots become
/// `-`, since a dot is the folder delimiter on many servers.
fn folder_name(name: &str) -> Option<String> {
    let name: String = name
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(MAX_LIST_NAME_CHARS)
        .collect();
    (!name.is_empty()).then_some(name)
}
