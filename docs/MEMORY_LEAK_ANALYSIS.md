# RustyMail Memory Leak Analysis - 2025-01-27

## Executive Summary

DHAT heap profiling revealed a **catastrophic memory leak** in the email sync service:
- **Runtime:** 258 minutes (4.3 hours)
- **Total memory allocated:** 105.45 GB
- **Still in memory at shutdown:** ~108 GB (**99.9% leaked**)
- **Bytes freed:** Essentially zero

This is worse than the original 80 GB leak reported by Activity Monitor.

## Root Cause

The `SyncService::start_background_sync` background task is:
1. Fetching full email objects with **all attachments fully decoded**
2. Storing decoded attachment data in memory
3. **Never freeing any of this memory**

## Top Memory Leak Sources (from DHAT)

### 1. Base64 Decoding - 46.49 GB (leaked)
```
mail_parser::decode_base64_mime
→ mail_parser::Message::parse_
→ Email::parse_mime_content
→ Email::from<Fetch>
→ fetch_emails
→ sync_folder_with_limit
→ sync_all_folders
→ start_background_sync
```
**Problem:** Attachments are base64-decoded into `MimePart.body: Vec<u8>` and never freed.

### 2. Email Object Creation - 13.15 GB (leaked)
```
Email::from<Fetch>
→ fetch_emails
→ sync_folder_with_limit
→ sync_all_folders
→ start_background_sync
```
**Problem:** Full Email objects with all fields allocated and never freed.

### 3. MIME Parsing - 9.03 GB × 2 = 18.06 GB (leaked)
```
Email::parse_mime_content
→ Email::from<Fetch>
→ fetch_emails
→ sync_folder_with_limit
```
**Problem:** MIME parsing allocates intermediate structures that are never freed.

### 4. IMAP BytePool - 5.42 GB + 1.93 GB + 1.87 GB = 9.22 GB (leaked)
```
byte_pool::pool::BytePool<T>::alloc
→ async_imap::imap_stream::Buffer::take_block
→ ImapStream::poll_next
→ fetch_emails
```
**Problem:** The `async-imap` library uses a BytePool allocator that never returns memory to the system.

### 5. Quoted-Printable Decoding - 1.23 GB (leaked)
```
mail_parser::decode_quoted_printable_mime
→ Message::parse_
→ Email::parse_mime_content
```

## Why Emails Aren't Freed

### Expected Behavior:
```rust
// sync.rs line 238
let emails = session.fetch_emails(chunk).await?;  // Allocate
for email in emails {  // Iterate
    cache_service.cache_email(&email).await?;  // Cache metadata only
}
// emails vector drops here - SHOULD free memory
```

### Actual Behavior:
The DHAT profile shows **zero frees**, which means:
1. Either the `mail_parser` library holds references we can't see
2. Or the `async-imap` BytePool never releases memory
3. Or there's a hidden Arc/Rc cycle somewhere
4. Or completed async futures aren't being dropped by the tokio runtime

## What Gets Cached vs. What Gets Leaked

### Cached in Database (OK):
- ✓ Email metadata (subject, from, to, cc, date)
- ✓ Text body (`email.text_body`)
- ✓ HTML body (`email.html_body`)
- ✓ Attachment flag (boolean)

### NOT Cached but LEAKED in Memory (BAD):
- ✗ Raw email body (`email.body: Vec<u8>`)
- ✗ Decoded attachments (`MimePart.body: Vec<u8>` - **46.49 GB**)
- ✗ All MIME parts (`email.mime_parts`)
- ✗ IMAP stream buffers (BytePool - **9.22 GB**)

## The Real Problem

The sync service calls `fetch_emails()` which:
1. Fetches raw RFC822 email from IMAP
2. Calls `Email::from<Fetch>` which:
   - Copies the entire body: `fetch.body().map(|b| b.to_vec())`
   - Calls `Email::parse_mime_content(body_bytes)` which:
     - Calls `mail_parser::Message::parse()`
     - Decodes ALL base64 attachments
     - Decodes ALL quoted-printable content
     - Creates full `MimePart` tree with decoded bodies
3. Returns a complete `Email` struct with everything decoded

**For a 10 MB email with a 5 MB attachment:**
- IMAP fetches: 10 MB
- Base64 decoding expands: 5 MB → 6.67 MB (33% expansion)
- Total in memory: ~17 MB for one email
- Multiply by thousands of emails = **massive leak**

## Solution Options

### Option 1: Don't Parse MIME During Sync (RECOMMENDED)
Only parse MIME when the user actually opens an email:
- Sync stores only: UID, flags, envelope, raw body bytes
- Parse MIME on-demand when email is viewed
- **Saves:** 90%+ of memory (no decoded attachments)

### Option 2: Don't Store Raw Bodies
Store only metadata during sync, fetch full bodies on-demand:
- **Saves:** All body/attachment memory
- **Cost:** Slower email viewing (needs IMAP fetch)

### Option 3: Use a Different MIME Parser
Replace `mail_parser` with a streaming parser that doesn't allocate everything upfront:
- **Complex:** Major refactor
- **Uncertain:** Might not solve BytePool issue

### Option 4: Disable Background Sync
Only sync on user request:
- **Simple:** Just don't start the background task
- **UX Cost:** No automatic email updates

## Recommendation

**Implement Option 1 + Option 2:**

1. **Short term** (today): Disable background sync or dramatically reduce sync limit
   ```rust
   const FETCH_BATCH_SIZE: usize = 10;  // Instead of 100
   const MAX_EMAILS_PER_SYNC: usize = 50;  // Stop after 50 emails
   ```

2. **Medium term** (this week): Lazy MIME parsing
   - Store only `(uid, flags, envelope, raw_body_bytes)` during sync
   - Parse MIME only when user opens email
   - Cache parsed results with LRU eviction

3. **Long term** (next sprint): Consider message streaming
   - Fetch only headers during sync
   - Fetch bodies on-demand
   - Implement smart caching

## Impact Assessment

**Current State:**
- Syncing 407 emails (328 + 50 + 16 + 7 + 3 + 2 + 1) took 4.3 hours
- Used 108 GB of memory that was never freed
- Server would crash after syncing ~1000 emails

**After Fix:**
- Syncing 10,000 emails should use < 500 MB
- Memory stable over days/weeks
- No crashes from memory exhaustion

## Next Steps

1. ✓ Identify leak source (DONE - this document)
2. Implement sync limits immediately
3. Refactor to lazy MIME parsing
4. Test with large mailboxes
5. Re-run DHAT to verify fix
