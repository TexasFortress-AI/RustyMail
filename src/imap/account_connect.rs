// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Load active IMAP accounts from the cache database and open sessions for them.
//!
//! Shared by the standalone binaries (`rustymail-sync`, `rustymail-triage`),
//! which run outside the server and read account credentials from the database.

use crate::dashboard::services::encryption::{CredentialEncryption, EncryptionError};
use crate::imap::client::ImapClient;
use crate::imap::session::AsyncImapSessionWrapper;
use log::{error, info};
use sqlx::{Row, SqlitePool};

/// An active account with its credentials decrypted in memory.
pub struct ImapAccount {
    pub email_address: String,
    pub imap_host: String,
    pub imap_port: i64,
    pub imap_user: String,
    pub imap_pass: String,
    pub imap_use_tls: bool,
    pub imap_use_starttls: bool,
    pub oauth_provider: Option<String>,
    pub oauth_access_token: Option<String>,
}

/// Load active accounts, optionally only `only_email`.
///
/// Credentials are stored encrypted (ENC:v1:) and decrypted with
/// ENCRYPTION_MASTER_KEY. An account whose credentials cannot be decrypted is
/// logged and skipped, so one bad row does not stop the others.
pub async fn load_active_accounts(
    pool: &SqlitePool,
    only_email: Option<&str>,
) -> Result<Vec<ImapAccount>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT email_address, imap_host, imap_port, imap_user, imap_pass, imap_use_tls, imap_use_starttls,
               oauth_provider, oauth_access_token
        FROM accounts WHERE is_active = 1 AND (?1 IS NULL OR email_address = ?1)
        "#,
    )
    .bind(only_email)
    .fetch_all(pool)
    .await?;

    let encryption = CredentialEncryption::new();
    let mut accounts = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let email_address: String = row.get("email_address");
        let raw_pass: String = row.get("imap_pass");
        let raw_token: Option<String> = row.get("oauth_access_token");
        let (imap_pass, oauth_access_token) = match decrypt_account_secrets(
            &encryption,
            &raw_pass,
            raw_token.as_deref(),
        ) {
            Ok(v) => v,
            Err(e) => {
                error!("Cannot decrypt credentials for {} (is ENCRYPTION_MASTER_KEY set correctly?): {}", email_address, e);
                continue;
            }
        };
        accounts.push(ImapAccount {
            email_address,
            imap_host: row.get("imap_host"),
            imap_port: row.get("imap_port"),
            imap_user: row.get("imap_user"),
            imap_pass,
            imap_use_tls: row.get("imap_use_tls"),
            imap_use_starttls: row.get("imap_use_starttls"),
            oauth_provider: row.get("oauth_provider"),
            oauth_access_token,
        });
    }
    Ok(accounts)
}

impl ImapAccount {
    /// Open an IMAP session: XOAUTH2 for OAuth accounts, password for others.
    pub async fn connect(
        &self,
    ) -> Result<ImapClient<AsyncImapSessionWrapper>, Box<dyn std::error::Error>> {
        let client = if self.oauth_provider.is_some() {
            let token = self
                .oauth_access_token
                .as_deref()
                .ok_or("OAuth account has no access token — complete OAuth flow first")?;
            info!("Using XOAUTH2 authentication for {}", self.email_address);
            ImapClient::<AsyncImapSessionWrapper>::connect_with_xoauth2_and_security(
                &self.imap_host,
                self.imap_port as u16,
                &self.imap_user,
                token,
                self.imap_use_tls,
                self.imap_use_starttls,
            )
            .await?
        } else {
            ImapClient::<AsyncImapSessionWrapper>::connect_with_security(
                &self.imap_host,
                self.imap_port as u16,
                &self.imap_user,
                &self.imap_pass,
                self.imap_use_tls,
                self.imap_use_starttls,
            )
            .await?
        };
        info!(
            "Connected to IMAP server {} for {}",
            self.imap_host, self.email_address
        );
        Ok(client)
    }
}

fn decrypt_account_secrets(
    encryption: &CredentialEncryption,
    imap_pass: &str,
    oauth_access_token: Option<&str>,
) -> Result<(String, Option<String>), EncryptionError> {
    Ok((
        encryption.decrypt(imap_pass)?,
        encryption.decrypt_opt(oauth_access_token)?,
    ))
}

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[test]
    fn decrypts_encrypted_and_passes_plaintext() {
        let enc = CredentialEncryption::from_key_hex(&"ab".repeat(32)).unwrap();
        let ct = enc.encrypt("pw").unwrap();
        let tok = enc.encrypt("tok").unwrap();
        let (p, t) = decrypt_account_secrets(&enc, &ct, Some(&tok)).unwrap();
        assert_eq!(p, "pw");
        assert_eq!(t.as_deref(), Some("tok"));

        let (p, t) = decrypt_account_secrets(&enc, "legacy", None).unwrap();
        assert_eq!(p, "legacy");
        assert!(t.is_none());
    }

    #[test]
    fn wrong_key_is_an_error() {
        let a = CredentialEncryption::from_key_hex(&"ab".repeat(32)).unwrap();
        let b = CredentialEncryption::from_key_hex(&"cd".repeat(32)).unwrap();
        let ct = a.encrypt("pw").unwrap();
        assert!(decrypt_account_secrets(&b, &ct, None).is_err());
    }
}
