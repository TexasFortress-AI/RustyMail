// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use sqlx::{SqlitePool, Row};
use log::{info, debug, error, warn};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use super::account_store::{AccountStore, StoredAccount, AccountStoreError};
use super::connection_status_store::{ConnectionStatusStore, ConnectionStatusStoreError};
use super::connection_status::AccountConnectionStatus;
use chrono::Utc;

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Account store error: {0}")]
    AccountStoreError(#[from] AccountStoreError),
    #[error("Connection status store error: {0}")]
    ConnectionStatusStoreError(#[from] ConnectionStatusStoreError),
    #[error("Account not found: {0}")]
    NotFound(String),
    #[error("Provider not supported: {0}")]
    ProviderNotSupported(String),
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),
    #[error("Account operation failed: {0}")]
    OperationFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    // email_address is the primary identifier, serialized as both "id" and "email_address"
    pub email_address: String,
    // Alias for email_address, serialized as "id" for frontend compatibility
    #[serde(skip_deserializing, rename = "id")]
    pub id: String,
    // display_name is serialized as both "account_name" and "display_name"
    #[serde(rename = "account_name", alias = "display_name")]
    pub display_name: String,
    pub provider_type: Option<String>,
    pub imap_host: String,
    pub imap_port: i64,
    pub imap_user: String,
    #[serde(skip_serializing)] // Never serialize passwords
    pub imap_pass: String,
    pub imap_use_tls: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_user: Option<String>,
    #[serde(skip_serializing)]
    pub smtp_pass: Option<String>,
    pub smtp_use_tls: Option<bool>,
    pub smtp_use_starttls: Option<bool>,
    #[serde(default = "default_is_active")]
    pub is_active: bool,
    #[serde(default)]
    pub is_default: bool,
    // Connection status for IMAP and SMTP (optional, populated from separate store)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_status: Option<super::connection_status::AccountConnectionStatus>,
}

// Default value function for is_active (defaults to true for new accounts)
fn default_is_active() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTemplate {
    pub provider_type: String,
    pub display_name: String,
    pub domain_patterns: Vec<String>,
    pub imap_host: String,
    pub imap_port: i64,
    pub imap_use_tls: bool,
    pub smtp_host: String,
    pub smtp_port: i64,
    pub smtp_use_tls: bool,
    pub smtp_use_starttls: bool,
    pub supports_oauth: bool,
    pub oauth_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoConfigResult {
    pub provider_found: bool,
    pub provider_type: Option<String>,
    pub display_name: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i64>,
    pub imap_use_tls: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_use_tls: Option<bool>,
    pub smtp_use_starttls: Option<bool>,
    pub supports_oauth: bool,
    pub oauth_provider: Option<String>,
}

pub struct AccountService {
    db_pool: Option<SqlitePool>,
    account_store: AccountStore,
    connection_status_store: ConnectionStatusStore,
}

impl AccountService {
    pub fn new(config_path: &str) -> Self {
        // Derive connection status path from accounts config path
        let connection_status_path = if config_path.ends_with(".json") {
            config_path.replace(".json", "_connection_status.json")
        } else {
            format!("{}_connection_status.json", config_path)
        };

        Self {
            db_pool: None,
            account_store: AccountStore::new(config_path),
            connection_status_store: ConnectionStatusStore::new(&connection_status_path),
        }
    }

    /// Initialize the account service with database pool
    pub async fn initialize(&mut self, db_pool: SqlitePool) -> Result<(), AccountError> {
        self.db_pool = Some(db_pool);
        self.account_store.initialize().await?;
        self.connection_status_store.initialize().await?;

        // Attempt to migrate accounts from database to file storage
        if let Err(e) = self.migrate_accounts_from_db().await {
            warn!("Account migration from database failed: {}. Continuing with file-based storage.", e);
        }

        // Sync accounts FROM file storage TO database
        if let Err(e) = self.sync_accounts_to_db().await {
            warn!("Failed to sync accounts to database: {}", e);
        }

        info!("Account service initialized with file-based storage");
        Ok(())
    }

    /// Create account from environment variables if no accounts exist
    pub async fn ensure_default_account_from_env(&mut self, settings: &crate::config::Settings) -> Result<(), AccountError> {
        use chrono::Utc;
        use crate::dashboard::services::account_store::{StoredAccount, ImapConfig};

        // Check if we already have accounts
        let existing_accounts = self.account_store.list_accounts().await?;
        if !existing_accounts.is_empty() {
            debug!("Accounts already exist, skipping environment auto-configuration");
            return Ok(());
        }

        // Check if IMAP credentials are provided in environment
        if settings.imap_user.is_empty() || settings.imap_pass.is_empty() {
            debug!("No IMAP credentials in environment, skipping auto-configuration");
            return Ok(());
        }

        info!("No accounts found, creating default account from environment variables");

        // Extract email address from IMAP user (often it's the full email)
        let email_address = if settings.imap_user.contains('@') {
            settings.imap_user.clone()
        } else {
            format!("{}@{}", settings.imap_user, settings.imap_host)
        };

        // Create account from environment variables (email_address is the primary identifier)
        let account = StoredAccount {
            display_name: format!("Default ({})", email_address),
            email_address: email_address.clone(),
            provider_type: Some("custom".to_string()),
            imap: ImapConfig {
                host: settings.imap_host.clone(),
                port: settings.imap_port,
                username: settings.imap_user.clone(),
                password: settings.imap_pass.clone(),
                use_tls: true,
            },
            smtp: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        match self.account_store.add_account(account.clone()).await {
            Ok(()) => {
                // Set as default account
                if let Err(e) = self.account_store.set_default_account(&account.email_address).await {
                    warn!("Failed to set default account: {}", e);
                }
                info!("Successfully created default account from environment: {}", account.email_address);

                // Sync to database cache so the account is immediately available for email operations
                if let Err(e) = self.sync_accounts_to_db().await {
                    warn!("Failed to sync new account to database cache: {}", e);
                    // Don't fail the account creation, but warn about it
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to create default account from environment: {}", e);
                Err(AccountError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Migrate accounts from database to file storage (one-time operation)
    async fn migrate_accounts_from_db(&self) -> Result<(), AccountError> {
        let db = self.db()?;

        // Check if accounts table exists in database
        let table_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'"
        )
        .fetch_one(db)
        .await? > 0;

        if !table_exists {
            debug!("No accounts table in database, skipping migration");
            return Ok(());
        }

        // Check if we already have accounts in file storage
        let existing_accounts = self.account_store.list_accounts().await?;
        if !existing_accounts.is_empty() {
            debug!("Accounts already exist in file storage, skipping migration");
            return Ok(());
        }

        // Fetch accounts from database (using raw query to avoid compile-time checks)
        let rows = sqlx::query(
            r#"
            SELECT
                id, display_name, email_address, provider_type,
                imap_host, imap_port, imap_user, imap_pass, imap_use_tls,
                smtp_host, smtp_port, smtp_user, smtp_pass,
                smtp_use_tls, smtp_use_starttls,
                is_active, is_default
            FROM accounts
            "#
        )
        .fetch_all(db)
        .await?;

        if rows.is_empty() {
            debug!("No accounts in database to migrate");
            return Ok(());
        }

        let account_count = rows.len();
        info!("Migrating {} accounts from database to file storage", account_count);

        // Migrate each account
        let mut default_account_id: Option<String> = None;

        for row in rows {
            let _id: i64 = row.get("id");
            let display_name: String = row.get("display_name");
            let email_address: String = row.get("email_address");
            let provider_type: Option<String> = row.get("provider_type");
            let imap_host: String = row.get("imap_host");
            let imap_port: i64 = row.get("imap_port");
            let imap_user: String = row.get("imap_user");
            let imap_pass: String = row.get("imap_pass");
            let imap_use_tls: i32 = row.get("imap_use_tls");
            let smtp_host: Option<String> = row.get("smtp_host");
            let smtp_port: Option<i64> = row.get("smtp_port");
            let smtp_user: Option<String> = row.get("smtp_user");
            let smtp_pass: Option<String> = row.get("smtp_pass");
            let smtp_use_tls: Option<i32> = row.get("smtp_use_tls");
            let smtp_use_starttls: Option<i32> = row.get("smtp_use_starttls");
            let is_active: i32 = row.get("is_active");
            let is_default: i32 = row.get("is_default");

            let stored_account = StoredAccount {
                display_name: display_name.clone(),
                email_address: email_address.clone(),
                provider_type,
                imap: super::account_store::ImapConfig {
                    host: imap_host,
                    port: imap_port as u16,
                    username: imap_user,
                    password: imap_pass,
                    use_tls: imap_use_tls != 0,
                },
                smtp: smtp_host.map(|host| {
                    super::account_store::SmtpConfig {
                        host,
                        port: smtp_port.unwrap_or(587) as u16,
                        username: smtp_user.unwrap_or_default(),
                        password: smtp_pass.unwrap_or_default(),
                        use_tls: smtp_use_tls.map(|v| v != 0).unwrap_or(true),
                        use_starttls: smtp_use_starttls.map(|v| v != 0).unwrap_or(true),
                    }
                }),
                is_active: is_active != 0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            self.account_store.add_account(stored_account).await?;

            if is_default != 0 {
                default_account_id = Some(email_address);
            }
        }

        // Set default account if one was marked
        if let Some(default_id) = default_account_id {
            self.account_store.set_default_account(&default_id).await?;
        }

        info!("Successfully migrated {} accounts to file storage", account_count);
        Ok(())
    }

    /// Sync accounts FROM file storage TO database cache
    async fn sync_accounts_to_db(&self) -> Result<(), AccountError> {
        let db = self.db()?;

        // Get all accounts from file storage
        let file_accounts = self.account_store.list_accounts().await?;

        if file_accounts.is_empty() {
            debug!("No accounts in file storage to sync");
            return Ok(());
        }

        info!("Syncing {} accounts from file storage to database", file_accounts.len());

        for account in file_accounts {
            // Check if account already exists in database by email_address
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM accounts WHERE email_address = ?"
            )
            .bind(&account.email_address)
            .fetch_one(db)
            .await? > 0;

            if exists {
                // Update existing account
                sqlx::query(
                    r#"
                    UPDATE accounts
                    SET display_name = ?, provider_type = ?,
                        imap_host = ?, imap_port = ?, imap_user = ?, imap_pass = ?, imap_use_tls = ?,
                        smtp_host = ?, smtp_port = ?, smtp_user = ?, smtp_pass = ?,
                        smtp_use_tls = ?, smtp_use_starttls = ?,
                        is_active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE email_address = ?
                    "#
                )
                .bind(&account.display_name)
                .bind(&account.provider_type)
                .bind(&account.imap.host)
                .bind(account.imap.port as i64)
                .bind(&account.imap.username)
                .bind(&account.imap.password)
                .bind(if account.imap.use_tls { 1 } else { 0 })
                .bind(account.smtp.as_ref().map(|s| &s.host))
                .bind(account.smtp.as_ref().map(|s| s.port as i64))
                .bind(account.smtp.as_ref().map(|s| &s.username))
                .bind(account.smtp.as_ref().map(|s| &s.password))
                .bind(account.smtp.as_ref().map(|s| if s.use_tls { 1 } else { 0 }))
                .bind(account.smtp.as_ref().map(|s| if s.use_starttls { 1 } else { 0 }))
                .bind(if account.is_active { 1 } else { 0 })
                .bind(&account.email_address)
                .execute(db)
                .await?;

                debug!("Updated account {} in database", account.email_address);
            } else {
                // Insert new account (email_address is the primary key)
                sqlx::query(
                    r#"
                    INSERT INTO accounts (
                        display_name, email_address, provider_type,
                        imap_host, imap_port, imap_user, imap_pass, imap_use_tls,
                        smtp_host, smtp_port, smtp_user, smtp_pass,
                        smtp_use_tls, smtp_use_starttls,
                        is_active, is_default, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    "#
                )
                .bind(&account.display_name)
                .bind(&account.email_address)
                .bind(&account.provider_type)
                .bind(&account.imap.host)
                .bind(account.imap.port as i64)
                .bind(&account.imap.username)
                .bind(&account.imap.password)
                .bind(if account.imap.use_tls { 1 } else { 0 })
                .bind(account.smtp.as_ref().map(|s| &s.host))
                .bind(account.smtp.as_ref().map(|s| s.port as i64))
                .bind(account.smtp.as_ref().map(|s| &s.username))
                .bind(account.smtp.as_ref().map(|s| &s.password))
                .bind(account.smtp.as_ref().map(|s| if s.use_tls { 1 } else { 0 }))
                .bind(account.smtp.as_ref().map(|s| if s.use_starttls { 1 } else { 0 }))
                .bind(if account.is_active { 1 } else { 0 })
                .execute(db)
                .await?;

                info!("Added account {} to database", account.email_address);
            }
        }

        info!("Successfully synced accounts to database");
        Ok(())
    }

    /// Get database pool or return error (only used for provider templates now)
    fn db(&self) -> Result<&SqlitePool, AccountError> {
        self.db_pool.as_ref().ok_or_else(|| {
            AccountError::OperationFailed("Database not initialized".to_string())
        })
    }

    /// Convert StoredAccount to Account (for API responses)
    fn stored_to_account(stored: StoredAccount) -> Account {
        Account {
            email_address: stored.email_address.clone(),
            id: stored.email_address, // Set id to match email_address for frontend
            display_name: stored.display_name,
            provider_type: stored.provider_type,
            imap_host: stored.imap.host,
            imap_port: stored.imap.port as i64,
            imap_user: stored.imap.username,
            imap_pass: stored.imap.password,
            imap_use_tls: stored.imap.use_tls,
            smtp_host: stored.smtp.as_ref().map(|s| s.host.clone()),
            smtp_port: stored.smtp.as_ref().map(|s| s.port as i64),
            smtp_user: stored.smtp.as_ref().map(|s| s.username.clone()),
            smtp_pass: stored.smtp.as_ref().map(|s| s.password.clone()),
            smtp_use_tls: stored.smtp.as_ref().map(|s| s.use_tls),
            smtp_use_starttls: stored.smtp.as_ref().map(|s| s.use_starttls),
            is_active: stored.is_active,
            is_default: false, // Will be set based on config default_account_id
            connection_status: None, // Will be populated from ConnectionStatusStore
        }
    }

    /// Auto-configure email settings based on email address
    pub async fn auto_configure(&self, email_address: &str) -> Result<AutoConfigResult, AccountError> {
        debug!("Auto-configuring for email: {}", email_address);

        // Extract domain from email
        let domain = Self::extract_domain(email_address)?;
        debug!("Extracted domain: {}", domain);

        // Query provider templates from database
        let template = self.find_provider_template(&domain).await?;

        match template {
            Some(tmpl) => {
                info!("Found provider template for domain: {} ({})", domain, tmpl.display_name);
                Ok(AutoConfigResult {
                    provider_found: true,
                    provider_type: Some(tmpl.provider_type),
                    display_name: Some(tmpl.display_name),
                    imap_host: Some(tmpl.imap_host),
                    imap_port: Some(tmpl.imap_port),
                    imap_use_tls: Some(tmpl.imap_use_tls),
                    smtp_host: Some(tmpl.smtp_host),
                    smtp_port: Some(tmpl.smtp_port),
                    smtp_use_tls: Some(tmpl.smtp_use_tls),
                    smtp_use_starttls: Some(tmpl.smtp_use_starttls),
                    supports_oauth: tmpl.supports_oauth,
                    oauth_provider: tmpl.oauth_provider,
                })
            }
            None => {
                info!("No provider template found for domain: {}", domain);
                Ok(AutoConfigResult {
                    provider_found: false,
                    provider_type: None,
                    display_name: None,
                    imap_host: None,
                    imap_port: None,
                    imap_use_tls: None,
                    smtp_host: None,
                    smtp_port: None,
                    smtp_use_tls: None,
                    smtp_use_starttls: None,
                    supports_oauth: false,
                    oauth_provider: None,
                })
            }
        }
    }

    /// Extract domain from email address
    fn extract_domain(email: &str) -> Result<String, AccountError> {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(AccountError::InvalidEmail(format!("Invalid email format: {}", email)));
        }
        Ok(parts[1].to_lowercase())
    }

    /// Find provider template by domain
    async fn find_provider_template(&self, domain: &str) -> Result<Option<ProviderTemplate>, AccountError> {
        let db = self.db()?;

        // Query all provider templates
        let rows = sqlx::query!(
            r#"
            SELECT
                provider_type,
                display_name,
                domain_patterns,
                imap_host,
                imap_port,
                imap_use_tls,
                smtp_host,
                smtp_port,
                smtp_use_tls,
                smtp_use_starttls,
                supports_oauth,
                oauth_provider
            FROM provider_templates
            "#
        )
        .fetch_all(db)
        .await?;

        // Check each template's domain patterns
        for row in rows {
            let domain_patterns: Vec<String> = serde_json::from_str(&row.domain_patterns)?;

            // Check if domain matches any pattern
            if domain_patterns.iter().any(|pattern| domain == pattern) {
                return Ok(Some(ProviderTemplate {
                    provider_type: row.provider_type.unwrap(),
                    display_name: row.display_name,
                    domain_patterns,
                    imap_host: row.imap_host,
                    imap_port: row.imap_port,
                    imap_use_tls: row.imap_use_tls,
                    smtp_host: row.smtp_host,
                    smtp_port: row.smtp_port,
                    smtp_use_tls: row.smtp_use_tls,
                    smtp_use_starttls: row.smtp_use_starttls,
                    supports_oauth: row.supports_oauth,
                    oauth_provider: row.oauth_provider,
                }));
            }
        }

        Ok(None)
    }

    /// Create a new account
    pub async fn create_account(&self, account: Account) -> Result<String, AccountError> {
        // Use email address as the account ID for consistency
        let stored_account = StoredAccount {
            display_name: account.display_name.clone(),
            email_address: account.email_address.clone(),
            provider_type: account.provider_type.clone(),
            imap: super::account_store::ImapConfig {
                host: account.imap_host.clone(),
                port: account.imap_port as u16,
                username: account.imap_user.clone(),
                password: account.imap_pass.clone(),
                use_tls: account.imap_use_tls,
            },
            smtp: account.smtp_host.as_ref().map(|host| {
                super::account_store::SmtpConfig {
                    host: host.clone(),
                    port: account.smtp_port.unwrap_or(587) as u16,
                    username: account.smtp_user.clone().unwrap_or_default(),
                    password: account.smtp_pass.clone().unwrap_or_default(),
                    use_tls: account.smtp_use_tls.unwrap_or(true),
                    use_starttls: account.smtp_use_starttls.unwrap_or(true),
                }
            }),
            is_active: account.is_active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Clone email_address before moving stored_account
        let account_email = stored_account.email_address.clone();

        self.account_store.add_account(stored_account).await?;
        info!("Created account: {} ({})", account.display_name, account.email_address);

        // Sync to database cache so the account is immediately available for email operations
        if let Err(e) = self.sync_accounts_to_db().await {
            warn!("Failed to sync new account to database cache: {}", e);
            // Don't fail the account creation, but warn about it
        }

        Ok(account_email)
    }

    /// Get account by ID
    pub async fn get_account(&self, account_id: &str) -> Result<Account, AccountError> {
        let stored = self.account_store.get_account(account_id).await?;

        // Check if this is the default account
        let config = self.account_store.load_config().await?;
        let is_default = config.default_account_id.as_deref() == Some(account_id);

        let mut account = Self::stored_to_account(stored);
        account.is_default = is_default;

        // Populate connection status
        account.connection_status = Some(
            self.connection_status_store
                .get_status_or_default(account_id)
                .await
        );

        Ok(account)
    }

    /// List all accounts
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AccountError> {
        let stored_accounts = self.account_store.list_accounts().await?;
        let config = self.account_store.load_config().await?;

        let mut accounts = Vec::new();
        for stored in stored_accounts {
            let is_default = config.default_account_id.as_deref() == Some(&stored.email_address);
            let mut account = Self::stored_to_account(stored.clone());
            account.is_default = is_default;

            // Populate connection status
            account.connection_status = Some(
                self.connection_status_store
                    .get_status_or_default(&stored.email_address)
                    .await
            );

            accounts.push(account);
        }

        Ok(accounts)
    }

    /// Get default account
    pub async fn get_default_account(&self) -> Result<Option<Account>, AccountError> {
        match self.account_store.get_default_account().await? {
            Some(stored) => {
                let mut account = Self::stored_to_account(stored);
                account.is_default = true;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    /// Update account (requires account_id as string)
    pub async fn update_account(&self, account_id: &str, account: Account) -> Result<(), AccountError> {
        // Get existing account to preserve created_at timestamp
        let existing = self.account_store.get_account(account_id).await?;

        let updated = StoredAccount {
            display_name: account.display_name.clone(),
            email_address: account.email_address.clone(),
            provider_type: account.provider_type.clone(),
            imap: super::account_store::ImapConfig {
                host: account.imap_host.clone(),
                port: account.imap_port as u16,
                username: account.imap_user.clone(),
                password: account.imap_pass.clone(),
                use_tls: account.imap_use_tls,
            },
            smtp: account.smtp_host.as_ref().map(|host| {
                super::account_store::SmtpConfig {
                    host: host.clone(),
                    port: account.smtp_port.unwrap_or(587) as u16,
                    username: account.smtp_user.clone().unwrap_or_default(),
                    password: account.smtp_pass.clone().unwrap_or_default(),
                    use_tls: account.smtp_use_tls.unwrap_or(true),
                    use_starttls: account.smtp_use_starttls.unwrap_or(true),
                }
            }),
            is_active: account.is_active,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        };

        self.account_store.update_account(updated).await?;
        info!("Updated account: {} ({})", account.display_name, account.email_address);
        Ok(())
    }

    /// Delete account
    pub async fn delete_account(&self, account_id: &str) -> Result<(), AccountError> {
        self.account_store.delete_account(account_id).await?;
        info!("Deleted account ID: {}", account_id);
        Ok(())
    }

    /// Set default account
    pub async fn set_default_account(&self, account_id: &str) -> Result<(), AccountError> {
        self.account_store.set_default_account(account_id).await?;
        info!("Set default account to ID: {}", account_id);
        Ok(())
    }

    /// Validate account credentials by attempting to connect and record status
    pub async fn validate_connection(&self, account: &Account) -> Result<(), AccountError> {
        use std::time::Duration;
        debug!("Validating connection for account: {}", account.display_name);

        // Attempt to connect using the provided credentials with 10 second timeout
        let timeout = Duration::from_secs(10);
        let connect_result = crate::imap::client::connect(
            &account.imap_host,
            account.imap_port as u16,
            &account.imap_user,
            &account.imap_pass,
            timeout,
        ).await;

        // Record connection status
        let mut status = self.connection_status_store
            .get_status_or_default(&account.email_address)
            .await;

        match connect_result {
            Ok(client) => {
                // Pre-create essential folders (Outbox, Sent, Drafts)
                // This ensures folders exist before first use, avoiding timeout issues during email send
                if let Err(e) = self.ensure_essential_folders_exist(&client).await {
                    warn!("Failed to create essential folders during validation: {}", e);
                    // Don't fail validation if folder creation fails - just log warning
                }

                // Successfully connected, logout gracefully
                if let Err(e) = client.logout().await {
                    debug!("Logout error during validation (non-critical): {}", e);
                }

                // Record success
                status.set_imap_success(format!("Connected to {} successfully", account.imap_host));
                if let Err(e) = self.connection_status_store.update_status(status).await {
                    warn!("Failed to record connection status: {}", e);
                }

                info!("Connection validation successful for: {}", account.display_name);
                Ok(())
            }
            Err(e) => {
                // Record failure
                status.set_imap_failed(&e);
                if let Err(err) = self.connection_status_store.update_status(status).await {
                    warn!("Failed to record connection status: {}", err);
                }

                error!("Connection validation failed for {}: {}", account.display_name, e);
                Err(AccountError::OperationFailed(format!("Connection failed: {}", e)))
            }
        }
    }

    /// Ensure essential IMAP folders exist (Outbox, Sent, Drafts)
    /// This prevents timeout issues during first email send operations
    async fn ensure_essential_folders_exist(
        &self,
        client: &crate::imap::client::ImapClient<crate::imap::session::AsyncImapSessionWrapper>,
    ) -> Result<(), AccountError> {
        // List of essential folders to pre-create
        let essential_folders = vec!["INBOX.Outbox", "INBOX.Sent", "INBOX.Drafts"];

        for folder_name in essential_folders {
            match client.create_folder(folder_name).await {
                Ok(_) => {
                    info!("Created essential folder: {}", folder_name);
                }
                Err(e) => {
                    // Check if error is "folder already exists" - this is not an error
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("already exists") || error_str.contains("alreadyexists") {
                        debug!("Folder {} already exists (expected)", folder_name);
                    } else {
                        warn!("Failed to create folder {}: {}", folder_name, e);
                        // Continue trying to create other folders even if one fails
                    }
                }
            }
        }

        Ok(())
    }

    /// Get connection status for an account
    pub async fn get_connection_status(
        &self,
        account_id: &str,
    ) -> Result<AccountConnectionStatus, AccountError> {
        Ok(self.connection_status_store.get_status_or_default(account_id).await)
    }

    /// Update IMAP connection status for an account
    pub async fn update_imap_status(
        &self,
        account_id: &str,
        success: bool,
        message: impl Into<String>,
    ) -> Result<(), AccountError> {
        let mut status = self.connection_status_store.get_status_or_default(account_id).await;

        if success {
            status.set_imap_success(message);
        } else {
            status.set_imap_failed(message.into());
        }

        self.connection_status_store.update_status(status).await?;
        Ok(())
    }

    /// Update SMTP connection status for an account
    pub async fn update_smtp_status(
        &self,
        account_id: &str,
        success: bool,
        message: impl Into<String>,
    ) -> Result<(), AccountError> {
        let mut status = self.connection_status_store.get_status_or_default(account_id).await;

        if success {
            status.set_smtp_success(message);
        } else {
            status.set_smtp_failed(message.into());
        }

        self.connection_status_store.update_status(status).await?;
        Ok(())
    }
}
