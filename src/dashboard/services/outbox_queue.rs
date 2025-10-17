use sqlx::SqlitePool;
use chrono::{DateTime, Utc, NaiveDateTime};
use serde::{Deserialize, Serialize};
use log::{info, warn};

// Helper to convert SQLite NaiveDateTime to DateTime<Utc>
fn naive_to_utc(naive: NaiveDateTime) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(naive, Utc)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxQueueItem {
    pub id: Option<i64>,
    pub account_email: String,
    pub message_id: Option<String>,

    // Email content
    pub to_addresses: Vec<String>,
    pub cc_addresses: Option<Vec<String>>,
    pub bcc_addresses: Option<Vec<String>>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub raw_email_bytes: Vec<u8>,

    // Status tracking
    pub status: OutboxStatus,
    pub smtp_sent: bool,
    pub outbox_saved: bool,
    pub sent_folder_saved: bool,

    // Retry logic
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub smtp_sent_at: Option<DateTime<Utc>>,
    pub last_retry_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutboxStatus {
    Pending,
    Sending,
    Sent,
    Failed,
}

impl OutboxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutboxStatus::Pending => "pending",
            OutboxStatus::Sending => "sending",
            OutboxStatus::Sent => "sent",
            OutboxStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "sending" => OutboxStatus::Sending,
            "sent" => OutboxStatus::Sent,
            "failed" => OutboxStatus::Failed,
            _ => OutboxStatus::Pending,
        }
    }
}

pub struct OutboxQueueService {
    pool: SqlitePool,
}

impl OutboxQueueService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Add a new email to the outbox queue
    pub async fn enqueue(&self, item: OutboxQueueItem) -> Result<i64, sqlx::Error> {
        let to_json = serde_json::to_string(&item.to_addresses).unwrap_or_default();
        let cc_json = item.cc_addresses.as_ref().map(|cc| serde_json::to_string(cc).unwrap_or_default());
        let bcc_json = item.bcc_addresses.as_ref().map(|bcc| serde_json::to_string(bcc).unwrap_or_default());
        let status_str = item.status.as_str().to_string();

        let result = sqlx::query!(
            r#"
            INSERT INTO outbox_queue (
                account_email, message_id, to_addresses, cc_addresses, bcc_addresses,
                subject, body_text, body_html, raw_email_bytes,
                status, smtp_sent, outbox_saved, sent_folder_saved,
                retry_count, max_retries
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            item.account_email,
            item.message_id,
            to_json,
            cc_json,
            bcc_json,
            item.subject,
            item.body_text,
            item.body_html,
            item.raw_email_bytes,
            status_str,
            item.smtp_sent,
            item.outbox_saved,
            item.sent_folder_saved,
            item.retry_count,
            item.max_retries
        )
        .execute(&self.pool)
        .await?;

        info!("Enqueued email for {} (subject: {})", item.account_email, item.subject);
        Ok(result.last_insert_rowid())
    }

    /// Get next pending email to process
    pub async fn get_next_pending(&self) -> Result<Option<OutboxQueueItem>, sqlx::Error> {
        let record = sqlx::query!(
            r#"
            SELECT id, account_email, message_id, to_addresses, cc_addresses, bcc_addresses,
                   subject, body_text, body_html, raw_email_bytes,
                   status, smtp_sent, outbox_saved, sent_folder_saved,
                   retry_count, max_retries, last_error,
                   created_at, smtp_sent_at, last_retry_at, completed_at
            FROM outbox_queue
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(|r| OutboxQueueItem {
            id: r.id,
            account_email: r.account_email,
            message_id: r.message_id,
            to_addresses: serde_json::from_str(&r.to_addresses).unwrap_or_default(),
            cc_addresses: r.cc_addresses.and_then(|cc| serde_json::from_str(&cc).ok()),
            bcc_addresses: r.bcc_addresses.and_then(|bcc| serde_json::from_str(&bcc).ok()),
            subject: r.subject,
            body_text: r.body_text,
            body_html: r.body_html,
            raw_email_bytes: r.raw_email_bytes,
            status: OutboxStatus::from_str(&r.status),
            smtp_sent: r.smtp_sent,
            outbox_saved: r.outbox_saved,
            sent_folder_saved: r.sent_folder_saved,
            retry_count: r.retry_count as i32,
            max_retries: r.max_retries as i32,
            last_error: r.last_error,
            created_at: r.created_at.map(naive_to_utc).unwrap_or_else(Utc::now),
            smtp_sent_at: r.smtp_sent_at.map(naive_to_utc),
            last_retry_at: r.last_retry_at.map(naive_to_utc),
            completed_at: r.completed_at.map(naive_to_utc),
        }))
    }

    /// Update status to sending
    pub async fn mark_sending(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE outbox_queue SET status = 'sending', last_retry_at = CURRENT_TIMESTAMP WHERE id = ?"#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark SMTP as sent successfully
    pub async fn mark_smtp_sent(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE outbox_queue SET smtp_sent = TRUE, smtp_sent_at = CURRENT_TIMESTAMP WHERE id = ?"#,
            id
        )
        .execute(&self.pool)
        .await?;

        info!("Marked queue item {} as SMTP sent", id);
        Ok(())
    }

    /// Mark outbox folder save complete
    pub async fn mark_outbox_saved(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE outbox_queue SET outbox_saved = TRUE WHERE id = ?"#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark sent folder save complete
    pub async fn mark_sent_folder_saved(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE outbox_queue SET sent_folder_saved = TRUE WHERE id = ?"#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark item as fully complete
    pub async fn mark_complete(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE outbox_queue SET status = 'sent', completed_at = CURRENT_TIMESTAMP WHERE id = ?"#,
            id
        )
        .execute(&self.pool)
        .await?;

        info!("Marked queue item {} as complete", id);
        Ok(())
    }

    /// Mark item as failed with error
    pub async fn mark_failed(&self, id: i64, error: String) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE outbox_queue
            SET status = 'failed', last_error = ?, retry_count = retry_count + 1
            WHERE id = ?
            "#,
            error,
            id
        )
        .execute(&self.pool)
        .await?;

        warn!("Marked queue item {} as failed: {}", id, error);
        Ok(())
    }

    /// Retry a failed item if under max retries
    pub async fn retry_if_eligible(&self, id: i64) -> Result<bool, sqlx::Error> {
        let record = sqlx::query!(
            r#"SELECT retry_count, max_retries FROM outbox_queue WHERE id = ?"#,
            id
        )
        .fetch_one(&self.pool)
        .await?;

        if record.retry_count < record.max_retries {
            sqlx::query!(
                r#"UPDATE outbox_queue SET status = 'pending' WHERE id = ?"#,
                id
            )
            .execute(&self.pool)
            .await?;

            info!("Retrying queue item {} (attempt {}/{})", id, record.retry_count + 1, record.max_retries);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all items for an account
    pub async fn get_by_account(&self, account_email: &str) -> Result<Vec<OutboxQueueItem>, sqlx::Error> {
        let records = sqlx::query!(
            r#"
            SELECT id, account_email, message_id, to_addresses, cc_addresses, bcc_addresses,
                   subject, body_text, body_html, raw_email_bytes,
                   status, smtp_sent, outbox_saved, sent_folder_saved,
                   retry_count, max_retries, last_error,
                   created_at, smtp_sent_at, last_retry_at, completed_at
            FROM outbox_queue
            WHERE account_email = ?
            ORDER BY created_at DESC
            "#,
            account_email
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records.into_iter().map(|r| OutboxQueueItem {
            id: r.id,
            account_email: r.account_email,
            message_id: r.message_id,
            to_addresses: serde_json::from_str(&r.to_addresses).unwrap_or_default(),
            cc_addresses: r.cc_addresses.and_then(|cc| serde_json::from_str(&cc).ok()),
            bcc_addresses: r.bcc_addresses.and_then(|bcc| serde_json::from_str(&bcc).ok()),
            subject: r.subject,
            body_text: r.body_text,
            body_html: r.body_html,
            raw_email_bytes: r.raw_email_bytes,
            status: OutboxStatus::from_str(&r.status),
            smtp_sent: r.smtp_sent,
            outbox_saved: r.outbox_saved,
            sent_folder_saved: r.sent_folder_saved,
            retry_count: r.retry_count as i32,
            max_retries: r.max_retries as i32,
            last_error: r.last_error,
            created_at: r.created_at.map(naive_to_utc).unwrap_or_else(Utc::now),
            smtp_sent_at: r.smtp_sent_at.map(naive_to_utc),
            last_retry_at: r.last_retry_at.map(naive_to_utc),
            completed_at: r.completed_at.map(naive_to_utc),
        }).collect())
    }
}
