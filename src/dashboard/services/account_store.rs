// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::{Path, PathBuf};
use tokio::fs as async_fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use log::{info, debug, warn};
use thiserror::Error;
use super::encryption::CredentialEncryption;

#[derive(Error, Debug)]
pub enum AccountStoreError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Account not found: {0}")]
    NotFound(String),
    #[error("Invalid account ID: {0}")]
    InvalidId(String),
    #[error("Duplicate account: {0}")]
    DuplicateAccount(String),
    #[error("Store operation failed: {0}")]
    OperationFailed(String),
    #[error("Encryption error: {0}")]
    EncryptionError(#[from] super::encryption::EncryptionError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[default]
    Password,
    OAuth2,
    AppPassword,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub password: String,
    pub use_tls: bool,
    pub use_starttls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    // email_address is the primary identifier - id field removed
    #[serde(rename = "display_name", alias = "account_name")]
    pub display_name: String,
    pub email_address: String,
    pub provider_type: Option<String>,
    pub imap: ImapConfig,
    pub smtp: Option<SmtpConfig>,
    /// OAuth provider identifier (e.g., "microsoft"). If set, XOAUTH2 is used instead of passwords.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oauth_provider: Option<String>,
    /// OAuth access token (encrypted at rest).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oauth_access_token: Option<String>,
    /// OAuth refresh token (encrypted at rest).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oauth_refresh_token: Option<String>,
    /// Unix timestamp (seconds) when the OAuth access token expires.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oauth_token_expiry: Option<i64>,
    pub is_active: bool,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl StoredAccount {
    /// Returns true if this account uses OAuth2 authentication.
    pub fn is_oauth(&self) -> bool {
        self.oauth_provider.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountsConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub default_account_id: Option<String>,
    #[serde(default)]
    pub accounts: Vec<StoredAccount>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl Default for AccountsConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            default_account_id: None,
            accounts: Vec::new(),
        }
    }
}

pub struct AccountStore {
    config_path: PathBuf,
    encryption: CredentialEncryption,
}

impl AccountStore {
    /// Create a new AccountStore with the given config file path
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        let encryption = CredentialEncryption::new();
        if encryption.is_enabled() {
            info!("AccountStore initialized with credential encryption enabled");
        } else {
            warn!("AccountStore initialized WITHOUT credential encryption - set ENCRYPTION_MASTER_KEY to enable");
        }
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            encryption,
        }
    }

    /// Initialize the account store, creating the file if it doesn't exist
    pub async fn initialize(&self) -> Result<(), AccountStoreError> {
        if !self.config_path.exists() {
            info!("Creating new accounts config file at: {:?}", self.config_path);

            // Create parent directory if it doesn't exist
            if let Some(parent) = self.config_path.parent() {
                async_fs::create_dir_all(parent).await?;
            }

            // Create empty config
            let config = AccountsConfig::default();
            self.save_config(&config).await?;

            // Set restrictive permissions (owner read/write only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = async_fs::metadata(&self.config_path).await?;
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o600); // rw-------
                async_fs::set_permissions(&self.config_path, permissions).await?;
                info!("Set restrictive permissions (0600) on accounts config file");
            }
        }

        Ok(())
    }

    /// Load accounts configuration from file, decrypting passwords
    pub async fn load_config(&self) -> Result<AccountsConfig, AccountStoreError> {
        debug!("Loading accounts config from: {:?}", self.config_path);

        let contents = async_fs::read_to_string(&self.config_path).await?;
        let mut config: AccountsConfig = serde_json::from_str(&contents)?;

        // Decrypt passwords and OAuth tokens for all accounts
        for account in &mut config.accounts {
            // Decrypt IMAP password
            if !account.imap.password.is_empty() {
                account.imap.password = self.encryption.decrypt(&account.imap.password)?;
            }
            // Decrypt SMTP password if present
            if let Some(smtp) = &mut account.smtp {
                if !smtp.password.is_empty() {
                    smtp.password = self.encryption.decrypt(&smtp.password)?;
                }
            }
            // Decrypt OAuth tokens if present
            if let Some(token) = &account.oauth_access_token {
                if !token.is_empty() {
                    account.oauth_access_token = Some(self.encryption.decrypt(token)?);
                }
            }
            if let Some(token) = &account.oauth_refresh_token {
                if !token.is_empty() {
                    account.oauth_refresh_token = Some(self.encryption.decrypt(token)?);
                }
            }
        }

        debug!("Loaded {} accounts from config", config.accounts.len());
        Ok(config)
    }

    /// Save accounts configuration to file (atomic write), encrypting passwords
    async fn save_config(&self, config: &AccountsConfig) -> Result<(), AccountStoreError> {
        debug!("Saving accounts config to: {:?}", self.config_path);

        // Clone config and encrypt passwords/tokens before saving
        let mut encrypted_config = config.clone();
        for account in &mut encrypted_config.accounts {
            // Encrypt IMAP password (skip if already encrypted or empty)
            if !account.imap.password.is_empty() && !account.imap.password.starts_with("ENC:") {
                account.imap.password = self.encryption.encrypt(&account.imap.password)?;
            }
            // Encrypt SMTP password if present
            if let Some(smtp) = &mut account.smtp {
                if !smtp.password.is_empty() && !smtp.password.starts_with("ENC:") {
                    smtp.password = self.encryption.encrypt(&smtp.password)?;
                }
            }
            // Encrypt OAuth tokens if present
            if let Some(token) = &account.oauth_access_token {
                if !token.is_empty() && !token.starts_with("ENC:") {
                    account.oauth_access_token = Some(self.encryption.encrypt(token)?);
                }
            }
            if let Some(token) = &account.oauth_refresh_token {
                if !token.is_empty() && !token.starts_with("ENC:") {
                    account.oauth_refresh_token = Some(self.encryption.encrypt(token)?);
                }
            }
        }

        // Serialize to JSON with pretty printing
        let json = serde_json::to_string_pretty(&encrypted_config)?;

        // Write to temporary file first (atomic write)
        let temp_path = self.config_path.with_extension("tmp");
        let _file = async_fs::File::create(&temp_path).await?;
        async_fs::write(&temp_path, json.as_bytes()).await?;

        // Set restrictive permissions on temp file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = async_fs::metadata(&temp_path).await?;
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            async_fs::set_permissions(&temp_path, permissions).await?;
        }

        // Atomic rename
        async_fs::rename(&temp_path, &self.config_path).await?;

        info!("Saved {} accounts to config (credentials encrypted: {})",
            config.accounts.len(), self.encryption.is_enabled());
        Ok(())
    }

    /// Add a new account
    pub async fn add_account(&self, account: StoredAccount) -> Result<(), AccountStoreError> {
        let mut config = self.load_config().await?;

        // Check for duplicate email (primary identifier)
        if config.accounts.iter().any(|a| a.email_address == account.email_address) {
            return Err(AccountStoreError::DuplicateAccount(account.email_address.clone()));
        }

        config.accounts.push(account);
        self.save_config(&config).await?;

        Ok(())
    }

    /// Get account by email address
    pub async fn get_account(&self, email_address: &str) -> Result<StoredAccount, AccountStoreError> {
        let config = self.load_config().await?;

        config.accounts
            .into_iter()
            .find(|a| a.email_address == email_address)
            .ok_or_else(|| AccountStoreError::NotFound(email_address.to_string()))
    }

    /// List all accounts
    pub async fn list_accounts(&self) -> Result<Vec<StoredAccount>, AccountStoreError> {
        let config = self.load_config().await?;
        Ok(config.accounts)
    }

    /// Update an existing account (matched by email_address)
    pub async fn update_account(&self, account: StoredAccount) -> Result<(), AccountStoreError> {
        let mut config = self.load_config().await?;

        let pos = config.accounts
            .iter()
            .position(|a| a.email_address == account.email_address)
            .ok_or_else(|| AccountStoreError::NotFound(account.email_address.clone()))?;

        config.accounts[pos] = account;
        self.save_config(&config).await?;

        Ok(())
    }

    /// Delete an account by email address
    pub async fn delete_account(&self, email_address: &str) -> Result<(), AccountStoreError> {
        let mut config = self.load_config().await?;

        let initial_len = config.accounts.len();
        config.accounts.retain(|a| a.email_address != email_address);

        if config.accounts.len() == initial_len {
            return Err(AccountStoreError::NotFound(email_address.to_string()));
        }

        // If we deleted the default account, clear the default
        if config.default_account_id.as_deref() == Some(email_address) {
            config.default_account_id = None;
        }

        self.save_config(&config).await?;

        Ok(())
    }

    /// Get the default account
    pub async fn get_default_account(&self) -> Result<Option<StoredAccount>, AccountStoreError> {
        let config = self.load_config().await?;

        if let Some(default_email) = &config.default_account_id {
            Ok(config.accounts.into_iter().find(|a| &a.email_address == default_email))
        } else {
            Ok(None)
        }
    }

    /// Set the default account by email address
    pub async fn set_default_account(&self, email_address: &str) -> Result<(), AccountStoreError> {
        let mut config = self.load_config().await?;

        // Verify account exists
        if !config.accounts.iter().any(|a| a.email_address == email_address) {
            return Err(AccountStoreError::NotFound(email_address.to_string()));
        }

        config.default_account_id = Some(email_address.to_string());
        self.save_config(&config).await?;

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_account_store_crud() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("accounts.json");
        let store = AccountStore::new(&config_path);

        // Initialize
        store.initialize().await.unwrap();
        assert!(config_path.exists());

        // Add account
        let account = StoredAccount {
            display_name: "Test Account".to_string(),
            email_address: "test@example.com".to_string(),
            provider_type: Some("gmail".to_string()),
            imap: ImapConfig {
                host: "imap.gmail.com".to_string(),
                port: 993,
                username: "test@example.com".to_string(),
                password: "password".to_string(),
                use_tls: true,
            },
            smtp: None,
            oauth_provider: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            oauth_token_expiry: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.add_account(account.clone()).await.unwrap();

        // Get account by email
        let retrieved = store.get_account("test@example.com").await.unwrap();
        assert_eq!(retrieved.email_address, "test@example.com");

        // List accounts
        let accounts = store.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);

        // Set default using email address
        store.set_default_account("test@example.com").await.unwrap();
        let default = store.get_default_account().await.unwrap();
        assert!(default.is_some());

        // Delete account by email
        store.delete_account("test@example.com").await.unwrap();
        let accounts = store.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 0);
    }

    #[tokio::test]
    async fn test_oauth_account_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("accounts.json");
        let store = AccountStore::new(&config_path);
        store.initialize().await.unwrap();

        let account = StoredAccount {
            display_name: "OAuth Test".to_string(),
            email_address: "user@outlook.com".to_string(),
            provider_type: Some("outlook".to_string()),
            imap: ImapConfig {
                host: "outlook.office365.com".to_string(),
                port: 993,
                username: "user@outlook.com".to_string(),
                password: String::new(), // OAuth accounts don't use passwords
                use_tls: true,
            },
            smtp: None,
            oauth_provider: Some("microsoft".to_string()),
            oauth_access_token: Some("test-access-token".to_string()),
            oauth_refresh_token: Some("test-refresh-token".to_string()),
            oauth_token_expiry: Some(1700000000),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.add_account(account).await.unwrap();

        let retrieved = store.get_account("user@outlook.com").await.unwrap();
        assert!(retrieved.is_oauth());
        assert_eq!(retrieved.oauth_provider.as_deref(), Some("microsoft"));
        assert_eq!(retrieved.oauth_access_token.as_deref(), Some("test-access-token"));
        assert_eq!(retrieved.oauth_refresh_token.as_deref(), Some("test-refresh-token"));
        assert_eq!(retrieved.oauth_token_expiry, Some(1700000000));
        // Password should be empty for OAuth accounts
        assert!(retrieved.imap.password.is_empty());
    }

    #[test]
    fn test_is_oauth() {
        let mut account = StoredAccount {
            display_name: "Test".to_string(),
            email_address: "test@test.com".to_string(),
            provider_type: None,
            imap: ImapConfig {
                host: "imap.test.com".to_string(),
                port: 993,
                username: "test".to_string(),
                password: "pass".to_string(),
                use_tls: true,
            },
            smtp: None,
            oauth_provider: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            oauth_token_expiry: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(!account.is_oauth());

        account.oauth_provider = Some("microsoft".to_string());
        assert!(account.is_oauth());
    }
}
