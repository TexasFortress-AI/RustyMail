# Task ID: 87

**Title:** Fix Exchange-Style Address Parsing in Address Report

**Status:** pending

**Dependencies:** None

**Priority:** medium

**Description:** Improve email address parser to handle IMCEAEX addresses and filter out unparseable domains from reports

**Details:**

Enhance the address parser to handle Exchange addresses:

```rust
// In email_parser.rs
pub fn parse_email_address(addr: &str) -> ParsedAddress {
    // Handle IMCEAEX format
    if addr.starts_with("IMCEAEX-") {
        // Try to extract domain from the encoded string
        // Format: IMCEAEX-_o=exchangelabs_ou=exchange+20administrative+20group+20...
        if let Some(domain_match) = regex!(r"_cn=recipients_cn=[^_]+_([^_]+\.[^_]+)").captures(addr) {
            return ParsedAddress {
                local: "exchange-user".to_string(),
                domain: domain_match[1].to_string(),
                is_valid: true,
            };
        }
        return ParsedAddress {
            local: addr.to_string(),
            domain: "exchange.internal".to_string(),
            is_valid: false,
        };
    }
    
    // Standard email parsing
    if let Some(at_pos) = addr.find('@') {
        let (local, domain) = addr.split_at(at_pos);
        let domain = &domain[1..]; // Skip @
        
        // Validate domain
        if domain.is_empty() || domain == ".missing-host-name." {
            return ParsedAddress {
                local: local.to_string(),
                domain: String::new(),
                is_valid: false,
            };
        }
        
        ParsedAddress {
            local: local.to_string(),
            domain: domain.to_string(),
            is_valid: true,
        }
    } else {
        ParsedAddress {
            local: addr.to_string(),
            domain: String::new(),
            is_valid: false,
        }
    }
}

// In address_report.rs
pub async fn generate_address_report(account_id: i64) -> AddressReport {
    let emails = get_all_emails(account_id).await?;
    let mut domain_counts: HashMap<String, usize> = HashMap::new();
    let mut unresolved_count = 0;
    
    for email in emails {
        for addr in [email.from_addr, email.to_addr, email.cc_addr].iter().flatten() {
            let parsed = parse_email_address(addr);
            if parsed.is_valid && !parsed.domain.is_empty() {
                *domain_counts.entry(parsed.domain).or_insert(0) += 1;
            } else {
                unresolved_count += 1;
            }
        }
    }
    
    // Filter out invalid domains from top domains
    let mut top_domains: Vec<_> = domain_counts.into_iter()
        .filter(|(domain, _)| domain != ".missing-host-name.")
        .collect();
    top_domains.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    
    AddressReport {
        top_domains: top_domains.into_iter().take(10).collect(),
        unresolved_count,
        total_addresses: emails.len(),
    }
}
```

**Test Strategy:**

1. Unit test parser with various Exchange-style addresses
2. Test with malformed addresses and edge cases
3. Verify .missing-host-name. is filtered from reports
4. Test regex extraction of domains from IMCEAEX strings
5. Verify unresolved_count is accurate
