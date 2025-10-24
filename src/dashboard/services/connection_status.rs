// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// Status of a connection attempt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    /// Connection successful
    Success,
    /// Connection failed with error
    Failed,
    /// Never attempted
    Unknown,
}

/// Details of a connection attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAttempt {
    /// When the attempt was made
    pub timestamp: DateTime<Utc>,
    /// Status of the attempt
    pub status: ConnectionStatus,
    /// Error message if failed, success message if succeeded
    pub message: String,
}

impl ConnectionAttempt {
    /// Create a successful connection attempt
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            status: ConnectionStatus::Success,
            message: message.into(),
        }
    }

    /// Create a failed connection attempt
    pub fn failed(error: impl std::fmt::Display) -> Self {
        Self {
            timestamp: Utc::now(),
            status: ConnectionStatus::Failed,
            message: error.to_string(),
        }
    }

    /// Create an unknown/never tested connection attempt
    pub fn unknown() -> Self {
        Self {
            timestamp: Utc::now(),
            status: ConnectionStatus::Unknown,
            message: "Never tested".to_string(),
        }
    }
}

impl Default for ConnectionAttempt {
    fn default() -> Self {
        Self::unknown()
    }
}

/// Connection status for an email account
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountConnectionStatus {
    /// Email address (account ID)
    pub email_address: String,
    /// Last IMAP connection attempt
    #[serde(default)]
    pub imap: ConnectionAttempt,
    /// Last SMTP connection attempt
    #[serde(default)]
    pub smtp: ConnectionAttempt,
}

impl AccountConnectionStatus {
    /// Create a new connection status for an account
    pub fn new(email_address: impl Into<String>) -> Self {
        Self {
            email_address: email_address.into(),
            imap: ConnectionAttempt::unknown(),
            smtp: ConnectionAttempt::unknown(),
        }
    }

    /// Update IMAP connection status with success
    pub fn set_imap_success(&mut self, message: impl Into<String>) {
        self.imap = ConnectionAttempt::success(message);
    }

    /// Update IMAP connection status with failure
    pub fn set_imap_failed(&mut self, error: impl std::fmt::Display) {
        self.imap = ConnectionAttempt::failed(error);
    }

    /// Update SMTP connection status with success
    pub fn set_smtp_success(&mut self, message: impl Into<String>) {
        self.smtp = ConnectionAttempt::success(message);
    }

    /// Update SMTP connection status with failure
    pub fn set_smtp_failed(&mut self, error: impl std::fmt::Display) {
        self.smtp = ConnectionAttempt::failed(error);
    }

    /// Check if IMAP is healthy
    pub fn is_imap_healthy(&self) -> bool {
        self.imap.status == ConnectionStatus::Success
    }

    /// Check if SMTP is healthy
    pub fn is_smtp_healthy(&self) -> bool {
        self.smtp.status == ConnectionStatus::Success
    }

    /// Check if both protocols are healthy
    pub fn is_healthy(&self) -> bool {
        self.is_imap_healthy() && self.is_smtp_healthy()
    }
}
