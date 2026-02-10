// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! XOAUTH2 authenticator for async-imap IMAP connections.
//!
//! Implements the `async_imap::Authenticator` trait for OAuth2-based
//! IMAP authentication (Microsoft 365, Gmail, etc.).
//!
//! Token format: `user=<email>\x01auth=Bearer <access_token>\x01\x01`
//!
//! Note: `async_imap::Authenticator::process()` returns the raw token;
//! the library itself handles base64 encoding before sending to the server.

use async_imap::Authenticator;

/// XOAUTH2 authenticator for IMAP SASL authentication.
#[derive(Debug, Clone)]
pub struct XOAuth2Authenticator {
    /// The pre-formatted XOAUTH2 token string.
    token: String,
}

impl XOAuth2Authenticator {
    /// Create a new XOAUTH2 authenticator.
    ///
    /// `email` — the user's email address (e.g., "user@outlook.com")
    /// `access_token` — the OAuth2 access token
    pub fn new(email: &str, access_token: &str) -> Self {
        Self {
            token: format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token),
        }
    }
}

impl Authenticator for XOAuth2Authenticator {
    type Response = String;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        self.token.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xoauth2_token_format() {
        let mut auth = XOAuth2Authenticator::new("user@outlook.com", "my-token-123");
        let response = auth.process(b"");
        assert_eq!(
            response,
            "user=user@outlook.com\x01auth=Bearer my-token-123\x01\x01"
        );
    }

    #[test]
    fn test_xoauth2_ignores_challenge() {
        let mut auth = XOAuth2Authenticator::new("a@b.com", "tok");
        // Challenge content should be ignored for XOAUTH2
        let r1 = auth.process(b"some challenge");
        let r2 = auth.process(b"");
        assert_eq!(r1, r2);
    }
}
