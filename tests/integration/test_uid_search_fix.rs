// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/// Integration test to verify UID SEARCH fix for Task #44
/// Tests that search_emails returns actual UIDs, not message sequence numbers

use rustymail::imap::session::ImapSession;

#[tokio::test]
async fn test_uid_search_returns_uids_not_sequence_numbers() {
    // Load credentials from environment or config
    let host = std::env::var("IMAP_HOST")
        .unwrap_or_else(|_| "p3plzcpnl505455.prod.phx3.secureserver.net".to_string());
    let port = std::env::var("IMAP_PORT")
        .unwrap_or_else(|_| "993".to_string())
        .parse::<u16>()
        .unwrap_or(993);
    let username = std::env::var("IMAP_USER")
        .unwrap_or_else(|_| "chris@texasfortress.ai".to_string());
    let password = std::env::var("IMAP_PASS")
        .unwrap_or_else(|_| "D.6WVnz&zVJh".to_string());

    // Create IMAP session
    let session = ImapSession::new(host, port, username, password)
        .await
        .expect("Failed to create IMAP session");

    // Select INBOX.Sent folder
    session
        .select_folder("INBOX.Sent")
        .await
        .expect("Failed to select INBOX.Sent");

    // Search for all messages
    let uids = session
        .search_emails("ALL")
        .await
        .expect("search_emails failed");

    println!("Search returned {} UIDs", uids.len());
    println!("First 10 UIDs: {:?}", &uids[..uids.len().min(10)]);
    if uids.len() > 10 {
        println!("Last 10 UIDs: {:?}", &uids[uids.len() - 10..]);
    }

    // The INBOX.Sent folder has deleted messages, so UIDs should have gaps
    // Expected UIDs: 1-9, 33-71 (48 total)
    // If we were getting sequence numbers, we'd get 1-48 continuously

    assert!(
        uids.len() > 25,
        "Expected more than 25 UIDs, got {}",
        uids.len()
    );

    // Check that UIDs have gaps (indicating they're real UIDs, not sequence numbers)
    let mut has_gap = false;
    for i in 1..uids.len() {
        if uids[i] != uids[i - 1] + 1 {
            has_gap = true;
            println!(
                "Found UID gap: {} -> {} (missing UIDs in between)",
                uids[i - 1],
                uids[i]
            );
            break;
        }
    }

    assert!(
        has_gap,
        "UIDs should have gaps due to deleted messages. Got continuous sequence: {:?}",
        &uids[..uids.len().min(20)]
    );

    // Verify we got 48 UIDs as expected
    assert_eq!(
        uids.len(),
        48,
        "Expected 48 UIDs in INBOX.Sent, got {}",
        uids.len()
    );

    println!("✓ Test passed: search_emails correctly returns UIDs, not sequence numbers");
}
