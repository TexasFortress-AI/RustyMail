use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use chrono::{DateTime, Utc};
use lru::LruCache;
use std::num::NonZeroUsize;
use log::{info, error, debug, warn};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use crate::imap::types::{Email, Address};

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Cache not initialized")]
    NotInitialized,
    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFolder {
    pub id: i64,
    pub name: String,
    pub delimiter: Option<String>,
    pub attributes: Vec<String>,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub total_messages: i32,
    pub unseen_messages: i32,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEmail {
    pub id: i64,
    pub folder_id: i64,
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub from_name: Option<String>,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub date: Option<DateTime<Utc>>,
    pub internal_date: Option<DateTime<Utc>>,
    pub size: Option<i64>,
    pub flags: Vec<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub cached_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SyncState {
    pub folder_id: i64,
    pub last_uid_synced: Option<u32>,
    pub last_full_sync: Option<DateTime<Utc>>,
    pub last_incremental_sync: Option<DateTime<Utc>>,
    pub sync_status: SyncStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Error,
}

pub struct CacheService {
    db_pool: Option<SqlitePool>,
    memory_cache: Arc<RwLock<LruCache<String, CachedEmail>>>,
    folder_cache: Arc<RwLock<HashMap<String, CachedFolder>>>,
    config: CacheConfig,
}

impl std::fmt::Debug for CacheService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheService")
            .field("db_pool", &self.db_pool.is_some())
            .field("memory_cache", &"<LruCache>")
            .field("folder_cache", &"<HashMap>")
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub database_url: String,
    pub max_memory_items: usize,
    pub max_cache_size_mb: u64,
    pub max_email_age_days: u32,
    pub sync_interval_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite:data/email_cache.db".to_string(),
            max_memory_items: 1000,
            max_cache_size_mb: 1000,
            max_email_age_days: 30,
            sync_interval_seconds: 300,
        }
    }
}

impl CacheService {
    pub fn new(config: CacheConfig) -> Self {
        let memory_cache = Arc::new(RwLock::new(
            LruCache::new(NonZeroUsize::new(config.max_memory_items).unwrap())
        ));
        let folder_cache = Arc::new(RwLock::new(HashMap::new()));

        Self {
            db_pool: None,
            memory_cache,
            folder_cache,
            config,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), CacheError> {
        info!("Initializing cache service with database: {}", self.config.database_url);

        // Extract the file path from the database URL
        let db_path = self.config.database_url.replace("sqlite:", "");
        let path = std::path::Path::new(&db_path);

        // Create data directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e|
                CacheError::OperationFailed(format!("Failed to create data directory: {}", e))
            )?;
        }

        // Create the database file if it doesn't exist
        if !path.exists() {
            info!("Database file doesn't exist, creating: {}", db_path);
            std::fs::File::create(&db_path).map_err(|e|
                CacheError::OperationFailed(format!("Failed to create database file: {}", e))
            )?;
        }

        // Create database connection pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&self.config.database_url)
            .await?;

        // Run migrations to ensure tables exist
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| CacheError::OperationFailed(format!("Failed to run migrations: {}", e)))?;

        self.db_pool = Some(pool);

        // Load folders into cache
        self.load_folders_to_cache().await?;

        info!("Cache service initialized successfully");
        Ok(())
    }

    async fn load_folders_to_cache(&self) -> Result<(), CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let folders = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i32, i32, Option<DateTime<Utc>>)>(
            "SELECT id, name, delimiter, attributes, uidvalidity, uidnext, total_messages, unseen_messages, last_sync FROM folders"
        )
        .fetch_all(pool)
        .await?;

        let mut folder_cache = self.folder_cache.write().await;
        for (id, name, delimiter, attributes_json, uidvalidity, uidnext, total_messages, unseen_messages, last_sync) in folders {
            let attributes: Vec<String> = attributes_json
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            let cached_folder = CachedFolder {
                id,
                name: name.clone(),
                delimiter,
                attributes,
                uidvalidity,
                uidnext,
                total_messages,
                unseen_messages,
                last_sync,
            };

            folder_cache.insert(name, cached_folder);
        }

        Ok(())
    }

    /// Get account's numeric ID by email address
    pub async fn get_account_id_by_email(&self, email: &str) -> Result<i64, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM accounts WHERE email_address = ?"
        )
        .bind(email)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| CacheError::OperationFailed(format!("Account {} not found in database", email)))
    }

    /// Get or create a folder for a specific account
    pub async fn get_or_create_folder_for_account(&self, name: &str, account_id: i64) -> Result<CachedFolder, CacheError> {
        // Check memory cache first (keyed by account_id:folder_name for multi-account support)
        let cache_key = format!("{}:{}", account_id, name);
        {
            let folder_cache = self.folder_cache.read().await;
            if let Some(folder) = folder_cache.get(&cache_key) {
                return Ok(folder.clone());
            }
        }

        // Not in cache, check database
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let folder = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i32, i32, Option<DateTime<Utc>>)>(
            "SELECT id, name, delimiter, attributes, uidvalidity, uidnext, total_messages, unseen_messages, last_sync FROM folders WHERE name = ? AND account_id = ?"
        )
        .bind(name)
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        if let Some((id, name_str, delimiter, attributes_json, uidvalidity, uidnext, total_messages, unseen_messages, last_sync)) = folder {
            let attributes: Vec<String> = attributes_json
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            let cached_folder = CachedFolder {
                id,
                name: name_str.clone(),
                delimiter,
                attributes,
                uidvalidity,
                uidnext,
                total_messages,
                unseen_messages,
                last_sync,
            };

            // Add to memory cache
            let mut folder_cache = self.folder_cache.write().await;
            folder_cache.insert(cache_key.clone(), cached_folder.clone());

            Ok(cached_folder)
        } else {
            // Create new folder in database
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO folders (account_id, name, delimiter, attributes) VALUES (?, ?, NULL, '[]') RETURNING id"
            )
            .bind(account_id)
            .bind(name)
            .fetch_one(pool)
            .await?;

            let cached_folder = CachedFolder {
                id,
                name: name.to_string(),
                delimiter: None,
                attributes: Vec::new(),
                uidvalidity: None,
                uidnext: None,
                total_messages: 0,
                unseen_messages: 0,
                last_sync: None,
            };

            // Add to memory cache
            let mut folder_cache = self.folder_cache.write().await;
            folder_cache.insert(cache_key, cached_folder.clone());

            Ok(cached_folder)
        }
    }

    /// Get or create a folder (defaults to account_id=1 for backwards compatibility)
    pub async fn get_or_create_folder(&self, name: &str) -> Result<CachedFolder, CacheError> {
        self.get_or_create_folder_for_account(name, 1).await
    }

    pub async fn cache_email(&self, folder_name: &str, email: &Email) -> Result<(), CacheError> {
        let folder = self.get_or_create_folder(folder_name).await?;
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Extract data from envelope
        let (message_id, subject, from, from_name, to, cc, date) = if let Some(envelope) = &email.envelope {
            let from_addr = envelope.from.first();
            let from_str = from_addr.map(|a| format!("{}@{}",
                a.mailbox.as_deref().unwrap_or(""),
                a.host.as_deref().unwrap_or(""))).unwrap_or_default();
            let from_name_str = from_addr.and_then(|a| a.name.clone());

            let to_vec: Vec<String> = envelope.to.iter()
                .map(|a| format!("{}@{}", a.mailbox.as_deref().unwrap_or(""), a.host.as_deref().unwrap_or("")))
                .collect();
            let cc_vec: Vec<String> = envelope.cc.iter()
                .map(|a| format!("{}@{}", a.mailbox.as_deref().unwrap_or(""), a.host.as_deref().unwrap_or("")))
                .collect();

            // Decode MIME-encoded subject if present
            let decoded_subject = envelope.subject.as_ref()
                .map(|s| crate::utils::decode_mime_header(s));

            (envelope.message_id.clone(), decoded_subject,
             Some(from_str), from_name_str, to_vec, cc_vec, None::<chrono::DateTime<chrono::Utc>>)
        } else {
            (None, None, None, None, Vec::new(), Vec::new(), None)
        };

        // Serialize arrays to JSON
        let to_addresses = serde_json::to_string(&to).unwrap_or_else(|_| "[]".to_string());
        let cc_addresses = serde_json::to_string(&cc).unwrap_or_else(|_| "[]".to_string());
        let flags = serde_json::to_string(&email.flags).unwrap_or_else(|_| "[]".to_string());
        let headers = "{}".to_string(); // Headers not directly available

        // Insert or update email in database
        let email_id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO emails (
                folder_id, uid, message_id, subject, from_address, from_name,
                to_addresses, cc_addresses, date, internal_date, size, flags,
                headers, body_text, body_html
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(folder_id, uid) DO UPDATE SET
                message_id = excluded.message_id,
                subject = excluded.subject,
                from_address = excluded.from_address,
                from_name = excluded.from_name,
                to_addresses = excluded.to_addresses,
                cc_addresses = excluded.cc_addresses,
                date = excluded.date,
                internal_date = excluded.internal_date,
                size = excluded.size,
                flags = excluded.flags,
                headers = excluded.headers,
                body_text = excluded.body_text,
                body_html = excluded.body_html,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id
            "#
        )
        .bind(folder.id)
        .bind(email.uid as i64)
        .bind(&message_id)
        .bind(&subject)
        .bind(&from)
        .bind(&from_name)
        .bind(to_addresses)
        .bind(cc_addresses)
        .bind(date)
        .bind(email.internal_date)
        .bind(email.body.as_ref().map(|b| b.len() as i64))
        .bind(flags)
        .bind(headers)
        .bind(&email.text_body)
        .bind(&email.html_body)
        .fetch_one(pool)
        .await?;

        // Create cached email for memory cache
        let cached_email = CachedEmail {
            id: email_id,
            folder_id: folder.id,
            uid: email.uid,
            message_id: message_id.clone(),
            subject: subject.clone(),
            from_address: from.clone(),
            from_name: from_name.clone(),
            to_addresses: to,
            cc_addresses: cc,
            date,
            internal_date: email.internal_date,
            size: email.body.as_ref().map(|b| b.len() as i64),
            flags: email.flags.clone(),
            body_text: email.text_body.clone(),
            body_html: email.html_body.clone(),
            cached_at: Utc::now(),
        };

        // Add to memory cache
        let cache_key = format!("{}:{}", folder_name, email.uid);
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.put(cache_key, cached_email);

        debug!("Cached email {} in folder {}", email.uid, folder_name);
        Ok(())
    }

    pub async fn get_cached_email(&self, folder_name: &str, uid: u32) -> Result<Option<CachedEmail>, CacheError> {
        // Check memory cache first
        let cache_key = format!("{}:{}", folder_name, uid);
        {
            let mut memory_cache = self.memory_cache.write().await;
            if let Some(email) = memory_cache.get(&cache_key) {
                debug!("Email {} found in memory cache", uid);
                return Ok(Some(email.clone()));
            }
        }

        // Not in memory, check database
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let email = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ? AND uid = ?
            "#
        )
        .bind(folder.id)
        .bind(uid as i64)
        .fetch_optional(pool)
        .await?;

        if let Some((id, folder_id, uid_i64, message_id, subject, from_address, from_name,
                    to_json, cc_json, date, internal_date, size, flags_json,
                    body_text, body_html, cached_at)) = email {

            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            let cached_email = CachedEmail {
                id,
                folder_id,
                uid: uid_i64 as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            };

            // Add to memory cache for future access
            let mut memory_cache = self.memory_cache.write().await;
            memory_cache.put(cache_key, cached_email.clone());

            debug!("Email {} loaded from database cache", uid);
            Ok(Some(cached_email))
        } else {
            Ok(None)
        }
    }

    /// Get cached emails with pagination support for a specific account
    pub async fn get_cached_emails_for_account(&self, folder_name: &str, account_id: i64, limit: usize, offset: usize, preview_mode: bool) -> Result<Vec<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // When in preview mode, truncate body text/html to 200 characters to save tokens
        let query = if preview_mode {
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags,
                   CASE WHEN body_text IS NOT NULL THEN SUBSTR(body_text, 1, 200) || '...' ELSE NULL END as body_text,
                   CASE WHEN body_html IS NOT NULL THEN SUBSTR(body_html, 1, 200) || '...' ELSE NULL END as body_html,
                   cached_at
            FROM emails
            WHERE folder_id = ?
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ? OFFSET ?
            "#
        } else {
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ?
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ? OFFSET ?
            "#
        };

        let emails = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(query)
        .bind(folder.id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        let mut cached_emails = Vec::new();
        for (id, folder_id, uid, message_id, subject, from_address, from_name,
             to_addresses, cc_addresses, date, internal_date, size,
             flags, body_text, body_html, cached_at) in emails {

            let to_addrs: Vec<String> = serde_json::from_str(&to_addresses).unwrap_or_default();
            let cc_addrs: Vec<String> = serde_json::from_str(&cc_addresses).unwrap_or_default();
            let flag_list: Vec<String> = serde_json::from_str(&flags).unwrap_or_default();

            cached_emails.push(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses: to_addrs,
                cc_addresses: cc_addrs,
                date,
                internal_date,
                size,
                flags: flag_list,
                body_text,
                body_html,
                cached_at,
            });
        }

        Ok(cached_emails)
    }

    /// Get cached emails (defaults to account_id=1 for backwards compatibility)
    pub async fn get_cached_emails(&self, folder_name: &str, limit: usize, offset: usize, preview_mode: bool) -> Result<Vec<CachedEmail>, CacheError> {
        self.get_cached_emails_for_account(folder_name, 1, limit, offset, preview_mode).await
    }

    pub async fn get_cached_emails_for_folder(&self, folder_name: &str, limit: usize) -> Result<Vec<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let emails = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ?
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ?
            "#
        )
        .bind(folder.id)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        let mut cached_emails = Vec::new();
        for (id, folder_id, uid, message_id, subject, from_address, from_name,
             to_json, cc_json, date, internal_date, size, flags_json,
             body_text, body_html, cached_at) in emails {

            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            cached_emails.push(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            });
        }

        Ok(cached_emails)
    }

    /// Get folder from cache for a specific account
    async fn get_folder_from_cache_for_account(&self, name: &str, account_id: i64) -> Option<CachedFolder> {
        let cache_key = format!("{}:{}", account_id, name);
        let folder_cache = self.folder_cache.read().await;
        folder_cache.get(&cache_key).cloned()
    }

    /// Get folder from cache (defaults to account_id=1 for backwards compatibility)
    async fn get_folder_from_cache(&self, name: &str) -> Option<CachedFolder> {
        self.get_folder_from_cache_for_account(name, 1).await
    }

    pub async fn update_sync_state(&self, folder_name: &str, last_uid: u32, status: SyncStatus) -> Result<(), CacheError> {
        let folder = self.get_or_create_folder(folder_name).await?;
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let status_str = match status {
            SyncStatus::Idle => "idle",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Error => "error",
        };

        sqlx::query(
            r#"
            INSERT INTO sync_state (folder_id, last_uid_synced, sync_status, last_incremental_sync)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(folder_id) DO UPDATE SET
                last_uid_synced = excluded.last_uid_synced,
                sync_status = excluded.sync_status,
                last_incremental_sync = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(folder.id)
        .bind(last_uid as i64)
        .bind(status_str)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_sync_state(&self, folder_name: &str) -> Result<Option<SyncState>, CacheError> {
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let state = sqlx::query_as::<_, (i64, Option<i64>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, String, Option<String>)>(
            "SELECT folder_id, last_uid_synced, last_full_sync, last_incremental_sync, sync_status, error_message
             FROM sync_state WHERE folder_id = ?"
        )
        .bind(folder.id)
        .fetch_optional(pool)
        .await?;

        if let Some((folder_id, last_uid, last_full, last_inc, status_str, error_msg)) = state {
            let sync_status = match status_str.as_str() {
                "syncing" => SyncStatus::Syncing,
                "error" => SyncStatus::Error,
                _ => SyncStatus::Idle,
            };

            Ok(Some(SyncState {
                folder_id,
                last_uid_synced: last_uid.map(|u| u as u32),
                last_full_sync: last_full,
                last_incremental_sync: last_inc,
                sync_status,
                error_message: error_msg,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn clear_folder_cache(&self, folder_name: &str) -> Result<(), CacheError> {
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Delete all emails in the folder
        sqlx::query("DELETE FROM emails WHERE folder_id = ?")
            .bind(folder.id)
            .execute(pool)
            .await?;

        // Clear memory cache entries for this folder
        let mut memory_cache = self.memory_cache.write().await;
        let keys_to_remove: Vec<String> = memory_cache
            .iter()
            .filter_map(|(k, _)| {
                if k.starts_with(&format!("{}:", folder_name)) {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            memory_cache.pop(&key);
        }

        info!("Cleared cache for folder {}", folder_name);
        Ok(())
    }

    pub async fn get_cache_stats(&self) -> Result<HashMap<String, serde_json::Value>, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let total_emails = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM emails")
            .fetch_one(pool)
            .await?;

        let total_folders = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM folders")
            .fetch_one(pool)
            .await?;

        let cache_size = sqlx::query_scalar::<_, i64>(
            "SELECT SUM(LENGTH(body_text) + LENGTH(body_html) + LENGTH(headers)) FROM emails"
        )
        .fetch_optional(pool)
            .await?
        .unwrap_or(0);

        let memory_cache = self.memory_cache.read().await;
        let memory_cache_size = memory_cache.len();

        let mut stats = HashMap::new();
        stats.insert("total_emails".to_string(), serde_json::json!(total_emails));
        stats.insert("total_folders".to_string(), serde_json::json!(total_folders));
        stats.insert("cache_size_bytes".to_string(), serde_json::json!(cache_size));
        stats.insert("cache_size_mb".to_string(), serde_json::json!(cache_size / (1024 * 1024)));
        stats.insert("memory_cache_items".to_string(), serde_json::json!(memory_cache_size));
        stats.insert("max_memory_items".to_string(), serde_json::json!(self.config.max_memory_items));

        Ok(stats)
    }

    /// Get a specific email by UID
    /// Get an email by UID for a specific account
    pub async fn get_email_by_uid_for_account(&self, folder_name: &str, uid: u32, account_id: i64) -> Result<Option<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let email = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ? AND uid = ?
            "#
        )
        .bind(folder.id)
        .bind(uid as i64)
        .fetch_optional(pool)
        .await?;

        if let Some((id, folder_id, uid, message_id, subject, from_address, from_name,
                     to_json, cc_json, date, internal_date, size, flags_json,
                     body_text, body_html, cached_at)) = email {
            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            Ok(Some(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get an email by UID (defaults to account_id=1 for backwards compatibility)
    pub async fn get_email_by_uid(&self, folder_name: &str, uid: u32) -> Result<Option<CachedEmail>, CacheError> {
        self.get_email_by_uid_for_account(folder_name, uid, 1).await
    }

    // Old implementation kept for reference
    async fn get_email_by_uid_old(&self, folder_name: &str, uid: u32) -> Result<Option<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let email = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ? AND uid = ?
            "#
        )
        .bind(folder.id)
        .bind(uid as i64)
        .fetch_optional(pool)
        .await?;

        if let Some((id, folder_id, uid, message_id, subject, from_address, from_name,
                     to_json, cc_json, date, internal_date, size, flags_json,
                     body_text, body_html, cached_at)) = email {

            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            Ok(Some(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Count emails in a folder for a specific account
    pub async fn count_emails_in_folder_for_account(&self, folder_name: &str, account_id: i64) -> Result<i64, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(0),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE folder_id = ?"
        )
        .bind(folder.id)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// Count emails in a folder (defaults to account_id=1 for backwards compatibility)
    pub async fn count_emails_in_folder(&self, folder_name: &str) -> Result<i64, CacheError> {
        self.count_emails_in_folder_for_account(folder_name, 1).await
    }

    /// Get folder statistics for a specific account
    pub async fn get_folder_stats_for_account(&self, folder_name: &str, account_id: i64) -> Result<serde_json::Map<String, serde_json::Value>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => {
                let mut stats = serde_json::Map::new();
                stats.insert("error".to_string(), serde_json::json!("Folder not found"));
                return Ok(stats);
            }
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Get total count
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM emails WHERE folder_id = ?"
        )
        .bind(folder.id)
        .fetch_one(pool)
        .await?;

        // Get unread count (emails without \Seen flag)
        let unread = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM emails
               WHERE folder_id = ?
               AND flags NOT LIKE '%"Seen"%'"#
        )
        .bind(folder.id)
        .fetch_one(pool)
        .await?;

        // Get total size
        let total_size = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT SUM(size) FROM emails WHERE folder_id = ?"
        )
        .bind(folder.id)
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        let mut stats = serde_json::Map::new();
        stats.insert("folder".to_string(), serde_json::json!(folder_name));
        stats.insert("total".to_string(), serde_json::json!(total));
        stats.insert("unread".to_string(), serde_json::json!(unread));
        stats.insert("read".to_string(), serde_json::json!(total - unread));
        stats.insert("size_bytes".to_string(), serde_json::json!(total_size));
        stats.insert("size_mb".to_string(), serde_json::json!(total_size as f64 / (1024.0 * 1024.0)));

        Ok(stats)
    }

    /// Get folder statistics (defaults to account_id=1 for backwards compatibility)
    pub async fn get_folder_stats(&self, folder_name: &str) -> Result<serde_json::Map<String, serde_json::Value>, CacheError> {
        self.get_folder_stats_for_account(folder_name, 1).await
    }

    /// Search cached emails for a specific account
    pub async fn search_cached_emails_for_account(&self, folder_name: &str, query: &str, limit: usize, account_id: i64) -> Result<Vec<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let search_pattern = format!("%{}%", query);

        let emails = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ?
            AND (subject LIKE ? OR from_address LIKE ? OR from_name LIKE ? OR body_text LIKE ?)
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ?
            "#
        )
        .bind(folder.id)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        let mut cached_emails = Vec::new();
        for (id, folder_id, uid, message_id, subject, from_address, from_name,
             to_json, cc_json, date, internal_date, size, flags_json,
             body_text, body_html, cached_at) in emails {

            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            cached_emails.push(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            });
        }

        Ok(cached_emails)
    }

    /// Search cached emails (defaults to account_id=1 for backwards compatibility)
    pub async fn search_cached_emails(&self, folder_name: &str, query: &str, limit: usize) -> Result<Vec<CachedEmail>, CacheError> {
        self.search_cached_emails_for_account(folder_name, query, limit, 1).await
    }

    // Old implementation kept for reference
    async fn search_cached_emails_old(&self, folder_name: &str, query: &str, limit: usize) -> Result<Vec<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache(folder_name).await {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let search_pattern = format!("%{}%", query);

        let emails = sqlx::query_as::<_, (
            i64, i64, i64, Option<String>, Option<String>, Option<String>, Option<String>,
            String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<i64>,
            String, Option<String>, Option<String>, DateTime<Utc>
        )>(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at
            FROM emails
            WHERE folder_id = ?
            AND (subject LIKE ? OR from_address LIKE ? OR from_name LIKE ? OR body_text LIKE ?)
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ?
            "#
        )
        .bind(folder.id)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        let mut cached_emails = Vec::new();
        for (id, folder_id, uid, message_id, subject, from_address, from_name,
             to_json, cc_json, date, internal_date, size, flags_json,
             body_text, body_html, cached_at) in emails {

            let to_addresses: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<String> = serde_json::from_str(&cc_json).unwrap_or_default();
            let flags: Vec<String> = serde_json::from_str(&flags_json).unwrap_or_default();

            cached_emails.push(CachedEmail {
                id,
                folder_id,
                uid: uid as u32,
                message_id,
                subject,
                from_address,
                from_name,
                to_addresses,
                cc_addresses,
                date,
                internal_date,
                size,
                flags,
                body_text,
                body_html,
                cached_at,
            });
        }

        Ok(cached_emails)
    }
}