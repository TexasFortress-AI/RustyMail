// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Following a domain's MX host to the provider whose settings to use.

use rustymail::dashboard::services::mail_provider::provider_domains;

#[test]
fn a_google_hosted_domain_leads_to_google_com() {
    assert_eq!(
        provider_domains("aspmx.l.google.com", "americanfortress.example"),
        vec!["l.google.com", "google.com"]
    );
}

#[test]
fn microsoft_and_country_code_hosts_are_walked_most_specific_first() {
    assert_eq!(
        provider_domains("example-com.mail.protection.outlook.com", "example.com"),
        vec![
            "mail.protection.outlook.com",
            "protection.outlook.com",
            "outlook.com"
        ]
    );
    assert_eq!(
        provider_domains("mx.provider.co.uk", "example.co.uk"),
        vec!["provider.co.uk", "co.uk"]
    );
}

#[test]
fn the_users_own_domain_and_bare_hosts_are_skipped() {
    // Self-hosted mail: mail.example.com only leads back to example.com.
    assert!(provider_domains("mail.example.com", "example.com").is_empty());
    assert!(provider_domains("localhost", "example.com").is_empty());
    assert!(provider_domains("example.com.", "example.com").is_empty());
}
