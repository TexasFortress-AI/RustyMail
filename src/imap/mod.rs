#![allow(unused_imports)]

// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.


// Public Interface for the IMAP module

pub mod atomic;
pub mod client;
pub mod error;
pub mod oauth2;
pub mod session;
pub mod types;
pub mod xoauth2;

// --- Re-exports ---
// Keep these minimal and focused on the public API

pub use client::ImapClient;
pub use error::ImapError;
pub use oauth2::{MicrosoftOAuth2Client, MicrosoftOAuth2Config, OAuth2Error, StoredToken, TokenResponse};
pub use session::{AsyncImapOps, AsyncImapSessionWrapper};
pub use types::{
    Address, Email, Envelope, FlagOperation, Flags, Folder, MailboxInfo, SearchCriteria,
    // Re-export necessary payload types if they are part of the public API
    AppendEmailPayload, ModifyFlagsPayload,
};
pub use xoauth2::XOAuth2Authenticator;

// --- Type Aliases (Consider if these are truly needed publicly) ---

// Remove unresolved AccountConfig import
// use crate::config::AccountConfig; // Needed for factory
use futures::future::BoxFuture; // Needed for factory
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::fmt;

// Import ImapClientFactory from session module
use crate::imap::session::ImapClientFactory;

// Result type for the factory
pub type ImapSessionFactoryResult = Result<ImapClient<AsyncImapSessionWrapper>, ImapError>;

// Add ImapSessionFactory as a type alias for ImapClientFactory
pub type ImapSessionFactory = Box<dyn Fn() -> BoxFuture<'static, ImapSessionFactoryResult> + Send + Sync>;

// Cloneable wrapper for ImapSessionFactory
#[derive(Clone)]
pub struct CloneableImapSessionFactory {
    factory: Arc<ImapSessionFactory>,
}

impl CloneableImapSessionFactory {
    pub fn new(factory: ImapSessionFactory) -> Self {
        Self {
            factory: Arc::new(factory),
        }
    }

    /// Create a session using the default factory (credentials from .env)
    pub fn create_session(&self) -> BoxFuture<ImapSessionFactoryResult> {
        (self.factory)()
    }

    /// Create a session for a specific account (using account's credentials)
    pub async fn create_session_for_account(
        &self,
        account: &crate::dashboard::services::account::Account,
    ) -> ImapSessionFactoryResult {
        use crate::imap::client::ImapClient;
        use log::debug;

        debug!("Creating IMAP session for account: {} ({})", account.email_address, account.imap_host);

        // Route to XOAUTH2 if account is configured for OAuth and has an access token
        if account.is_oauth() {
            if let Some(ref token) = account.oauth_access_token {
                debug!("Using XOAUTH2 authentication for {}", account.email_address);
                let client = ImapClient::<AsyncImapSessionWrapper>::connect_with_xoauth2(
                    &account.imap_host,
                    account.imap_port as u16,
                    &account.imap_user,
                    token,
                ).await?;
                return Ok(client);
            }
            return Err(ImapError::Auth("OAuth account has no access token — complete OAuth flow first".to_string()));
        }

        // Password-based authentication
        let client = ImapClient::<AsyncImapSessionWrapper>::connect(
            &account.imap_host,
            account.imap_port as u16,
            &account.imap_user,
            &account.imap_pass,
        ).await?;

        Ok(client)
    }
}

impl fmt::Debug for CloneableImapSessionFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloneableImapSessionFactory")
            .field("factory", &"<function>")
            .finish()
    }
}

// SAFETY: ImapClientFactory is Box<dyn Fn() + Send + Sync>, so Arc<ImapClientFactory> is Send
unsafe impl Send for CloneableImapSessionFactory {}

// SAFETY: ImapClientFactory is Box<dyn Fn() + Send + Sync>, so Arc<ImapClientFactory> is Sync
unsafe impl Sync for CloneableImapSessionFactory {}

// Previous commented-out definition for reference
// pub type ImapSessionFactory = Arc<dyn Fn(&AccountConfig) -> BoxFuture<ImapSessionFactoryResult> + Send + Sync>;

// --- Potentially Remove or Move Internal Re-exports ---
// These seem like internal details or duplicates from the top-level re-exports

// pub use client::{ImapClientBuilder}; // Builder might be internal or exposed differently
// pub use session::{TlsImapSession}; // Likely internal

// Remove duplicate imports if already covered by `pub use` or not needed
// use std::sync::Arc;
// use session::{TlsCompatibleStream}; // Likely internal

// Remove the test module re-export if it was temporary
// #[cfg(test)] // Only expose for tests if absolutely necessary
// pub mod client_test;