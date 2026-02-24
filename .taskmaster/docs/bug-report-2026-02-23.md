# RustyMail Bug Report & Feature Requests — February 23, 2026

## Context
These issues were found during MCP tool testing for an insurance claim investigation using the jobs@mleehealthcare.ai account. The tools need to reliably extract email evidence from the cached mailbox.

## PRIORITY 1 — Critical

### ISSUE-001: Implement OAuth Token Auto-Refresh

**Priority:** Critical
**Category:** Auth/Infrastructure

The OAuth token for Microsoft 365 accounts expires every ~7 days without auto-refresh. When expired, all IMAP operations fail silently. The user must manually re-authenticate via the web UI each time.

**Requirements:**
- Implement a background task that checks `oauth_token_expiry` and proactively refreshes the access token using the Microsoft refresh token when less than 1 hour remains before expiry
- Use the standard Microsoft OAuth2 refresh flow: `POST https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token` with `grant_type=refresh_token`
- Store the new access token and updated expiry in the account store (via `AccountService::update_oauth_tokens()`)
- If the refresh token itself has expired (90-day lifetime), alert the user via logging and a UI notification
- Log all token refresh attempts and failures
- The refresh token is already stored in `config/accounts.json` alongside the access token

**Test strategy:** Unit test the refresh flow with mock HTTP responses. Integration test with an actual expired token to verify the refresh cycle works end-to-end.

---

## PRIORITY 2 — Data Quality

### ISSUE-002: Deduplicate Emails During Sync (Root Cause Fix)

**Priority:** High
**Category:** Sync/Database

Multiple sync passes of the same folder create duplicate email entries in the cache. The same email (identified by message_id) appears multiple times with different database row IDs/UIDs. This inflates counts and causes duplicate results in search, thread, and address report tools.

**Requirements:**
- During sync, before inserting a new email into the cache, check if an email with the same `message_id` already exists in the cache for that folder and account
- If a duplicate exists, update the existing record (flags, metadata) rather than inserting a new row
- Add a UNIQUE constraint or index on (account_id, folder, message_id) in the database schema to prevent future duplicates
- Run a one-time migration to deduplicate existing data (keep the most recent row, delete older duplicates)

**Test strategy:** Unit test the upsert logic. Test with a known-duplicate dataset to verify deduplication.

---

### ISSUE-003: Deduplicate get_email_thread Query Results

**Priority:** Medium
**Category:** Query/Data
**Depends on:** ISSUE-002

Even after sync deduplication (ISSUE-002), add query-level deduplication to `get_email_thread` as defense in depth.

**Requirements:**
- Modify the `get_thread_emails` query in the cache service to use `DISTINCT` on `message_id` (or equivalent GROUP BY), keeping only the most recent row per message_id
- Ensure the thread results are ordered chronologically (earliest first)

**Test strategy:** Unit test with intentionally duplicate data to verify deduplication at query level.

---

### ISSUE-004: Fix Malformed Domain in Address Report

**Priority:** Medium
**Category:** Parsing

The `get_address_report` tool shows `@.missing-host-name.` as the #1 domain with 5,467 entries. These are emails with Exchange-style internal addresses (e.g., `IMCEAEX-_o=exchangelabs_ou=...`) that can't be parsed into standard `user@domain` format.

**Requirements:**
- Improve the email address parser to handle Exchange-style addresses and extract a meaningful domain when possible
- Categorize truly unparseable addresses separately in the report (e.g., `"unresolved_count": 5467`) rather than lumping them under a fake domain
- Filter `@.missing-host-name.` from the `top_domains` ranking so it doesn't pollute results
- If an IMCEAEX address contains a recognizable domain pattern, extract and use it

**Test strategy:** Unit test the address parser with Exchange-style addresses, malformed addresses, and normal addresses.

---

### ISSUE-005: Fix has_attachments Inconsistency Across Endpoints

**Priority:** Medium
**Category:** Data/Consistency

The same email returns different `has_attachments` values from `list_cached_emails` vs `get_email_by_uid`. Some emails whose body text references attachments also show `has_attachments: false`.

**Requirements:**
- Ensure `has_attachments` is computed once during sync and stored consistently in the database cache
- Both `list_cached_emails` and `get_email_by_uid` must read the same stored value (no re-computation at query time)
- Consider distinguishing between inline images (CID references) and true file attachments if not already done

**Test strategy:** Unit test that both endpoints return the same has_attachments value for the same email.

---

### ISSUE-006: Fix INBOX Read/Unread Count

**Priority:** Medium
**Category:** Sync/IMAP

`get_folder_stats` for INBOX reports `read: 19669, unread: 0` even though Outlook web UI shows all as unread. The `INBOX/OnlineArchiveBackup` folder reports counts correctly.

**Requirements:**
- Ensure all IMAP fetch operations use `BODY.PEEK[]` instead of `BODY[]` to avoid marking messages as read during sync
- Audit all IMAP FETCH commands in the sync code to verify they use PEEK variants
- Consider a one-time flag re-sync for affected folders: fetch the current `\Seen` status from the server for all cached messages and update the cache

**Test strategy:** Verify that the sync code uses BODY.PEEK[] in all fetch operations. Test that syncing does not change the \Seen flag on the server.

---

### ISSUE-007: Deduplicate Flag Values

**Priority:** Low
**Category:** Parsing

INBOX emails show `flags: ["Seen", "Seen"]` (duplicated). OnlineArchiveBackup emails show `flags: ["Recent"]` without duplication.

**Requirements:**
- Deduplicate the flags array during sync before storing in the database
- A simple set/distinct operation on the flags before inserting/updating would suffice
- Alternatively, deduplicate at query time when constructing the flags JSON array

**Test strategy:** Unit test that flags are never duplicated in the output.

---

### ISSUE-008: Populate In-Reply-To and References Headers During Sync

**Priority:** High
**Category:** Sync/IMAP

The email schema has `in_reply_to` and `references_header` columns, but all tested emails have these fields as `null`, even clear replies with "RE:" subjects. Without these headers, `get_email_thread` can only find copies of the same message, not full conversation chains.

**Requirements:**
- During IMAP sync, explicitly fetch the `In-Reply-To` and `References` headers from the email's MIME header section
- Store them in the corresponding database columns
- Update `get_email_thread` to traverse the `In-Reply-To` chain: given a message_id, find all messages that reference it (via `references_header` LIKE '%message_id%') AND the message that this one replies to (via `in_reply_to`)
- This enables true thread reconstruction across entire conversation chains

**Test strategy:** Unit test the header extraction logic. Test thread traversal with a known conversation chain.

---

## PRIORITY 3 — Feature Request

### ISSUE-009: Evidence Export Tool for Attachment Packaging

**Priority:** Medium
**Category:** Feature/Evidence
**Depends on:** ISSUE-002, ISSUE-008

Create an MCP tool that packages cached emails and their attachments into an organized evidence folder structure suitable for attorney review. Attachments are already saved to disk during sync (in `attachments/{account_id}/...`).

**Requirements:**
- New MCP tool `export_evidence` that accepts: account_id, folder (or search criteria), output_path
- Creates an organized output directory: `{output_path}/{account_id}/{date}/`
- Copies or symlinks all matching attachments into the output folder with clear filenames
- Generates a manifest file (CSV or Markdown) with columns: email_date, from, to/cc, subject, 3-5 line synopsis, attachment file paths
- The whole structure should be zippable and ready to hand to attorneys
- Uses existing `storage_path` from the attachment database to locate files on disk

**Test strategy:** Unit test manifest generation. Integration test with a small set of cached emails to verify the output structure.
