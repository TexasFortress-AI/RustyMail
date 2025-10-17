use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::Mutex as TokioMutex;
use log::{info, error, warn};
use crate::dashboard::services::{OutboxQueueService, SmtpService, AccountService};
use crate::prelude::CloneableImapSessionFactory;

/// Background worker that processes the outbox queue
pub struct OutboxWorker {
    queue_service: Arc<OutboxQueueService>,
    smtp_service: Arc<SmtpService>,
    imap_factory: CloneableImapSessionFactory,
    account_service: Arc<TokioMutex<AccountService>>,
    poll_interval: Duration,
}

// SAFETY: All fields are Send: Arc<T> is Send if T is Send+Sync, CloneableImapSessionFactory is Send+Sync, Duration is Send
unsafe impl Send for OutboxWorker {}

// SAFETY: All fields are Sync: Arc<T> is Sync, CloneableImapSessionFactory is Sync, Duration is Sync
unsafe impl Sync for OutboxWorker {}

impl OutboxWorker {
    pub fn new(
        queue_service: Arc<OutboxQueueService>,
        smtp_service: Arc<SmtpService>,
        imap_factory: CloneableImapSessionFactory,
        account_service: Arc<TokioMutex<AccountService>>,
    ) -> Self {
        let poll_interval = std::env::var("OUTBOX_WORKER_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        Self {
            queue_service,
            smtp_service,
            imap_factory,
            account_service,
            poll_interval: Duration::from_secs(poll_interval),
        }
    }

    /// Start the background worker loop
    pub async fn start(self: Arc<Self>) {
        info!("Starting outbox worker with {} second poll interval", self.poll_interval.as_secs());

        loop {
            if let Err(e) = self.process_next().await {
                error!("Error processing outbox queue: {}", e);
            }

            sleep(self.poll_interval).await;
        }
    }

    /// Process next item in the queue
    async fn process_next(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get next pending item
        let item = match self.queue_service.get_next_pending().await? {
            Some(item) => item,
            None => return Ok(()), // No pending items
        };

        let id = item.id.ok_or("Queue item missing ID")?;

        info!("Processing outbox queue item {} for account {}", id, item.account_email);

        // Mark as sending
        if let Err(e) = self.queue_service.mark_sending(id).await {
            error!("Failed to mark item {} as sending: {}", id, e);
            return Ok(());
        }

        // Step 1: Save to IMAP Outbox folder FIRST (so user can see it in their email client)
        if !item.outbox_saved {
            match self.save_to_folder(&item, "INBOX.Outbox").await {
                Ok(_) => {
                    info!("Email saved to Outbox folder - user can now see it in their email client");
                    if let Err(e) = self.queue_service.mark_outbox_saved(id).await {
                        warn!("Failed to mark outbox saved for item {}: {}", id, e);
                    }
                }
                Err(e) => {
                    // If we can't save to Outbox, just log and continue
                    // The email will still be sent via SMTP
                    warn!("Failed to save to Outbox folder for item {}: {}. Will attempt SMTP send anyway.", id, e);
                }
            }
        }

        // Step 2: Send via SMTP
        if !item.smtp_sent {
            match self.send_via_smtp(&item).await {
                Ok(_) => {
                    info!("Email sent successfully via SMTP");
                    if let Err(e) = self.queue_service.mark_smtp_sent(id).await {
                        error!("Failed to mark SMTP sent for item {}: {}", id, e);
                    }
                }
                Err(e) => {
                    error!("SMTP send failed for item {}: {}", id, e);
                    self.handle_failure(id, format!("SMTP send failed: {}", e)).await;
                    return Ok(());
                }
            }
        }

        // Step 3: Save to Sent folder (and ideally remove from Outbox)
        if !item.sent_folder_saved {
            match self.save_to_folder(&item, "INBOX.Sent").await {
                Ok(_) => {
                    info!("Email saved to Sent folder");
                    if let Err(e) = self.queue_service.mark_sent_folder_saved(id).await {
                        warn!("Failed to mark sent folder saved for item {}: {}", id, e);
                    }
                    // TODO: Remove from Outbox folder after successful send
                }
                Err(e) => {
                    // Don't fail the whole operation, just log
                    warn!("Failed to save to Sent folder for item {}: {}. Email was sent successfully.", id, e);
                }
            }
        }

        // Mark as complete
        if let Err(e) = self.queue_service.mark_complete(id).await {
            error!("Failed to mark item {} as complete: {}", id, e);
        } else {
            info!("Successfully processed outbox queue item {}", id);
        }

        Ok(())
    }

    /// Send email via SMTP
    async fn send_via_smtp(&self, item: &crate::dashboard::services::OutboxQueueItem) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Rebuild the send request from the queue item
        let request = crate::dashboard::services::SendEmailRequest {
            to: item.to_addresses.clone(),
            cc: item.cc_addresses.clone(),
            bcc: item.bcc_addresses.clone(),
            subject: item.subject.clone(),
            body: item.body_text.clone(),
            body_html: item.body_html.clone(),
        };

        // Send using SMTP-only method (no IMAP operations)
        // The worker handles IMAP saves separately
        self.smtp_service.send_email_smtp_only(&item.account_email, request).await?;

        Ok(())
    }

    /// Save email to IMAP folder (Outbox or Sent)
    async fn save_to_folder(&self, item: &crate::dashboard::services::OutboxQueueItem, folder: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get the account for this email
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(&item.account_email)
            .await
            .map_err(|e| format!("Failed to get account {}: {}", item.account_email, e))?;
        drop(account_service);

        // Create IMAP session for this specific account
        let mut session = self.imap_factory.create_session_for_account(&account).await?;

        // Select folder
        session.select_folder(folder).await?;

        // APPEND email with \Seen flag
        let flags = vec!["\\Seen".to_string()];
        session.append(folder, &item.raw_email_bytes, &flags).await?;

        info!("Saved email to {} folder for account {}", folder, item.account_email);
        Ok(())
    }

    /// Handle failure with retry logic
    async fn handle_failure(&self, id: i64, error: String) {
        // Check if we should retry
        match self.queue_service.retry_if_eligible(id).await {
            Ok(true) => {
                info!("Queue item {} will be retried", id);
            }
            Ok(false) => {
                warn!("Queue item {} has exhausted retries, marking as failed", id);
                if let Err(e) = self.queue_service.mark_failed(id, error).await {
                    error!("Failed to mark item {} as failed: {}", id, e);
                }
            }
            Err(e) => {
                error!("Error checking retry eligibility for item {}: {}", id, e);
            }
        }
    }
}
