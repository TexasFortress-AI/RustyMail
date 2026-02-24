# Task ID: 58

**Title:** Implement Domain-Based Search and Address Aggregation

**Status:** done

**Dependencies:** 53 ✓

**Priority:** medium

**Description:** Create functionality to search emails by sender/recipient domain and generate aggregated reports of email addresses and domains for analytics.

**Details:**

1. Implement domain extraction and indexing:
```rust
fn extract_domain(email: &str) -> Option<String> {
    email.split('@').nth(1).map(|d| d.to_lowercase())
}
```
2. Add search_by_domain endpoint:
```rust
async fn search_by_domain(
    domain: String,
    search_in: Vec<String>, // ["from", "to", "cc"]
    folder: String
) -> Vec<EmailSummary>
```
3. Implement get_email_address_report:
```rust
#[derive(Serialize)]
struct AddressReport {
    unique_addresses: Vec<AddressCount>,
    unique_domains: Vec<DomainCount>,
    total_addresses: usize,
}
```
4. Add database indexes on email domains
5. Implement batch processing for large folders

**Test Strategy:**

1. Test domain extraction with various email formats
2. Verify domain search is case-insensitive
3. Test aggregation report accuracy
4. Performance test with 20k+ emails
5. Test with international domains
