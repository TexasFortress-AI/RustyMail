use sqlx::SqlitePool;
use log::{info, debug, error};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use super::account_store::{AccountStore, StoredAccount, AccountStoreError};
use chrono::Utc;

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Account store error: {0}")]
    AccountStoreError(#[from] AccountStoreError),
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
    pub id: i64,
    pub account_name: String,
    pub email_address: String,
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
    pub is_active: bool,
    pub is_default: bool,
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
}

impl AccountService {
    pub fn new(config_path: &str) -> Self {
        Self {
            db_pool: None,
            account_store: AccountStore::new(config_path),
        }
    }

    /// Initialize the account service with database pool
    pub async fn initialize(&mut self, db_pool: SqlitePool) -> Result<(), AccountError> {
        self.db_pool = Some(db_pool);
        self.account_store.initialize().await?;
        info!("Account service initialized with file-based storage");
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
            id: 0, // ID is string-based in StoredAccount, not used in Account anymore
            account_name: stored.account_name,
            email_address: stored.email_address,
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
                    provider_type: row.provider_type,
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
        let account_id = AccountStore::generate_account_id();

        let stored_account = StoredAccount {
            id: account_id.clone(),
            account_name: account.account_name.clone(),
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

        self.account_store.add_account(stored_account).await?;
        info!("Created account: {} ({})", account.account_name, account.email_address);
        Ok(account_id)
    }

    /// Get account by ID
    pub async fn get_account(&self, account_id: &str) -> Result<Account, AccountError> {
        let stored = self.account_store.get_account(account_id).await?;

        // Check if this is the default account
        let config = self.account_store.load_config().await?;
        let is_default = config.default_account_id.as_deref() == Some(account_id);

        let mut account = Self::stored_to_account(stored);
        account.is_default = is_default;
        Ok(account)
    }

    /// List all accounts
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AccountError> {
        let stored_accounts = self.account_store.list_accounts().await?;
        let config = self.account_store.load_config().await?;

        let accounts = stored_accounts
            .into_iter()
            .map(|stored| {
                let is_default = config.default_account_id.as_deref() == Some(&stored.id);
                let mut account = Self::stored_to_account(stored);
                account.is_default = is_default;
                account
            })
            .collect();

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
        // Get existing account to preserve id
        let existing = self.account_store.get_account(account_id).await?;

        let updated = StoredAccount {
            id: existing.id,
            account_name: account.account_name.clone(),
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
        info!("Updated account: {} ({})", account.account_name, account.email_address);
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

    /// Validate account credentials by attempting to connect
    pub async fn validate_connection(&self, account: &Account) -> Result<(), AccountError> {
        debug!("Validating connection for account: {}", account.account_name);

        // Attempt to connect using the provided credentials with 10 second timeout
        let timeout = Duration::from_secs(10);
        let connect_result = crate::imap::client::connect(
            &account.imap_host,
            account.imap_port as u16,
            &account.imap_user,
            &account.imap_pass,
            timeout,
        ).await;

        match connect_result {
            Ok(client) => {
                // Successfully connected, logout gracefully
                if let Err(e) = client.logout().await {
                    debug!("Logout error during validation (non-critical): {}", e);
                }
                info!("Connection validation successful for: {}", account.account_name);
                Ok(())
            }
            Err(e) => {
                error!("Connection validation failed for {}: {}", account.account_name, e);
                Err(AccountError::OperationFailed(format!("Connection failed: {}", e)))
            }
        }
    }
}
