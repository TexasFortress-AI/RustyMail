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

        // Send the email
        mailer.send(email.clone()).await?;

        // Save sent email to Sent folder via IMAP with timeout
        // Note: We do this as a best-effort - if it fails, we still return success
        // since the email was successfully sent via SMTP
        let append_timeout = Duration::from_secs(10); // 10 second timeout for IMAP append
        match timeout(append_timeout, self.append_to_sent_folder(&account.email_address, &email)).await {
            Ok(Ok(_)) => {
                log::info!("Successfully saved sent email to Sent folder");
            }
            Ok(Err(e)) => {
                log::warn!("Failed to save sent email to Sent folder: {}", e);
            }
            Err(_) => {
                log::warn!("Timeout saving sent email to Sent folder (exceeded {} seconds)", append_timeout.as_secs());
            }
        }

        Ok(SendEmailResponse {
            success: true,
            message_id,
            message: "Email sent successfully".to_string(),
        })
    }

    /// Append a sent message to the IMAP Sent folder
    async fn append_to_sent_folder(
        &self,
        account_email: &str,
        email: &Message,
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

        // Convert the email to RFC822 format (raw bytes)
        let email_bytes = email.formatted();

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
