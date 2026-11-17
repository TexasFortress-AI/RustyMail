// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The server's own Sent, Drafts, Trash and Junk folders, and RustyMail's Outbox.
//!
//! Servers name them differently ("Sent Items", "Sent", "INBOX.Sent"), and an
//! account moved between servers keeps the old names next to the new ones.
//! The server marks the folder it uses with an RFC 6154 special-use attribute
//! (`\Sent`, `\Drafts`); that wins. Without one, the usual names are tried,
//! then a new folder in the account's namespace.

use super::client::ImapClient;
use super::error::ImapError;
use super::session::AsyncImapOps;
use super::types::Folder;
use log::info;
use std::fmt::Debug;

/// The mailbox whose children, if any, form the account's folder namespace.
const INBOX: &str = "INBOX";

/// Hierarchy delimiter assumed when the server reports none.
const DEFAULT_DELIMITER: &str = ".";

/// RustyMail's Outbox is not an RFC 6154 folder, so it is found by name:
/// the names used so far, then a new folder named `OUTBOX_LEAF`.
const OUTBOX_NAMES: [&str; 2] = ["INBOX.Outbox", "Outbox"];
const OUTBOX_LEAF: &str = "Outbox";

/// A special-use folder that RustyMail writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUse {
    Sent,
    Drafts,
    Trash,
    Junk,
}

impl SpecialUse {
    /// The attribute as `list_folders_hierarchical` records it (the Debug
    /// form of `imap_proto::NameAttribute`).
    fn attribute(self) -> &'static str {
        match self {
            SpecialUse::Sent => "Sent",
            SpecialUse::Drafts => "Drafts",
            SpecialUse::Trash => "Trash",
            SpecialUse::Junk => "Junk",
        }
    }

    /// Names tried, in order, when no folder carries the attribute.
    fn usual_names(self) -> &'static [&'static str] {
        match self {
            SpecialUse::Sent => &["Sent Items", "Sent", "INBOX.Sent", "Sent Messages"],
            SpecialUse::Drafts => &["Drafts", "INBOX.Drafts"],
            SpecialUse::Trash => &["Trash", "Deleted Items", "Deleted Messages", "INBOX.Trash"],
            SpecialUse::Junk => &["Junk", "Junk Mail", "Spam", "INBOX.Junk", "INBOX.spam"],
        }
    }

    /// Name for a new folder when the server has none.
    fn default_leaf(self) -> &'static str {
        self.attribute()
    }
}

/// Where to put mail of this kind: the existing folder, or one to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialFolder {
    Existing(String),
    Create(String),
}

impl SpecialFolder {
    pub fn name(&self) -> &str {
        match self {
            SpecialFolder::Existing(name) | SpecialFolder::Create(name) => name,
        }
    }
}

/// A folder named outright, the server's folder for a special use, or
/// RustyMail's Outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderTarget<'a> {
    Named(&'a str),
    Special(SpecialUse),
    Outbox,
}

/// The selectable folder the server marks with `kind`'s attribute, if any.
pub fn marked_folder(folders: &[Folder], kind: SpecialUse) -> Option<String> {
    let mut selectable = Vec::new();
    flatten(folders, &mut selectable);
    selectable
        .iter()
        .find(|f| f.attributes.iter().any(|a| a == kind.attribute()))
        .map(|f| f.full_path.clone())
}

/// Pick the folder for `kind` from the server's folder tree.
pub fn find_special_folder(folders: &[Folder], kind: SpecialUse) -> SpecialFolder {
    match marked_folder(folders, kind) {
        Some(name) => SpecialFolder::Existing(name),
        None => find_by_name(folders, kind.usual_names(), kind.default_leaf()),
    }
}

/// RustyMail's Outbox: an existing one, else a new one in the namespace.
pub fn find_outbox(folders: &[Folder]) -> SpecialFolder {
    find_by_name(folders, &OUTBOX_NAMES, OUTBOX_LEAF)
}

/// The first selectable folder named one of `names` (any case), else `leaf`
/// in the account's namespace, to be created.
fn find_by_name(folders: &[Folder], names: &[&str], leaf: &str) -> SpecialFolder {
    let mut selectable = Vec::new();
    flatten(folders, &mut selectable);
    let found = |name: &&str| {
        selectable
            .iter()
            .find(|f| f.full_path.eq_ignore_ascii_case(name))
            .map(|f| f.full_path.clone())
    };
    match names.iter().find_map(found) {
        Some(name) => SpecialFolder::Existing(name),
        None => SpecialFolder::Create(format!("{}{}", inbox_namespace(folders), leaf)),
    }
}

/// Selectable folders of the tree, depth first.
fn flatten<'a>(folders: &'a [Folder], into: &mut Vec<&'a Folder>) {
    for folder in folders {
        if folder.selectable {
            into.push(folder);
        }
        flatten(&folder.children, into);
    }
}

/// `INBOX` and the server's delimiter (`INBOX.` or `INBOX/`) when it keeps
/// user folders under the INBOX, else empty. Decided by real children of the
/// INBOX in the folder tree, not by names: on Gmail, whose delimiter is `/`,
/// a label named `INBOX.Outbox` is a top-level folder.
pub fn inbox_namespace(folders: &[Folder]) -> String {
    for folder in folders {
        if let (Some(delimiter), Some(parent)) = (&folder.delimiter, &folder.parent) {
            if parent.eq_ignore_ascii_case(INBOX) {
                return format!("{}{}", INBOX, delimiter);
            }
        }
        let nested = inbox_namespace(&folder.children);
        if !nested.is_empty() {
            return nested;
        }
    }
    String::new()
}

/// The server's hierarchy delimiter: the INBOX's, else the first one listed.
pub fn server_delimiter(folders: &[Folder]) -> String {
    let mut all = Vec::new();
    flatten_all(folders, &mut all);
    all.iter()
        .find(|f| f.full_path.eq_ignore_ascii_case(INBOX))
        .or_else(|| all.iter().find(|f| f.delimiter.is_some()))
        .and_then(|f| f.delimiter.clone())
        .unwrap_or_else(|| DEFAULT_DELIMITER.to_string())
}

/// Every folder of the tree, selectable or not, depth first.
fn flatten_all<'a>(folders: &'a [Folder], into: &mut Vec<&'a Folder>) {
    for folder in folders {
        into.push(folder);
        flatten_all(&folder.children, into);
    }
}

impl<T: AsyncImapOps + Send + Sync + Debug + 'static> ImapClient<T> {
    /// The folder `target` names: as given, or the server's special-use folder.
    pub async fn target_folder(&self, target: FolderTarget<'_>) -> Result<String, ImapError> {
        match target {
            FolderTarget::Named(name) => Ok(name.to_string()),
            FolderTarget::Special(kind) => self.special_folder(kind).await,
            FolderTarget::Outbox => self.outbox_folder().await,
        }
    }

    /// The server's folder for `kind` (Sent, Drafts), created if it has none.
    pub async fn special_folder(&self, kind: SpecialUse) -> Result<String, ImapError> {
        let folders = self.session().list_folders_hierarchical().await?;
        self.existing_or_created(find_special_folder(&folders, kind))
            .await
    }

    /// RustyMail's Outbox on this server, created if it has none.
    pub async fn outbox_folder(&self) -> Result<String, ImapError> {
        let folders = self.session().list_folders_hierarchical().await?;
        self.existing_or_created(find_outbox(&folders)).await
    }

    async fn existing_or_created(&self, folder: SpecialFolder) -> Result<String, ImapError> {
        match folder {
            SpecialFolder::Existing(name) => Ok(name),
            SpecialFolder::Create(name) => {
                info!("Creating folder {}", name);
                self.create_folder(&name).await?;
                Ok(name)
            }
        }
    }
}
