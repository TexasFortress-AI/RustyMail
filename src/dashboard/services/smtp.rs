use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use lettre::message::header;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::timeout;

use super::account::{AccountService};
use crate::prelude::CloneableImapSessionFactory;

// Folder name constants (can be configured via environment or config file in the future)
const OUTBOX_FOLDER: &str = "INBOX.Outbox";
const SENT_FOLDER: &str = "INBOX.Sent";

#[derive(Error, Debug)]
pub enum SmtpError {
    #[error("SMTP configuration error: {0}")]
    ConfigError(String),

    #[error("Email building error: {0}")]
    BuildError(#[from] lettre::error::Error),

    #[error("Email sending error: {0}")]
    SendError(#[from] lettre::transport::smtp::Error),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("SMTP credentials not configured for account: {0}")]
    MissingCredentials(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub body: String,
    pub body_html: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendEmailResponse {
    pub success: bool,
    pub message_id: Option<String>,
    pub message: String,
}

pub struct SmtpService {
    account_service: Arc<TokioMutex<AccountService>>,
    imap_session_factory: CloneableImapSessionFactory,
}

impl SmtpService {
    pub fn new(
        account_service: Arc<TokioMutex<AccountService>>,
        imap_session_factory: CloneableImapSessionFactory,
    ) -> Self {
        Self {
            account_service,
            imap_session_factory,
        }
    }

    pub async fn send_email(
        &self,
        account_email: &str,
        request: SendEmailRequest,
    ) -> Result<SendEmailResponse, SmtpError> {
        // Get account details
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(account_email)
            .await
            .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;

        // Validate SMTP configuration
        let smtp_host = account
            .smtp_host
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;
        let smtp_user = account
            .smtp_user
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;
        let smtp_pass = account
            .smtp_pass
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;

        let smtp_port = account.smtp_port.unwrap_or(587) as u16;
        let use_starttls = account.smtp_use_starttls.unwrap_or(true);

        // Build from address with properly quoted display name
        let from_mailbox: Mailbox = if account.display_name.is_empty() {
            // Just use the email address if no display name
            account.email_address
                .parse()
                .map_err(|e| SmtpError::ConfigError(format!("Invalid from address: {}", e)))?
        } else {
            // Quote the display name if it contains special characters
            let quoted_name = if account.display_name.contains(|c: char| "()<>[]:;@\\,\"".contains(c)) {
                format!("\"{}\"", account.display_name.replace('\"', "\\\""))
            } else {
                account.display_name.clone()
            };
            format!("{} <{}>", quoted_name, account.email_address)
                .parse()
                .map_err(|e| SmtpError::ConfigError(format!("Invalid from address: {}", e)))?
        };

        // Build email message
        let mut email_builder = Message::builder()
            .from(from_mailbox)
            .subject(&request.subject);

        // Add To recipients
        for to_addr in &request.to {
            email_builder = email_builder.to(to_addr.parse().map_err(|e| {
                SmtpError::ConfigError(format!("Invalid to address {}: {}", to_addr, e))
            })?);
        }

        // Add CC recipients
        if let Some(cc_addrs) = &request.cc {
            for cc_addr in cc_addrs {
                email_builder = email_builder.cc(cc_addr.parse().map_err(|e| {
                    SmtpError::ConfigError(format!("Invalid cc address {}: {}", cc_addr, e))
                })?);
            }
        }

        // Add BCC recipients
        if let Some(bcc_addrs) = &request.bcc {
            for bcc_addr in bcc_addrs {
                email_builder = email_builder.bcc(bcc_addr.parse().map_err(|e| {
                    SmtpError::ConfigError(format!("Invalid bcc address {}: {}", bcc_addr, e))
                })?);
            }
        }

        // Build multipart body (plain text + optional HTML)
        let email = if let Some(html_body) = &request.body_html {
            email_builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(request.body.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(html_body.clone()),
                    ),
            )?
        } else {
            email_builder.header(ContentType::TEXT_PLAIN).body(request.body.clone())?
        };

        // Get message ID before sending
        let message_id = email
            .headers()
            .get_raw("Message-ID")
            .map(|v| v.to_string());

        // Build SMTP transport
        let creds = Credentials::new(smtp_user.clone(), smtp_pass.clone());

        let mailer = if use_starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .build()
        };

        // Convert email to RFC822 format for IMAP operations
        let email_bytes = email.formatted();

        // Step 1: Save to Outbox FIRST (blocking, must succeed - proper Outbox pattern)
        log::info!("Saving email to {} before sending (this may take up to 30 seconds)", OUTBOX_FOLDER);
        match self.ensure_folder_exists_and_append(&account.email_address, &email_bytes, OUTBOX_FOLDER).await {
            Ok(_) => {
                log::info!("Successfully saved email to {}", OUTBOX_FOLDER);
            }
            Err(e) => {
                // CRITICAL: If we can't save to Outbox, abort the send completely
                log::error!("CRITICAL: Failed to save email to {}. Aborting send. Error: {}", OUTBOX_FOLDER, e);
                return Err(e);
            }
        }

        // Step 2: Now try to send via SMTP (Outbox has the email, so user can see it)
        log::info!("Attempting to send email via SMTP...");
        match mailer.send(email.clone()).await {
            Ok(_) => {
                log::info!("Email sent successfully via SMTP");

                // Step 3: Save to Sent folder (best-effort, don't fail if it doesn't work)
                match self.ensure_folder_exists_and_append(&account.email_address, &email_bytes, SENT_FOLDER).await {
                    Ok(_) => {
                        log::info!("Successfully saved sent email to Sent folder");
                    }
                    Err(e) => {
                        log::warn!("Failed to save sent email to Sent folder: {}. Email was sent successfully.", e);
                    }
                }

                // Step 4: Clean up Outbox after successful send
                match self.delete_from_outbox(&account.email_address, &message_id).await {
                    Ok(_) => {
                        log::info!("Successfully removed email from Outbox");
                    }
                    Err(e) => {
                        log::warn!("Failed to remove email from Outbox: {}. Email was sent successfully.", e);
                    }
                }

                Ok(SendEmailResponse {
                    success: true,
                    message_id,
                    message: "Email sent successfully and moved to Sent folder".to_string(),
                })
            }
            Err(e) => {
                log::error!("SMTP send failed: {}. Email remains in Outbox - please check Outbox folder to retry.", e);
                Err(SmtpError::SendError(e))
            }
        }
    }

    /// Append email to folder (expects folder to exist, creates as fallback)
    ///
    /// NOTE: Folders should be pre-created during account validation (see account.rs:ensure_essential_folders_exist)
    /// This function only creates folders as a fallback for edge cases or legacy accounts.
    async fn ensure_folder_exists_and_append(
        &self,
        account_email: &str,
        email_bytes: &[u8],
        folder_name: &str,
    ) -> Result<(), SmtpError> {
        // Hard timeout for the entire operation (session creation + append)
        // This prevents indefinite hangs with slow IMAP servers
        let operation_timeout = Duration::from_secs(40); // 5 seconds more than IMAP APPEND timeout

        log::info!("Starting IMAP APPEND to '{}' with {}s timeout", folder_name, operation_timeout.as_secs());

        // Wrap the entire operation in a timeout
        let result = timeout(operation_timeout, async {
            // Get account to create IMAP session
            let account_service = self.account_service.lock().await;
            let account = account_service
                .get_account(account_email)
                .await
                .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;
            drop(account_service);

            // Create IMAP session for this account
            log::info!("Creating IMAP session for APPEND operation...");
            let session = self.imap_session_factory
                .create_session_for_account(&account)
                .await
                .map_err(|e| SmtpError::ConfigError(format!("Failed to create IMAP session: {}", e)))?;

            // Try to append to the folder (folder should already exist from account validation)
            log::info!("Attempting IMAP APPEND to folder '{}'", folder_name);
            let flags: Vec<String> = vec![];
            match session.append(folder_name, email_bytes, &flags).await {
                Ok(_) => {
                    log::info!("Successfully saved email to folder: {}", folder_name);
                    Ok(())
                }
                Err(append_err) => {
                    let err_str = append_err.to_string().to_lowercase();

                    // Check if error is due to folder not existing (common IMAP error patterns)
                    let folder_not_found = err_str.contains("no such") ||
                                          err_str.contains("not found") ||
                                          err_str.contains("nonexistent") ||
                                          err_str.contains("does not exist");

                    if folder_not_found {
                        log::warn!("Folder '{}' does not exist (should have been pre-created). Attempting to create...", folder_name);

                        // Try to create the folder as fallback
                        match session.create_folder(folder_name).await {
                            Ok(_) => {
                                log::info!("Successfully created missing folder: {}", folder_name);

                                // Now try to append again
                                match session.append(folder_name, email_bytes, &flags).await {
                                    Ok(_) => {
                                        log::info!("Successfully saved email to newly created folder: {}", folder_name);
                                        Ok(())
                                    }
                                    Err(e) => {
                                        Err(SmtpError::ConfigError(format!(
                                            "IMAP APPEND failed after creating folder '{}': {}. Server may be rejecting the operation.",
                                            folder_name, e
                                        )))
                                    }
                                }
                            }
                            Err(create_err) => {
                                Err(SmtpError::ConfigError(format!(
                                    "Folder '{}' missing and could not be created: {}. Check account folder permissions.",
                                    folder_name, create_err
                                )))
                            }
                        }
                    } else {
                        // APPEND failed for a reason other than folder not existing (timeout, permissions, etc)
                        Err(SmtpError::ConfigError(format!(
                            "IMAP APPEND to existing folder '{}' failed: {}. This may be due to slow server processing or permissions.",
                            folder_name, append_err
                        )))
                    }
                }
            }
        }).await;

        // Handle timeout result
        match result {
            Ok(Ok(())) => {
                log::info!("IMAP APPEND operation completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                log::error!("IMAP APPEND operation failed: {}", e);
                Err(e)
            }
            Err(_elapsed) => {
                log::error!("IMAP APPEND operation timed out after {}s. Abandoning session to prevent indefinite hang.", operation_timeout.as_secs());
                Err(SmtpError::ConfigError(format!(
                    "IMAP APPEND to folder '{}' timed out after {}s. Server may be performing security scanning. The operation has been cancelled.",
                    folder_name, operation_timeout.as_secs()
                )))
            }
        }
    }

    /// Append an email to the IMAP Outbox folder before sending
    async fn append_to_outbox(
        &self,
        account_email: &str,
        email_bytes: &[u8],
    ) -> Result<(), SmtpError> {
        // Get account to create IMAP session
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(account_email)
            .await
            .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;
        drop(account_service);

        // Create IMAP session for this account
        let session = self.imap_session_factory
            .create_session_for_account(&account)
            .await
            .map_err(|e| SmtpError::ConfigError(format!("Failed to create IMAP session: {}", e)))?;

        // Common Outbox folder names to try
        let outbox_folders = vec!["INBOX.Outbox", "Outbox", "Drafts", "INBOX.Drafts"];

        let mut last_error = None;

        // Try each possible Outbox folder name
        for folder in &outbox_folders {
            // No flags for outbox messages (not read, not flagged)
            let flags: Vec<String> = vec![];
            match session.append(folder, email_bytes, &flags).await {
                Ok(_) => {
                    log::info!("Successfully saved email to Outbox folder: {}", folder);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!("Failed to append to Outbox folder '{}': {}", folder, e);
                    last_error = Some(e);
                }
            }
        }

        // If all folders failed, return the last error
        if let Some(err) = last_error {
            Err(SmtpError::ConfigError(format!(
                "Could not save to Outbox folder. Tried: {}. Last error: {}",
                outbox_folders.join(", "),
                err
            )))
        } else {
            Err(SmtpError::ConfigError("No Outbox folders to try".to_string()))
        }
    }

    /// Delete an email from the IMAP Outbox folder after successful send
    async fn delete_from_outbox(
        &self,
        account_email: &str,
        message_id: &Option<String>,
    ) -> Result<(), SmtpError> {
        // If there's no message ID, we can't search for the message
        let msg_id = match message_id {
            Some(id) => id,
            None => {
                log::warn!("No Message-ID available to delete from Outbox");
                return Ok(()); // Not an error - just skip cleanup
            }
        };

        // Get account to create IMAP session
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(account_email)
            .await
            .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;
        drop(account_service);

        // Create IMAP session for this account
        let session = self.imap_session_factory
            .create_session_for_account(&account)
            .await
            .map_err(|e| SmtpError::ConfigError(format!("Failed to create IMAP session: {}", e)))?;

        // Common Outbox folder names to try
        let outbox_folders = vec!["INBOX.Outbox", "Outbox", "Drafts", "INBOX.Drafts"];

        let mut last_error = None;

        // Try to find and delete from each possible Outbox folder
        for folder in &outbox_folders {
            // Select the folder
            match session.select_folder(folder).await {
                Ok(_) => {
                    // Search for the message by Message-ID
                    let search_criteria = format!("HEADER Message-ID {}", msg_id);
                    match session.search_emails(&search_criteria).await {
                        Ok(uids) => {
                            if !uids.is_empty() {
                                // Delete all matching messages (should be only one)
                                match session.delete_messages(&uids).await {
                                    Ok(_) => {
                                        log::info!("Successfully deleted {} message(s) from Outbox folder: {}", uids.len(), folder);
                                        return Ok(());
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to delete message from Outbox folder '{}': {}", folder, e);
                                        last_error = Some(e);
                                    }
                                }
                            } else {
                                log::debug!("No messages found in Outbox folder '{}' with Message-ID: {}", folder, msg_id);
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to search Outbox folder '{}': {}", folder, e);
                            last_error = Some(e);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Failed to select Outbox folder '{}': {}", folder, e);
                    last_error = Some(e);
                }
            }
        }

        // If we couldn't delete from any folder, log warning but don't fail
        // (The message might have already been cleaned up or the folder might not exist)
        if let Some(err) = last_error {
            log::warn!(
                "Could not delete from Outbox. Tried: {}. Last error: {}",
                outbox_folders.join(", "),
                err
            );
        }

        // Don't return error - cleanup is best-effort
        Ok(())
    }

    /// Append a sent message to the IMAP Sent folder
    async fn append_to_sent_folder(
        &self,
        account_email: &str,
        email_bytes: &[u8],
    ) -> Result<(), SmtpError> {
        // Get account to create IMAP session
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(account_email)
            .await
            .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;
        drop(account_service);

        // Create IMAP session for this account
        let session = self.imap_session_factory
            .create_session_for_account(&account)
            .await
            .map_err(|e| SmtpError::ConfigError(format!("Failed to create IMAP session: {}", e)))?;

        // Common Sent folder names to try
        let sent_folders = vec!["INBOX.Sent", "Sent", "Sent Items", "[Gmail]/Sent Mail"];

        let mut last_error = None;

        // Try each possible Sent folder name
        for folder in &sent_folders {
            // No flags for sent messages
            let flags: Vec<String> = vec![];
            match session.append(folder, &email_bytes, &flags).await {
                Ok(_) => {
                    log::info!("Successfully saved sent email to folder: {}", folder);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!("Failed to append to folder '{}': {}", folder, e);
                    last_error = Some(e);
                }
            }
        }

        // If all folders failed, return the last error
        if let Some(err) = last_error {
            Err(SmtpError::ConfigError(format!(
                "Could not save to Sent folder. Tried: {}. Last error: {}",
                sent_folders.join(", "),
                err
            )))
        } else {
            Err(SmtpError::ConfigError("No Sent folders to try".to_string()))
        }
    }

    pub async fn test_smtp_connection(&self, account_email: &str) -> Result<(), SmtpError> {
        // Get account details
        let account_service = self.account_service.lock().await;
        let account = account_service
            .get_account(account_email)
            .await
            .map_err(|_| SmtpError::AccountNotFound(account_email.to_string()))?;

        // Validate SMTP configuration
        let smtp_host = account
            .smtp_host
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;
        let smtp_user = account
            .smtp_user
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;
        let smtp_pass = account
            .smtp_pass
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account_email.to_string()))?;

        let smtp_port = account.smtp_port.unwrap_or(587) as u16;
        let use_starttls = account.smtp_use_starttls.unwrap_or(true);

        // Build SMTP transport
        let creds = Credentials::new(smtp_user.clone(), smtp_pass.clone());

        let mailer: AsyncSmtpTransport<Tokio1Executor> = if use_starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .build()
        };

        // Test connection
        mailer.test_connection().await?;

        Ok(())
    }
}
