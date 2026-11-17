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
//!   a `List-Id` but no address to post to, and stay with the classifier.
//!
//! The folder name is the first label of the `List-Id` (`RustyMail`,
//! `zeromq-dev`). The sender writes that header, so the name is cut to safe
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
        let (bucket, reason) = if self.is_repo(&list_id) {
            (Bucket::Repos, "code repository notification (List-Id)")
        } else if has_post_address(message) {
            (Bucket::Lists, "mailing list post (List-Id and List-Post)")
        } else {
            return None;
        };
        let mut proposal = Proposal::from_rule(bucket, reason);
        proposal.list_name = Some(folder_name(&list_id)?);
        Some(proposal)
    }

    fn is_repo(&self, list_id: &str) -> bool {
        let list_id = list_id.to_ascii_lowercase();
        self.repo_domains
            .iter()
            .any(|domain| list_id.ends_with(&format!(".{}", domain)))
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

/// The first label of the list id, reduced to letters, digits, `-` and `_`.
fn folder_name(list_id: &str) -> Option<String> {
    let name: String = list_id
        .split('.')
        .next()?
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(MAX_LIST_NAME_CHARS)
        .collect();
    (!name.is_empty()).then_some(name)
}
