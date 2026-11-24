// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The folder each proposal moves to during one apply run, created when
//! allowed. List and repo mail goes to a child of the bucket's folder named
//! after the list: `INBOX.Lists.zeromq-dev`, or `Lists/zeromq-dev` on a
//! server that keeps folders at the top level.

use super::folders::{bucket_namespace, special_use, Destination, FolderMap};
use super::runner::Client;
use super::Bucket;
use crate::imap::session::AsyncImapOps;
use crate::imap::special_folders::{inbox_namespace, marked_folder, server_delimiter};
use crate::imap::types::Folder;
use log::info;
use std::collections::HashMap;

/// Resolves and caches destinations for one account's apply run.
pub struct FolderResolver<'a> {
    client: &'a Client,
    account_id: &'a str,
    folders: &'a FolderMap,
    tree: Vec<Folder>,
    existing: Vec<String>,
    resolved: HashMap<(Bucket, Option<String>), Result<String, String>>,
    /// Folders this run created, for the report.
    pub created: Vec<String>,
}

impl<'a> FolderResolver<'a> {
    /// Reads the account's folders once.
    pub async fn new(
        client: &'a Client,
        account_id: &'a str,
        folders: &'a FolderMap,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            client,
            account_id,
            folders,
            existing: client.list_folders().await?,
            tree: client.session().list_folders_hierarchical().await?,
            resolved: HashMap::new(),
            created: Vec::new(),
        })
    }

    /// The folder for `bucket` (and, for list and repo mail, `list_name`).
    pub async fn folder_for(
        &mut self,
        bucket: Bucket,
        list_name: Option<&str>,
    ) -> Result<String, String> {
        let key = (bucket, list_name.map(str::to_string));
        if let Some(known) = self.resolved.get(&key) {
            return known.clone();
        }
        let result = self.resolve(bucket, list_name).await;
        self.resolved.insert(key, result.clone());
        result
    }

    /// The bucket's folder only if it already exists (the folder the server
    /// marks for it, or the configured one); never creates a folder.
    pub fn existing_folder(&self, bucket: Bucket) -> Option<String> {
        let marked = special_use(bucket).and_then(|kind| marked_folder(&self.tree, kind));
        let namespace = bucket_namespace(&self.existing, &inbox_namespace(&self.tree));
        match self.folders.resolve(
            self.account_id,
            bucket,
            &self.existing,
            marked.as_deref(),
            &namespace,
        ) {
            Destination::Existing(name) => Some(name),
            Destination::Create(_) | Destination::Unavailable(_) => None,
        }
    }

    async fn resolve(&mut self, bucket: Bucket, list_name: Option<&str>) -> Result<String, String> {
        let parent = self.bucket_folder(bucket).await?;
        if !matches!(bucket, Bucket::Lists | Bucket::Repos) {
            return Ok(parent);
        }
        let name = list_name.ok_or("no list name recorded for this proposal")?;
        let child = format!("{}{}{}", parent, server_delimiter(&self.tree), name);
        self.existing_or_created(child).await
    }

    /// The bucket's own folder; a trash folder is never created.
    async fn bucket_folder(&mut self, bucket: Bucket) -> Result<String, String> {
        let marked = special_use(bucket).and_then(|kind| marked_folder(&self.tree, kind));
        let namespace = bucket_namespace(&self.existing, &inbox_namespace(&self.tree));
        match self.folders.resolve(
            self.account_id,
            bucket,
            &self.existing,
            marked.as_deref(),
            &namespace,
        ) {
            Destination::Existing(name) => Ok(name),
            Destination::Unavailable(reason) => Err(reason),
            Destination::Create(name) => self.create(name).await,
        }
    }

    async fn existing_or_created(&mut self, name: String) -> Result<String, String> {
        match self
            .existing
            .iter()
            .find(|folder| folder.eq_ignore_ascii_case(&name))
        {
            Some(folder) => Ok(folder.clone()),
            None => self.create(name).await,
        }
    }

    async fn create(&mut self, name: String) -> Result<String, String> {
        self.client
            .create_folder(&name)
            .await
            .map_err(|e| format!("cannot create folder {}: {}", name, e))?;
        info!("{}: created folder {}", self.account_id, name);
        self.existing.push(name.clone());
        self.created.push(name.clone());
        Ok(name)
    }
}
