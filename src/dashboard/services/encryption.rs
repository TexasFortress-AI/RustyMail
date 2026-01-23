// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Credential encryption module for storing sensitive data at rest.
//!
//! Uses AES-256-GCM for authenticated encryption. The master key is loaded
//! from the `ENCRYPTION_MASTER_KEY` environment variable. If no key is set,
//! encryption is disabled for backward compatibility.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during encryption/decryption operations.
#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption key not configured - set ENCRYPTION_MASTER_KEY environment variable")]
    KeyNotConfigured,
    #[error("Invalid encryption key: must be 32 bytes (64 hex characters)")]
    InvalidKeyLength,
    #[error("Invalid hex in encryption key: {0}")]
    InvalidKeyHex(String),
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid encrypted data format: {0}")]
    InvalidFormat(String),
}

/// Encrypted data container storing the nonce and ciphertext together.
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedData {
    /// Random nonce (12 bytes, base64 encoded)
    nonce: String,
    /// Encrypted ciphertext (base64 encoded)
    ciphertext: String,
}

/// Credential encryption service using AES-256-GCM.
///
/// The service loads the master key from the `ENCRYPTION_MASTER_KEY` environment
/// variable. The key must be 32 bytes (64 hex characters).
pub struct CredentialEncryption {
    cipher: Option<Aes256Gcm>,
}

impl CredentialEncryption {
    /// Create a new CredentialEncryption instance.
    ///
    /// Loads the master key from `ENCRYPTION_MASTER_KEY` environment variable.
    /// If the key is not set, encryption is disabled (encrypt/decrypt become no-ops).
    pub fn new() -> Self {
        match Self::load_key_from_env() {
            Ok(cipher) => {
                debug!("Credential encryption initialized with master key");
                Self { cipher: Some(cipher) }
            }
            Err(EncryptionError::KeyNotConfigured) => {
                warn!("ENCRYPTION_MASTER_KEY not set - credentials will be stored in plaintext");
                Self { cipher: None }
            }
            Err(e) => {
                warn!("Failed to initialize encryption: {} - credentials will be stored in plaintext", e);
                Self { cipher: None }
            }
        }
    }

    /// Check if encryption is enabled.
    pub fn is_enabled(&self) -> bool {
        self.cipher.is_some()
    }

    /// Encrypt a plaintext credential.
    ///
    /// Returns the encrypted data as a prefixed string: `ENC:v1:<base64-json>`.
    /// If encryption is disabled, returns the plaintext unchanged.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptionError> {
        let cipher = match &self.cipher {
            Some(c) => c,
            None => return Ok(plaintext.to_string()), // Encryption disabled
        };

        // Generate random 12-byte nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // Package as JSON with base64-encoded components
        let encrypted_data = EncryptedData {
            nonce: BASE64.encode(nonce_bytes),
            ciphertext: BASE64.encode(ciphertext),
        };

        let json = serde_json::to_string(&encrypted_data)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // Prefix with version marker for future compatibility
        Ok(format!("ENC:v1:{}", BASE64.encode(json.as_bytes())))
    }

    /// Decrypt an encrypted credential.
    ///
    /// Expects the encrypted data in format: `ENC:v1:<base64-json>`.
    /// If encryption is disabled or the data is not encrypted, returns unchanged.
    pub fn decrypt(&self, encrypted: &str) -> Result<String, EncryptionError> {
        // Check if this is encrypted data
        if !encrypted.starts_with("ENC:v1:") {
            // Not encrypted - return as-is (backward compatibility)
            return Ok(encrypted.to_string());
        }

        let cipher = match &self.cipher {
            Some(c) => c,
            None => {
                return Err(EncryptionError::KeyNotConfigured);
            }
        };

        // Extract and decode the base64 JSON
        let encoded_json = &encrypted[7..]; // Skip "ENC:v1:"
        let json_bytes = BASE64
            .decode(encoded_json)
            .map_err(|e| EncryptionError::InvalidFormat(format!("base64 decode: {}", e)))?;

        let json_str = String::from_utf8(json_bytes)
            .map_err(|e| EncryptionError::InvalidFormat(format!("utf8 decode: {}", e)))?;

        let encrypted_data: EncryptedData = serde_json::from_str(&json_str)
            .map_err(|e| EncryptionError::InvalidFormat(format!("json parse: {}", e)))?;

        // Decode nonce and ciphertext
        let nonce_bytes = BASE64
            .decode(&encrypted_data.nonce)
            .map_err(|e| EncryptionError::InvalidFormat(format!("nonce decode: {}", e)))?;

        if nonce_bytes.len() != 12 {
            return Err(EncryptionError::InvalidFormat(format!(
                "invalid nonce length: {} (expected 12)",
                nonce_bytes.len()
            )));
        }

        let ciphertext = BASE64
            .decode(&encrypted_data.ciphertext)
            .map_err(|e| EncryptionError::InvalidFormat(format!("ciphertext decode: {}", e)))?;

        // Decrypt
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| EncryptionError::DecryptionFailed(format!("utf8: {}", e)))
    }

    /// Load the AES-256 cipher from the ENCRYPTION_MASTER_KEY environment variable.
    fn load_key_from_env() -> Result<Aes256Gcm, EncryptionError> {
        let key_hex = std::env::var("ENCRYPTION_MASTER_KEY")
            .map_err(|_| EncryptionError::KeyNotConfigured)?;

        // Key must be 32 bytes = 64 hex characters
        if key_hex.len() != 64 {
            return Err(EncryptionError::InvalidKeyLength);
        }

        let key_bytes = hex::decode(&key_hex)
            .map_err(|e| EncryptionError::InvalidKeyHex(e.to_string()))?;

        Ok(Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| EncryptionError::InvalidKeyLength)?)
    }
}

impl Default for CredentialEncryption {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_encrypt_decrypt_roundtrip() {
        // Set a test key
        std::env::set_var(
            "ENCRYPTION_MASTER_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let encryption = CredentialEncryption::new();
        assert!(encryption.is_enabled());

        let plaintext = "my-secret-password";
        let encrypted = encryption.encrypt(plaintext).unwrap();

        // Encrypted should be different from plaintext
        assert_ne!(encrypted, plaintext);
        assert!(encrypted.starts_with("ENC:v1:"));

        // Decrypt should return original
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);

        // Clean up
        std::env::remove_var("ENCRYPTION_MASTER_KEY");
    }

    #[test]
    #[serial]
    fn test_plaintext_passthrough() {
        // Without encryption key
        std::env::remove_var("ENCRYPTION_MASTER_KEY");

        let encryption = CredentialEncryption::new();
        assert!(!encryption.is_enabled());

        let plaintext = "my-password";
        let result = encryption.encrypt(plaintext).unwrap();

        // Should return unchanged
        assert_eq!(result, plaintext);

        // Decrypt should also return unchanged
        let decrypted = encryption.decrypt(plaintext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    #[serial]
    fn test_decrypt_unencrypted_data() {
        std::env::set_var(
            "ENCRYPTION_MASTER_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let encryption = CredentialEncryption::new();

        // Decrypting plaintext should return unchanged (backward compatibility)
        let plaintext = "old-unencrypted-password";
        let decrypted = encryption.decrypt(plaintext).unwrap();
        assert_eq!(decrypted, plaintext);

        std::env::remove_var("ENCRYPTION_MASTER_KEY");
    }

    #[test]
    #[serial]
    fn test_invalid_key_length() {
        std::env::set_var("ENCRYPTION_MASTER_KEY", "tooshort");

        let encryption = CredentialEncryption::new();
        assert!(!encryption.is_enabled()); // Falls back to disabled

        std::env::remove_var("ENCRYPTION_MASTER_KEY");
    }

    #[test]
    #[serial]
    fn test_unique_ciphertexts() {
        std::env::set_var(
            "ENCRYPTION_MASTER_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let encryption = CredentialEncryption::new();
        let plaintext = "same-password";

        // Encrypting the same plaintext should produce different ciphertexts (random nonce)
        let encrypted1 = encryption.encrypt(plaintext).unwrap();
        let encrypted2 = encryption.encrypt(plaintext).unwrap();
        assert_ne!(encrypted1, encrypted2);

        // Both should decrypt to the same plaintext
        assert_eq!(encryption.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(encryption.decrypt(&encrypted2).unwrap(), plaintext);

        std::env::remove_var("ENCRYPTION_MASTER_KEY");
    }
}
