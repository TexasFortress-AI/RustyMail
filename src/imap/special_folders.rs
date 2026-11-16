// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The server's own Sent and Drafts folders.
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

/// Namespace used when the server keeps user folders under the INBOX.
const INBOX_NAMESPACE: &str = "INBOX.";

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

/// A folder named outright, or the server's folder for a special use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderTarget<'a> {
    Named(&'a str),
    Special(SpecialUse),
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
    if let Some(name) = marked_folder(folders, kind) {
        return SpecialFolder::Existing(name);
    }
    let mut selectable = Vec::new();
    flatten(folders, &mut selectable);
    let found = |name: &&str| {
        selectable
            .iter()
            .find(|f| f.full_path.eq_ignore_ascii_case(name))
            .map(|f| f.full_path.clone())
    };
    if let Some(name) = kind.usual_names().iter().find_map(found) {
        return SpecialFolder::Existing(name);
    }
    SpecialFolder::Create(format!("{}{}", namespace(&selectable), kind.default_leaf()))
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

/// `INBOX.` when the server keeps user folders under the INBOX.
fn namespace(folders: &[&Folder]) -> &'static str {
    if folders
        .iter()
        .any(|f| f.full_path.starts_with(INBOX_NAMESPACE))
    {
        INBOX_NAMESPACE
    } else {
        ""
    }
}

impl<T: AsyncImapOps + Send + Sync + Debug + 'static> ImapClient<T> {
    /// The folder `target` names: as given, or the server's special-use folder.
    pub async fn target_folder(&self, target: FolderTarget<'_>) -> Result<String, ImapError> {
        match target {
            FolderTarget::Named(name) => Ok(name.to_string()),
            FolderTarget::Special(kind) => self.special_folder(kind).await,
        }
    }

    /// The server's folder for `kind` (Sent, Drafts), created if it has none.
    pub async fn special_folder(&self, kind: SpecialUse) -> Result<String, ImapError> {
        let folders = self.session().list_folders_hierarchical().await?;
        match find_special_folder(&folders, kind) {
            SpecialFolder::Existing(name) => Ok(name),
            SpecialFolder::Create(name) => {
                info!("No {:?} folder on the server; creating {}", kind, name);
                self.create_folder(&name).await?;
                Ok(name)
            }
        }
    }
}
