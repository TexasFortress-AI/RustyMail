// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! SMTP transport builder with OAuth2 XOAUTH2 support.
//!
//! Provides a helper to build an `AsyncSmtpTransport` that uses either
//! password-based PLAIN auth or XOAUTH2, depending on the account's
//! `oauth_provider` field.

use lettre::{
    transport::smtp::authentication::{Credentials, Mechanism},
    AsyncSmtpTransport, Tokio1Executor,
};
use log::info;

use super::account::Account;
use super::smtp::SmtpError;

/// Build an async SMTP transport for the given account.
///
/// If the account has `oauth_provider` set and an `oauth_access_token`,
/// the transport uses XOAUTH2. Otherwise it uses password (PLAIN) auth.
pub fn build_smtp_transport(
    account: &Account,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, SmtpError> {
    let smtp_host = account
        .smtp_host
        .as_ref()
        .ok_or_else(|| SmtpError::MissingCredentials(account.email_address.clone()))?;
    let smtp_port = account.smtp_port.unwrap_or(587) as u16;
    let use_starttls = account.smtp_use_starttls.unwrap_or(true);

    if account.is_oauth() {
        let access_token = account
            .oauth_access_token
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(
                format!("{} (OAuth token missing)", account.email_address),
            ))?;

        // For XOAUTH2: user = email, secret = access_token
        let creds = Credentials::new(account.email_address.clone(), access_token.clone());

        info!("Building SMTP transport with XOAUTH2 for {}", account.email_address);

        let mailer = if use_starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .authentication(vec![Mechanism::Xoauth2])
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
                .map_err(|e| SmtpError::ConfigError(format!("SMTP relay error: {}", e)))?
                .port(smtp_port)
                .credentials(creds)
                .authentication(vec![Mechanism::Xoauth2])
                .build()
        };

        Ok(mailer)
    } else {
        let smtp_user = account
            .smtp_user
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account.email_address.clone()))?;
        let smtp_pass = account
            .smtp_pass
            .as_ref()
            .ok_or_else(|| SmtpError::MissingCredentials(account.email_address.clone()))?;

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

        Ok(mailer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_account(oauth: bool) -> Account {
        Account {
            email_address: "user@outlook.com".to_string(),
            id: "user@outlook.com".to_string(),
            display_name: "Test User".to_string(),
            provider_type: Some("outlook".to_string()),
            imap_host: "outlook.office365.com".to_string(),
            imap_port: 993,
            imap_user: "user@outlook.com".to_string(),
            imap_pass: String::new(),
            imap_use_tls: true,
            smtp_host: Some("smtp.office365.com".to_string()),
            smtp_port: Some(587),
            smtp_user: if oauth { None } else { Some("user@outlook.com".to_string()) },
            smtp_pass: if oauth { None } else { Some("password".to_string()) },
            smtp_use_tls: Some(true),
            smtp_use_starttls: Some(true),
            oauth_provider: if oauth { Some("microsoft".to_string()) } else { None },
            oauth_access_token: if oauth { Some("test-token".to_string()) } else { None },
            oauth_refresh_token: if oauth { Some("test-refresh".to_string()) } else { None },
            oauth_token_expiry: if oauth { Some(9999999999) } else { None },
            is_active: true,
            is_default: false,
            connection_status: None,
        }
    }

    #[test]
    fn test_build_password_transport() {
        let account = make_test_account(false);
        let result = build_smtp_transport(&account);
        // This should succeed (transport built, not connected)
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_oauth_transport() {
        let account = make_test_account(true);
        let result = build_smtp_transport(&account);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_oauth_transport_missing_token() {
        let mut account = make_test_account(true);
        account.oauth_access_token = None; // Remove token
        let result = build_smtp_transport(&account);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_transport_missing_smtp_host() {
        let mut account = make_test_account(false);
        account.smtp_host = None;
        let result = build_smtp_transport(&account);
        assert!(result.is_err());
    }
}
