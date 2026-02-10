// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! XOAUTH2 authenticator for async-imap
//!
//! XOAUTH2 is a SASL authentication mechanism used by Google and Microsoft
//! for OAuth2-based IMAP authentication.
//! Format: base64("user=" + username + "\x01auth=Bearer " + access_token + "\x01\x01")

use async_imap::Authenticator;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// XOAUTH2 authenticator for OAuth2 IMAP authentication
#[derive(Debug, Clone)]
pub struct XOAuth2Authenticator {
    username: String,
    access_token: String,
}

impl XOAuth2Authenticator {
    /// Create a new XOAUTH2 authenticator
    pub fn new(username: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            access_token: access_token.into(),
        }
    }

    /// Encode the XOAUTH2 string
    /// Format: user={user}^Aauth=Bearer {token}^A^A
    /// where ^A is ASCII 0x01 (control character)
    fn encode(&self) -> Vec<u8> {
        let auth_string = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.username, self.access_token
        );
        BASE64.encode(auth_string).into_bytes()
    }
}

impl Authenticator for XOAuth2Authenticator {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        self.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xoauth2_encode() {
        let mut auth = XOAuth2Authenticator::new("user@example.com", "ya29.vF9dft4qmTc2Nvb3RlckBhdHRhdmlzdGEuY29tCg");
        let encoded = auth.process(&[]);

        // Should be valid base64
        let decoded = BASE64.decode(&encoded).unwrap();
        let decoded_str = String::from_utf8(decoded).unwrap();

        // Check format
        assert!(decoded_str.contains("user=user@example.com"));
        assert!(decoded_str.contains("auth=Bearer ya29.vF9dft4qmTc2Nvb3RlckBhdHRhdmlzdGEuY29tCg"));
    }
}
