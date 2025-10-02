use sqlx::SqlitePool;
use log::{info, debug, error};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use std::time::Duration;

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
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
}

impl AccountService {
    pub fn new() -> Self {
        Self { db_pool: None }
    }

    /// Initialize the account service with database pool
    pub async fn initialize(&mut self, db_pool: SqlitePool) -> Result<(), AccountError> {
        self.db_pool = Some(db_pool);
        info!("Account service initialized");
        Ok(())
    }

    /// Get database pool or return error
    fn db(&self) -> Result<&SqlitePool, AccountError> {
        self.db_pool.as_ref().ok_or_else(|| {
            AccountError::OperationFailed("Database not initialized".to_string())
        })
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
    pub async fn create_account(&self, account: Account) -> Result<i64, AccountError> {
        let db = self.db()?;

        let result = sqlx::query!(
            r#"
            INSERT INTO accounts (
                account_name, email_address, provider_type,
                imap_host, imap_port, imap_user, imap_pass, imap_use_tls,
                smtp_host, smtp_port, smtp_user, smtp_pass, smtp_use_tls, smtp_use_starttls,
                is_active, is_default
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            account.account_name,
            account.email_address,
            account.provider_type,
            account.imap_host,
            account.imap_port,
            account.imap_user,
            account.imap_pass,
            account.imap_use_tls,
            account.smtp_host,
            account.smtp_port,
            account.smtp_user,
            account.smtp_pass,
            account.smtp_use_tls,
            account.smtp_use_starttls,
            account.is_active,
            account.is_default
        )
        .execute(db)
        .await?;

        info!("Created account: {} ({})", account.account_name, account.email_address);
        Ok(result.last_insert_rowid())
    }

    /// Get account by ID
    pub async fn get_account(&self, account_id: i64) -> Result<Account, AccountError> {
        let db = self.db()?;

        let row = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                account_name as "account_name!",
                email_address as "email_address!",
                provider_type,
                imap_host as "imap_host!",
                imap_port as "imap_port!",
                imap_user as "imap_user!",
                imap_pass as "imap_pass!",
                imap_use_tls as "imap_use_tls!",
                smtp_host,
                smtp_port,
                smtp_user,
                smtp_pass,
                smtp_use_tls,
                smtp_use_starttls,
                is_active as "is_active!",
                is_default as "is_default!"
            FROM accounts
            WHERE id = ?
            "#,
            account_id
        )
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AccountError::NotFound(format!("Account ID: {}", account_id)))?;

        Ok(Account {
            id: row.id,
            account_name: row.account_name,
            email_address: row.email_address,
            provider_type: row.provider_type,
            imap_host: row.imap_host,
            imap_port: row.imap_port,
            imap_user: row.imap_user,
            imap_pass: row.imap_pass,
            imap_use_tls: row.imap_use_tls,
            smtp_host: row.smtp_host,
            smtp_port: row.smtp_port,
            smtp_user: row.smtp_user,
            smtp_pass: row.smtp_pass,
            smtp_use_tls: row.smtp_use_tls,
            smtp_use_starttls: row.smtp_use_starttls,
            is_active: row.is_active,
            is_default: row.is_default,
        })
    }

    /// List all accounts
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AccountError> {
        let db = self.db()?;

        let rows = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                account_name as "account_name!",
                email_address as "email_address!",
                provider_type,
                imap_host as "imap_host!",
                imap_port as "imap_port!",
                imap_user as "imap_user!",
                imap_pass as "imap_pass!",
                imap_use_tls as "imap_use_tls!",
                smtp_host,
                smtp_port,
                smtp_user,
                smtp_pass,
                smtp_use_tls,
                smtp_use_starttls,
                is_active as "is_active!",
                is_default as "is_default!"
            FROM accounts
            ORDER BY is_default DESC, account_name ASC
            "#
        )
        .fetch_all(db)
        .await?;

        let accounts = rows
            .into_iter()
            .map(|row| Account {
                id: row.id,
                account_name: row.account_name,
                email_address: row.email_address,
                provider_type: row.provider_type,
                imap_host: row.imap_host,
                imap_port: row.imap_port,
                imap_user: row.imap_user,
                imap_pass: row.imap_pass,
                imap_use_tls: row.imap_use_tls,
                smtp_host: row.smtp_host,
                smtp_port: row.smtp_port,
                smtp_user: row.smtp_user,
                smtp_pass: row.smtp_pass,
                smtp_use_tls: row.smtp_use_tls,
                smtp_use_starttls: row.smtp_use_starttls,
                is_active: row.is_active,
                is_default: row.is_default,
            })
            .collect();

        Ok(accounts)
    }

    /// Get default account
    pub async fn get_default_account(&self) -> Result<Option<Account>, AccountError> {
        let db = self.db()?;

        let row = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                account_name as "account_name!",
                email_address as "email_address!",
                provider_type,
                imap_host as "imap_host!",
                imap_port as "imap_port!",
                imap_user as "imap_user!",
                imap_pass as "imap_pass!",
                imap_use_tls as "imap_use_tls!",
                smtp_host,
                smtp_port,
                smtp_user,
                smtp_pass,
                smtp_use_tls,
                smtp_use_starttls,
                is_active as "is_active!",
                is_default as "is_default!"
            FROM accounts
            WHERE is_default = TRUE
            LIMIT 1
            "#
        )
        .fetch_optional(db)
        .await?;

        Ok(row.map(|row| Account {
            id: row.id,
            account_name: row.account_name,
            email_address: row.email_address,
            provider_type: row.provider_type,
            imap_host: row.imap_host,
            imap_port: row.imap_port,
            imap_user: row.imap_user,
            imap_pass: row.imap_pass,
            imap_use_tls: row.imap_use_tls,
            smtp_host: row.smtp_host,
            smtp_port: row.smtp_port,
            smtp_user: row.smtp_user,
            smtp_pass: row.smtp_pass,
            smtp_use_tls: row.smtp_use_tls,
            smtp_use_starttls: row.smtp_use_starttls,
            is_active: row.is_active,
            is_default: row.is_default,
        }))
    }

    /// Update account
    pub async fn update_account(&self, account: Account) -> Result<(), AccountError> {
        let db = self.db()?;

        sqlx::query!(
            r#"
            UPDATE accounts
            SET account_name = ?, email_address = ?, provider_type = ?,
                imap_host = ?, imap_port = ?, imap_user = ?, imap_pass = ?, imap_use_tls = ?,
                smtp_host = ?, smtp_port = ?, smtp_user = ?, smtp_pass = ?,
                smtp_use_tls = ?, smtp_use_starttls = ?,
                is_active = ?, is_default = ?
            WHERE id = ?
            "#,
            account.account_name,
            account.email_address,
            account.provider_type,
            account.imap_host,
            account.imap_port,
            account.imap_user,
            account.imap_pass,
            account.imap_use_tls,
            account.smtp_host,
            account.smtp_port,
            account.smtp_user,
            account.smtp_pass,
            account.smtp_use_tls,
            account.smtp_use_starttls,
            account.is_active,
            account.is_default,
            account.id
        )
        .execute(db)
        .await?;

        info!("Updated account: {} ({})", account.account_name, account.email_address);
        Ok(())
    }

    /// Delete account
    pub async fn delete_account(&self, account_id: i64) -> Result<(), AccountError> {
        let db = self.db()?;

        let result = sqlx::query!(
            r#"
            DELETE FROM accounts WHERE id = ?
            "#,
            account_id
        )
        .execute(db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AccountError::NotFound(format!("Account ID: {}", account_id)));
        }

        info!("Deleted account ID: {}", account_id);
        Ok(())
    }

    /// Set default account
    pub async fn set_default_account(&self, account_id: i64) -> Result<(), AccountError> {
        let db = self.db()?;

        // Start transaction
        let mut tx = db.begin().await?;

        // Clear all default flags
        sqlx::query!("UPDATE accounts SET is_default = FALSE")
            .execute(&mut *tx)
            .await?;

        // Set new default
        let result = sqlx::query!(
            "UPDATE accounts SET is_default = TRUE WHERE id = ?",
            account_id
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AccountError::NotFound(format!("Account ID: {}", account_id)));
        }

        // Commit transaction
        tx.commit().await?;

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
