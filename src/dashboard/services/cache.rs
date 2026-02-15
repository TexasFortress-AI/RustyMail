// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use chrono::{DateTime, Utc};
use lru::LruCache;
use std::num::NonZeroUsize;
use log::{info, error, debug, warn};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use crate::imap::types::{Email, Address};

// Default account email for backwards compatibility wrapper methods
// This should match one of the actual accounts in the database
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
    pub cached_count: i32,
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
    pub has_attachments: bool,
    pub in_reply_to: Option<String>,
    pub references_header: Option<String>,
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
    pub db_pool: Option<SqlitePool>,
    memory_cache: Arc<RwLock<LruCache<String, CachedEmail>>>,
    folder_cache: Arc<RwLock<LruCache<String, CachedFolder>>>,
    config: CacheConfig,
}

impl std::fmt::Debug for CacheService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheService")
            .field("db_pool", &self.db_pool.is_some())
            .field("memory_cache", &"<LruCache>")
            .field("folder_cache", &"<LruCache>")
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub database_url: String,
    pub max_memory_items: usize,
    pub max_folder_items: usize,
    pub max_cache_size_mb: u64,
    pub max_email_age_days: u32,
    pub sync_interval_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite:data/email_cache.db".to_string(),
            max_memory_items: 200,   // Reduced from 1000 to limit memory usage
            max_folder_items: 50,    // Reduced from 100
            max_cache_size_mb: 500,  // Reduced from 1000
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
        let folder_cache = Arc::new(RwLock::new(
            LruCache::new(NonZeroUsize::new(config.max_folder_items).unwrap())
        ));

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
                cached_count: 0,
                last_sync,
            };

            folder_cache.put(name, cached_folder);
        }

        Ok(())
    }


    /// Get or create a folder for a specific account
    pub async fn get_or_create_folder_for_account(&self, name: &str, account_id: &str) -> Result<CachedFolder, CacheError> {
        // Check memory cache first (keyed by account_id:folder_name for multi-account support)
        let cache_key = format!("{}:{}", account_id, name);
        {
            let folder_cache = self.folder_cache.read().await;
            if let Some(folder) = folder_cache.peek(&cache_key) {
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
                cached_count: 0,
                last_sync,
            };

            // Add to memory cache
            let mut folder_cache = self.folder_cache.write().await;
            folder_cache.put(cache_key.clone(), cached_folder.clone());

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
                cached_count: 0,
                last_sync: None,
            };

            // Add to memory cache
            let mut folder_cache = self.folder_cache.write().await;
            folder_cache.put(cache_key, cached_folder.clone());

            Ok(cached_folder)
        }
    }


    pub async fn cache_email(&self, folder_name: &str, email: &Email, account_id: &str) -> Result<(), CacheError> {
        let folder = self.get_or_create_folder_for_account(folder_name, account_id).await?;
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

            // Parse envelope date string to DateTime<Utc>
            let parsed_date = envelope.date.as_ref().and_then(|date_str| {
                // Try common email date formats
                chrono::DateTime::parse_from_rfc2822(date_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
                    .or_else(|| {
                        // Fallback: try RFC3339
                        chrono::DateTime::parse_from_rfc3339(date_str)
                            .map(|dt| dt.with_timezone(&Utc))
                            .ok()
                    })
            });

            (envelope.message_id.clone(), decoded_subject,
             Some(from_str), from_name_str, to_vec, cc_vec, parsed_date)
        } else {
            (None, None, None, None, Vec::new(), Vec::new(), None)
        };

        // Extract thread headers
        let in_reply_to = email.envelope.as_ref().and_then(|e| e.in_reply_to.clone());
        let references_header = email.body.as_ref().and_then(|body| {
            mail_parser::Message::parse(body).and_then(|msg| {
                msg.header_raw("References").map(|v| v.to_string())
            })
        });

        // Serialize arrays to JSON
        let to_addresses = serde_json::to_string(&to).unwrap_or_else(|_| "[]".to_string());
        let cc_addresses = serde_json::to_string(&cc).unwrap_or_else(|_| "[]".to_string());
        let flags = serde_json::to_string(&email.flags).unwrap_or_else(|_| "[]".to_string());
        let headers = "{}".to_string(); // Headers not directly available

        // Determine if email has attachments from MIME structure
        let has_attachments = !email.attachments.is_empty();

        // Insert or update email in database
        let email_id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO emails (
                folder_id, uid, message_id, subject, from_address, from_name,
                to_addresses, cc_addresses, date, internal_date, size, flags,
                headers, body_text, body_html, has_attachments,
                in_reply_to, references_header
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                has_attachments = excluded.has_attachments,
                in_reply_to = excluded.in_reply_to,
                references_header = excluded.references_header,
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
        .bind(has_attachments)
        .bind(&in_reply_to)
        .bind(&references_header)
        .fetch_one(pool)
        .await?;

        // Store attachment metadata if the email has attachments and a message_id
        if !email.attachments.is_empty() {
            if let Some(ref msg_id) = message_id {
                if let Err(e) = super::attachment_storage::store_attachment_metadata_from_mime(
                    pool, account_id, msg_id, &email.attachments,
                ).await {
                    warn!("Failed to store attachment metadata for email {}: {}", email.uid, e);
                }
            }
        }

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
            has_attachments: !email.attachments.is_empty(),
            in_reply_to: in_reply_to.clone(),
            references_header: references_header.clone(),
        };

        // Add to memory cache with account_id to prevent cross-account data leakage
        let cache_key = format!("{}:{}:{}", account_id, folder_name, email.uid);
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.put(cache_key, cached_email);

        debug!("Cached email {} in folder {} for account {}", email.uid, folder_name, account_id);
        Ok(())
    }

    /// Get all cached UIDs for a folder (used for flag resync).
    pub async fn get_cached_uids(&self, folder_name: &str, account_id: &str) -> Result<Vec<u32>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;
        let rows = sqlx::query_scalar::<_, i64>("SELECT uid FROM emails WHERE folder_id = ?")
            .bind(folder.id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|uid| uid as u32).collect())
    }

    /// Update only the flags for an existing cached email (lightweight flag resync).
    pub async fn update_email_flags(&self, folder_name: &str, uid: u32, flags: &[String], account_id: &str) -> Result<(), CacheError> {
        let folder = self.get_or_create_folder_for_account(folder_name, account_id).await?;
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;
        let flags_json = serde_json::to_string(flags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query("UPDATE emails SET flags = ?, updated_at = CURRENT_TIMESTAMP WHERE folder_id = ? AND uid = ?")
            .bind(&flags_json)
            .bind(folder.id)
            .bind(uid as i64)
            .execute(pool)
            .await?;

        // Invalidate memory cache entry
        let cache_key = format!("{}:{}:{}", account_id, folder_name, uid);
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.pop(&cache_key);

        Ok(())
    }

    pub async fn get_cached_email(&self, folder_name: &str, uid: u32, account_id: &str) -> Result<Option<CachedEmail>, CacheError> {
        // Check memory cache first
        let cache_key = format!("{}:{}:{}", account_id, folder_name, uid);
        {
            let mut memory_cache = self.memory_cache.write().await;
            if let Some(email) = memory_cache.get(&cache_key) {
                debug!("Email {} found in memory cache for account {}", uid, account_id);
                return Ok(Some(email.clone()));
            }
        }

        // Not in memory, check database
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let row = sqlx::query(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at, has_attachments,
                   in_reply_to, references_header
            FROM emails
            WHERE folder_id = ? AND uid = ?
            "#
        )
        .bind(folder.id)
        .bind(uid as i64)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let to_json: String = row.get("to_addresses");
            let cc_json: String = row.get("cc_addresses");
            let flags_json: String = row.get("flags");

            let cached_email = CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_json).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_json).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_json).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
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
    pub async fn get_cached_emails_for_account(&self, folder_name: &str, account_id: &str, limit: usize, offset: usize, preview_mode: bool) -> Result<Vec<CachedEmail>, CacheError> {
        // Get folder from cache or database (don't create if it doesn't exist)
        let folder = match self.get_or_create_folder_for_account(folder_name, account_id).await {
            Ok(f) => f,
            Err(_) => return Ok(Vec::new()), // Folder doesn't exist, return empty list
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
                   cached_at, has_attachments, in_reply_to, references_header
            FROM emails
            WHERE folder_id = ?
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ? OFFSET ?
            "#
        } else {
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at, has_attachments,
                   in_reply_to, references_header
            FROM emails
            WHERE folder_id = ?
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ? OFFSET ?
            "#
        };

        let rows = sqlx::query(query)
            .bind(folder.id)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(pool)
            .await?;

        let mut cached_emails = Vec::new();
        for row in rows {
            let id: i64 = row.get("id");
            let folder_id: i64 = row.get("folder_id");
            let uid: i64 = row.get("uid");
            let message_id: Option<String> = row.get("message_id");
            let subject: Option<String> = row.get("subject");
            let from_address: Option<String> = row.get("from_address");
            let from_name: Option<String> = row.get("from_name");
            let to_addresses: String = row.get("to_addresses");
            let cc_addresses: String = row.get("cc_addresses");
            let date: Option<DateTime<Utc>> = row.get("date");
            let internal_date: Option<DateTime<Utc>> = row.get("internal_date");
            let size: Option<i64> = row.get("size");
            let flags: String = row.get("flags");
            let body_text: Option<String> = row.get("body_text");
            let body_html: Option<String> = row.get("body_html");
            let cached_at: DateTime<Utc> = row.get("cached_at");
            let has_attachments_i32: i32 = row.get("has_attachments");

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
                has_attachments: has_attachments_i32 != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            });
        }

        Ok(cached_emails)
    }

    /// Get cached emails filtered by flags for a specific account.
    /// `flags_include`: email must contain ALL of these flags (e.g., ["Seen"])
    /// `flags_exclude`: email must NOT contain ANY of these flags (e.g., ["Seen"] for unread)
    pub async fn get_cached_emails_by_flags(
        &self, folder_name: &str, account_id: &str,
        flags_include: &[String], flags_exclude: &[String],
        limit: usize, offset: usize,
    ) -> Result<Vec<CachedEmail>, CacheError> {
        let folder = match self.get_or_create_folder_for_account(folder_name, account_id).await {
            Ok(f) => f,
            Err(_) => return Ok(Vec::new()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Build dynamic query with flag filters
        // Flags are stored as JSON arrays like: ["Seen","Flagged"]
        // SQL: flags LIKE '%"Seen"%' matches the quoted flag within the JSON
        let mut conditions = vec!["folder_id = ?".to_string()];
        for flag in flags_include {
            conditions.push(format!("flags LIKE '%\"{}\"%'", flag));
        }
        for flag in flags_exclude {
            conditions.push(format!("flags NOT LIKE '%\"{}\"%'", flag));
        }

        let where_clause = conditions.join(" AND ");
        let query_str = format!(
            r#"SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at, has_attachments,
                   in_reply_to, references_header
            FROM emails
            WHERE {}
            ORDER BY COALESCE(date, internal_date) DESC
            LIMIT ? OFFSET ?"#,
            where_clause
        );

        let mut query = sqlx::query(&query_str).bind(folder.id);
        // Bind limit and offset
        let rows = query
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(pool)
            .await?;

        let mut cached_emails = Vec::new();
        for row in rows {
            let to_str: String = row.get("to_addresses");
            let cc_str: String = row.get("cc_addresses");
            let flags_str: String = row.get("flags");

            cached_emails.push(CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_str).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_str).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_str).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            });
        }

        Ok(cached_emails)
    }

    /// Get folder from cache for a specific account
    /// First checks in-memory cache, then falls back to database lookup
    /// Automatically tries "INBOX." prefix if exact match fails (for GoDaddy/hierarchical folder names)
    async fn get_folder_from_cache_for_account(&self, name: &str, account_id: &str) -> Option<CachedFolder> {
        let cache_key = format!("{}:{}", account_id, name);

        // Check in-memory cache first
        {
            let folder_cache = self.folder_cache.read().await;
            if let Some(folder) = folder_cache.peek(&cache_key) {
                return Some(folder.clone());
            }
        }

        // Not in memory cache, check database
        let pool = self.db_pool.as_ref()?;

        // Try exact match first
        let mut folder = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i32, i32, Option<DateTime<Utc>>)>(
            "SELECT id, name, delimiter, attributes, uidvalidity, uidnext, total_messages, unseen_messages, last_sync FROM folders WHERE name = ? AND account_id = ?"
        )
        .bind(name)
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .ok()?;

        // If not found and doesn't start with "INBOX.", try with "INBOX." prefix
        // This handles cases where user asks for "Sent" but folder is "INBOX.Sent"
        if folder.is_none() && !name.starts_with("INBOX.") {
            let prefixed_name = format!("INBOX.{}", name);
            debug!("Folder '{}' not found, trying with prefix: '{}'", name, prefixed_name);

            folder = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i32, i32, Option<DateTime<Utc>>)>(
                "SELECT id, name, delimiter, attributes, uidvalidity, uidnext, total_messages, unseen_messages, last_sync FROM folders WHERE name = ? AND account_id = ?"
            )
            .bind(&prefixed_name)
            .bind(account_id)
            .fetch_optional(pool)
            .await
            .ok()?;
        }

        let (id, name_str, delimiter, attributes_json, uidvalidity, uidnext, total_messages, unseen_messages, last_sync) = folder?;

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
            cached_count: 0,
            last_sync,
        };

        // Add to memory cache for future access
        let mut folder_cache = self.folder_cache.write().await;
        folder_cache.put(cache_key, cached_folder.clone());

        Some(cached_folder)
    }

    /// Get all cached folders for a specific account from the database
    /// Returns folder names with message counts and last sync time
    pub async fn get_all_cached_folders_for_account(&self, account_id: &str) -> Result<Vec<CachedFolder>, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let rows = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, i32, i32, i32, Option<DateTime<Utc>>)>(
            "SELECT f.id, f.name, f.delimiter, f.attributes, f.uidvalidity, f.uidnext, f.total_messages, f.unseen_messages, (SELECT COUNT(*) FROM emails e WHERE e.folder_id = f.id) AS cached_count, f.last_sync FROM folders f WHERE f.account_id = ? ORDER BY f.name"
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;

        let folders: Vec<CachedFolder> = rows.into_iter().map(|(id, name, delimiter, attributes_json, uidvalidity, uidnext, total_messages, unseen_messages, cached_count, last_sync)| {
            let attributes: Vec<String> = attributes_json
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            CachedFolder { id, name, delimiter, attributes, uidvalidity, uidnext, total_messages, unseen_messages, cached_count, last_sync }
        }).collect();

        Ok(folders)
    }

    pub async fn update_sync_state(&self, folder_name: &str, last_uid: u32, status: SyncStatus, account_id: &str) -> Result<(), CacheError> {
        let folder = self.get_or_create_folder_for_account(folder_name, account_id).await?;
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

    pub async fn get_sync_state(&self, folder_name: &str, account_id: &str) -> Result<Option<SyncState>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
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

    /// Delete specific emails from cache by UIDs
    pub async fn delete_emails_by_uids(&self, folder_name: &str, uids: &[u32], account_id: &str) -> Result<(), CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(()),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Delete emails from database
        for uid in uids {
            sqlx::query("DELETE FROM emails WHERE folder_id = ? AND uid = ?")
                .bind(folder.id)
                .bind(*uid as i64)
                .execute(pool)
                .await?;

            // Remove from memory cache
            let cache_key = format!("{}:{}:{}", account_id, folder_name, uid);
            let mut memory_cache = self.memory_cache.write().await;
            memory_cache.pop(&cache_key);
        }

        info!("Deleted {} email(s) from cache for folder {}", uids.len(), folder_name);
        Ok(())
    }

    pub async fn clear_folder_cache(&self, folder_name: &str, account_id: &str) -> Result<(), CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
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
    pub async fn get_email_by_uid_for_account(&self, folder_name: &str, uid: u32, account_id: &str) -> Result<Option<CachedEmail>, CacheError> {
        let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
            Some(f) => f,
            None => return Ok(None),
        };

        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        let row = sqlx::query(
            r#"
            SELECT id, folder_id, uid, message_id, subject, from_address, from_name,
                   to_addresses, cc_addresses, date, internal_date, size,
                   flags, body_text, body_html, cached_at, has_attachments,
                   in_reply_to, references_header
            FROM emails
            WHERE folder_id = ? AND uid = ?
            "#
        )
        .bind(folder.id)
        .bind(uid as i64)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let to_json: String = row.get("to_addresses");
            let cc_json: String = row.get("cc_addresses");
            let flags_json: String = row.get("flags");

            Ok(Some(CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_json).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_json).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_json).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            }))
        } else {
            Ok(None)
        }
    }



    /// Count emails in a folder for a specific account
    pub async fn count_emails_in_folder_for_account(&self, folder_name: &str, account_id: &str) -> Result<i64, CacheError> {
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


    /// Get folder statistics for a specific account
    pub async fn get_folder_stats_for_account(&self, folder_name: &str, account_id: &str) -> Result<serde_json::Map<String, serde_json::Value>, CacheError> {
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


    /// Search cached emails for a specific account
    pub async fn search_cached_emails_for_account(&self, folder_name: &str, query: &str, limit: usize, account_id: &str) -> Result<Vec<CachedEmail>, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;
        let search_pattern = format!("%{}%", query);

        let mut qb = sqlx::QueryBuilder::new(
            r#"
            SELECT DISTINCT e.id, e.folder_id, e.uid, e.message_id, e.subject, e.from_address, e.from_name,
                   e.to_addresses, e.cc_addresses, e.date, e.internal_date, e.size,
                   e.flags, e.body_text, e.body_html, e.cached_at, e.has_attachments,
                   e.in_reply_to, e.references_header
            FROM emails e
            LEFT JOIN attachment_metadata a ON e.message_id = a.message_id AND a.account_email =
            "#
        );
        qb.push_bind(account_id);
        qb.push(r#" WHERE (e.subject LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#" OR e.from_address LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#" OR e.from_name LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#" OR e.body_text LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#" OR e.body_html LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#" OR a.filename LIKE "#);
        qb.push_bind(&search_pattern);
        qb.push(r#") "#);

        if !folder_name.is_empty() {
            let folder = match self.get_folder_from_cache_for_account(folder_name, account_id).await {
                Some(f) => f,
                None => return Ok(Vec::new()),
            };
            qb.push(" AND e.folder_id = ");
            qb.push_bind(folder.id);
        }

        qb.push(r#" ORDER BY COALESCE(e.date, e.internal_date) DESC LIMIT "#);
        qb.push_bind(limit as i64);

        let rows = qb.build().fetch_all(pool).await?;

        let mut cached_emails = Vec::new();
        for row in rows {
            let to_addresses_str: String = row.get("to_addresses");
            let cc_addresses_str: String = row.get("cc_addresses");
            let flags_str: String = row.get("flags");

            let cached_email = CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_addresses_str).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_addresses_str).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_str).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            };
            cached_emails.push(cached_email);
        }

        Ok(cached_emails)
    }

    /// Get all emails in the same thread as the given message_id
    pub async fn get_thread_emails(&self, message_id: &str, account_id: &str) -> Result<Vec<CachedEmail>, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Step 1: Look up the seed email to get its references chain
        let seed_row = sqlx::query(
            "SELECT in_reply_to, references_header FROM emails e
             JOIN folders f ON e.folder_id = f.id
             WHERE e.message_id = ? AND f.account_id = ?"
        )
        .bind(message_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        // Step 2: Build the full set of message_ids in this thread
        let mut thread_ids: Vec<String> = vec![message_id.to_string()];
        if let Some(row) = &seed_row {
            if let Some(irt) = row.get::<Option<String>, _>("in_reply_to") {
                if !irt.is_empty() {
                    thread_ids.push(irt);
                }
            }
            if let Some(refs) = row.get::<Option<String>, _>("references_header") {
                for r in refs.split_whitespace() {
                    let r = r.trim_matches(|c| c == '<' || c == '>');
                    if !r.is_empty() && !thread_ids.contains(&r.to_string()) {
                        thread_ids.push(r.to_string());
                    }
                }
            }
        }

        // Step 3: Find all emails where message_id or in_reply_to is in thread_ids
        let placeholders: Vec<&str> = thread_ids.iter().map(|_| "?").collect();
        let ph_str = placeholders.join(", ");
        let sql = format!(
            "SELECT e.id, e.folder_id, e.uid, e.message_id, e.subject, e.from_address, e.from_name,
                    e.to_addresses, e.cc_addresses, e.date, e.internal_date, e.size,
                    e.flags, e.body_text, e.body_html, e.cached_at, e.has_attachments,
                    e.in_reply_to, e.references_header
             FROM emails e
             JOIN folders f ON e.folder_id = f.id
             WHERE f.account_id = ? AND (e.message_id IN ({ph}) OR e.in_reply_to IN ({ph}))
             ORDER BY COALESCE(e.date, e.internal_date) ASC",
            ph = ph_str
        );

        let mut query = sqlx::query(&sql).bind(account_id);
        // Bind thread_ids twice (once for message_id IN, once for in_reply_to IN)
        for id in &thread_ids {
            query = query.bind(id);
        }
        for id in &thread_ids {
            query = query.bind(id);
        }

        let rows = query.fetch_all(pool).await?;
        let mut emails = Vec::new();
        for row in rows {
            let to_str: String = row.get("to_addresses");
            let cc_str: String = row.get("cc_addresses");
            let flags_str: String = row.get("flags");
            emails.push(CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_str).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_str).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_str).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            });
        }
        Ok(emails)
    }

    /// Search cached emails by sender/recipient domain
    pub async fn search_by_domain(&self, domain: &str, search_in: &[&str], account_id: &str, limit: usize) -> Result<Vec<CachedEmail>, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;
        let domain_pattern = format!("%@{}%", domain.to_lowercase());

        let mut conditions = Vec::new();
        for field in search_in {
            match *field {
                "from" => conditions.push("LOWER(e.from_address) LIKE ?".to_string()),
                "to" => conditions.push("e.to_addresses LIKE ?".to_string()),
                "cc" => conditions.push("e.cc_addresses LIKE ?".to_string()),
                _ => {}
            }
        }
        if conditions.is_empty() {
            conditions.push("LOWER(e.from_address) LIKE ?".to_string());
        }

        let where_clause = conditions.join(" OR ");
        let sql = format!(
            "SELECT e.id, e.folder_id, e.uid, e.message_id, e.subject, e.from_address, e.from_name,
                    e.to_addresses, e.cc_addresses, e.date, e.internal_date, e.size,
                    e.flags, e.body_text, e.body_html, e.cached_at, e.has_attachments,
                    e.in_reply_to, e.references_header
             FROM emails e
             JOIN folders f ON e.folder_id = f.id
             WHERE f.account_id = ? AND ({})
             ORDER BY COALESCE(e.date, e.internal_date) DESC
             LIMIT ?",
            where_clause
        );

        let mut query = sqlx::query(&sql).bind(account_id);
        for _ in &conditions {
            query = query.bind(&domain_pattern);
        }
        query = query.bind(limit as i64);

        let rows = query.fetch_all(pool).await?;
        let mut emails = Vec::new();
        for row in rows {
            let to_str: String = row.get("to_addresses");
            let cc_str: String = row.get("cc_addresses");
            let flags_str: String = row.get("flags");
            emails.push(CachedEmail {
                id: row.get("id"),
                folder_id: row.get("folder_id"),
                uid: row.get::<i64, _>("uid") as u32,
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                from_address: row.get("from_address"),
                from_name: row.get("from_name"),
                to_addresses: serde_json::from_str(&to_str).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_str).unwrap_or_default(),
                date: row.get("date"),
                internal_date: row.get("internal_date"),
                size: row.get("size"),
                flags: serde_json::from_str(&flags_str).unwrap_or_default(),
                body_text: row.get("body_text"),
                body_html: row.get("body_html"),
                cached_at: row.get("cached_at"),
                has_attachments: row.get::<i32, _>("has_attachments") != 0,
                in_reply_to: row.get("in_reply_to"),
                references_header: row.get("references_header"),
            });
        }
        Ok(emails)
    }

    /// Get aggregated address/domain report for an account
    pub async fn get_address_report(&self, account_id: &str) -> Result<serde_json::Value, CacheError> {
        let pool = self.db_pool.as_ref().ok_or(CacheError::NotInitialized)?;

        // Get unique sender addresses with counts
        let sender_rows = sqlx::query(
            "SELECT LOWER(e.from_address) as addr, COUNT(*) as cnt
             FROM emails e JOIN folders f ON e.folder_id = f.id
             WHERE f.account_id = ? AND e.from_address IS NOT NULL AND e.from_address != ''
             GROUP BY LOWER(e.from_address)
             ORDER BY cnt DESC"
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;

        let mut addresses: Vec<serde_json::Value> = Vec::new();
        let mut domains: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

        for row in &sender_rows {
            let addr: String = row.get("addr");
            let count: i64 = row.get("cnt");
            addresses.push(serde_json::json!({"address": addr, "count": count}));

            if let Some(domain) = addr.split('@').nth(1) {
                *domains.entry(domain.to_string()).or_insert(0) += count;
            }
        }

        let mut domain_list: Vec<serde_json::Value> = domains.into_iter()
            .map(|(d, c)| serde_json::json!({"domain": d, "count": c}))
            .collect();
        domain_list.sort_by(|a, b| b["count"].as_i64().cmp(&a["count"].as_i64()));

        Ok(serde_json::json!({
            "unique_addresses": addresses.len(),
            "unique_domains": domain_list.len(),
            "top_addresses": addresses.iter().take(50).collect::<Vec<_>>(),
            "top_domains": domain_list.iter().take(30).collect::<Vec<_>>(),
        }))
    }

}