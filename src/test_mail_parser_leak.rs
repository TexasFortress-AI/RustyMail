// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use mail_parser::{Message, MimeHeaders};
use std::time::Instant;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Test to isolate mail_parser memory behavior
/// Run with: cargo test test_mail_parser_leak --release -- --nocapture
#[cfg(test)]
mod tests {
    use super::*;

    fn get_memory_usage() -> usize {
        // Get RSS memory in bytes - platform specific
        #[cfg(target_os = "linux")]
        {
            let pid = std::process::id();
            let status = std::fs::read_to_string(format!("/proc/{}/status", pid))
                .unwrap_or_default();

            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
            0
        }

        #[cfg(target_os = "macos")]
        {
            // On macOS, we'll use a simplified approach
            // In production, you'd use mach_task_info or similar
            println!("Memory tracking on macOS not fully implemented");
            0
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // For other platforms, return a placeholder value
            println!("Memory tracking not available on this platform");
            0
        }
    }

    #[test]
    fn test_mail_parser_memory_leak() {
        println!("Testing mail_parser memory behavior...");

        // Create a test email with a large base64 attachment
        let attachment_size = 10 * 1024 * 1024; // 10 MB
        let attachment_data = vec![b'A'; attachment_size];
        let base64_attachment = BASE64.encode(&attachment_data);

        let email_content = format!(
            r#"From: test@example.com
To: recipient@example.com
Subject: Test Email with Attachment
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary="boundary123"

--boundary123
Content-Type: text/plain; charset=utf-8

This is the email body.

--boundary123
Content-Type: application/octet-stream; name="test.bin"
Content-Transfer-Encoding: base64
Content-Disposition: attachment; filename="test.bin"

{}

--boundary123--
"#,
            base64_attachment
        );

        let email_bytes = email_content.as_bytes();

        println!("Test email size: {} MB", email_bytes.len() as f64 / 1024.0 / 1024.0);

        // Measure memory before parsing
        let mem_before = get_memory_usage();
        println!("Memory before parsing: {} MB", mem_before as f64 / 1024.0 / 1024.0);

        // Parse emails in a loop to see if memory accumulates
        for i in 0..5 {
            println!("\n--- Iteration {} ---", i + 1);

            // Parse the email
            let start = Instant::now();
            let message = Message::parse(email_bytes);
            let parse_time = start.elapsed();

            if let Some(msg) = message {
                // Access the parsed data to ensure it's fully materialized
                let attachments: Vec<_> = msg.attachments().collect();
                let attachment_count = attachments.len();
                let total_attachment_size: usize = attachments
                    .iter()
                    .map(|a| a.contents().len())
                    .sum();

                println!("Parsed {} attachments, total decoded size: {} MB",
                         attachment_count,
                         total_attachment_size as f64 / 1024.0 / 1024.0);
                println!("Parse time: {:?}", parse_time);

                // Explicitly drop the message
                drop(msg);
            } else {
                println!("Failed to parse email!");
            }

            // Force a small delay to let the system update memory stats
            std::thread::sleep(std::time::Duration::from_millis(100));

            let mem_after = get_memory_usage();
            let mem_used = (mem_after as i64 - mem_before as i64) as f64 / 1024.0 / 1024.0;
            println!("Memory after iteration {}: {} MB (delta: {:+.2} MB)",
                     i + 1,
                     mem_after as f64 / 1024.0 / 1024.0,
                     mem_used);
        }

        // Final memory check after all drops
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mem_final = get_memory_usage();
        let total_leaked = (mem_final as i64 - mem_before as i64) as f64 / 1024.0 / 1024.0;

        println!("\n=== Final Results ===");
        println!("Initial memory: {} MB", mem_before as f64 / 1024.0 / 1024.0);
        println!("Final memory: {} MB", mem_final as f64 / 1024.0 / 1024.0);
        println!("Memory leaked: {:+.2} MB", total_leaked);

        // Assert that memory shouldn't grow more than 20 MB (allowing for some overhead)
        assert!(total_leaked < 20.0,
                "Memory leak detected! Leaked {:.2} MB after parsing 5 emails",
                total_leaked);
    }

    #[test]
    fn test_our_email_struct_memory() {
        use crate::imap::types::Email;

        println!("\nTesting our Email struct memory behavior...");

        let mem_before = get_memory_usage();
        println!("Memory before creating Emails: {} MB", mem_before as f64 / 1024.0 / 1024.0);

        // Create Email structs with large bodies
        let mut emails: Vec<Email> = Vec::new();

        for i in 0..5 {
            let body_size = 10 * 1024 * 1024; // 10 MB
            let body = vec![b'X'; body_size];

            let email = Email {
                uid: i,
                flags: vec!["\\Seen".to_string()],
                internal_date: None,
                envelope: None,
                body: Some(body.clone()),
                mime_parts: vec![],
                text_body: Some(String::from_utf8_lossy(&body[..1000]).to_string()),
                html_body: None,
                attachments: vec![],
            };

            emails.push(email);

            let mem_current = get_memory_usage();
            println!("Memory after creating Email {}: {} MB (delta: {:+.2} MB)",
                     i + 1,
                     mem_current as f64 / 1024.0 / 1024.0,
                     (mem_current as i64 - mem_before as i64) as f64 / 1024.0 / 1024.0);
        }

        println!("\nDropping all emails...");
        drop(emails);

        // Give time for memory to be freed
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mem_final = get_memory_usage();
        let total_leaked = (mem_final as i64 - mem_before as i64) as f64 / 1024.0 / 1024.0;

        println!("\n=== Final Results ===");
        println!("Initial memory: {} MB", mem_before as f64 / 1024.0 / 1024.0);
        println!("Final memory: {} MB", mem_final as f64 / 1024.0 / 1024.0);
        println!("Memory leaked: {:+.2} MB", total_leaked);

        // Our Email struct should definitely free memory when dropped
        assert!(total_leaked < 5.0,
                "Memory leak in Email struct! Leaked {:.2} MB after dropping 5 emails",
                total_leaked);
    }
}