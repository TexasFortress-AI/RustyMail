use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use lettre::message::header;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;

use super::account::{AccountService};

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
}

impl SmtpService {
    pub fn new(account_service: Arc<TokioMutex<AccountService>>) -> Self {
        Self { account_service }
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

        // Build from address
        let from_mailbox: Mailbox = format!("{} <{}>", account.display_name, account.email_address)
            .parse()
            .map_err(|e| SmtpError::ConfigError(format!("Invalid from address: {}", e)))?;

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
        mailer.send(email).await?;

        Ok(SendEmailResponse {
            success: true,
            message_id,
            message: "Email sent successfully".to_string(),
        })
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
