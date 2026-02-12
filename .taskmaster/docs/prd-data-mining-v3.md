# RustyMail MCP Capability Assessment
## Email Data Mining Requirements for jobs@mleehealthcare.ai

**Date:** February 12, 2026
**Assessor:** Claude (via MCP tool testing)
**Account:** jobs@mleehealthcare.ai (19,669 emails cached in INBOX)
**Revision:** v3 — Updated after OAuth re-authentication and full re-test

---

## Executive Summary

The RustyMail MCP server is **running and operational**. After re-authenticating the OAuth token for jobs@mleehealthcare.ai, live IMAP access is restored and folder listing works. However, testing reveals several genuine bugs and feature gaps that prevent the requested data mining workflows. Notably, only the INBOX has been cached — all subfolders are empty in the cache despite being visible via IMAP.

---

## UI Bug: OAuth Redirect Page

**Issue:** After clicking "Authorize Microsoft 365" in the RustyMail UI (which succeeds), the browser redirects to a raw JSON page:
```json
{"success":true,"email":"jobs@mleehealthcare.ai","message":"OAuth authorization successful. Account linked."}
```
The user is no longer in the RustyMail interface and must manually navigate back.

**Expected behavior:** After successful OAuth, redirect the user back to the RustyMail UI (e.g., the account settings page) with a success toast/notification.

**Priority:** Low — cosmetic/UX, but looks unprofessional.

---

## RESOLVED: OAuth Token for jobs@mleehealthcare.ai

**Previously:** XOAUTH2 login failed.
**Status:** ✅ Re-authenticated successfully. IMAP test connection confirmed working.

---

## Requirement 1: Timesheet Emails with Attachments (Images, PDFs)

**Current Capability: ❌ NOT SUPPORTED (genuine tool gap)**

**What works:**
- `search_cached_emails` finds emails with "timesheet" in the subject and returns full body text.

**Confirmed issues (not connection-related):**
- The `has_attachments` field is **unreliable and inconsistent**:
  - Emails whose body text explicitly says "Please see attached Taju's signed timesheet" return `has_attachments: false`.
  - The same email (UID 679142) returned `has_attachments: true` from `list_cached_emails` but `has_attachments: false` from `get_email_by_uid`. The flag is inconsistent across endpoints.
- **No attachment metadata** — no filenames, MIME types, file sizes, or attachment count on any endpoint.
- **No filter by attachment type** (e.g., "emails with .jpg, .png, .pdf attachments").
- **No ability to download or access attachment content.**

**Tools Needed:**
1. `get_attachment_metadata(uid, folder)` → List of attachments per email: filename, MIME type, size, attachment ID.
2. `search_emails_by_attachment(mime_types[], folder)` → Filter emails by attachment type (e.g., `image/*`, `application/pdf`).
3. `download_attachment(uid, attachment_id, folder)` → Retrieve attachment content (base64 or file path).
4. **Bug fix:** Make `has_attachments` consistent and accurate across all endpoints.

---

## Requirement 2: Resume Emails with Attachments (Word, PDF, TXT, RTF)

**Current Capability: ❌ NOT SUPPORTED (genuine tool gap)**

**What works:**
- Subject-line search for "resume" returns results. There is also a dedicated `INBOX/resumes` subfolder (see Requirement 6).

**Confirmed issues (not connection-related):**
- Same attachment problems as Requirement 1 — no metadata, unreliable flag, no download.
- Resume emails return `has_attachments: false` even when body says "I have attached the resume."
- Cannot distinguish resume-with-attachment from forwarded resume discussion.
- The `INBOX/resumes` subfolder exists but **has not been cached** — contains 0 emails in the cache.

**Tools Needed:**
- Same as Requirement 1 (attachment metadata, type-based filtering, download).
- **Sync `INBOX/resumes` subfolder** to cache (see Requirement 6).

---

## Requirement 3: Email Threads / Conversations

**Current Capability: ⚠️ PARTIALLY SUPPORTED (genuine tool gap)**

**What works:**
- Emails include `message_id` field (standard IMAP Message-ID).
- Body HTML/text contains forwarded/replied message headers inline, parseable for thread reconstruction.
- Subject "RE:" / "FW:" prefixes allow basic grouping.

**Confirmed issues (not connection-related):**
- **No `In-Reply-To` or `References` headers exposed.** These are the standard IMAP headers for proper thread linking.
- **No thread ID or conversation ID.**
- **No tool to retrieve a full thread** by message ID or conversation.

**Tools Needed:**
1. Expose `In-Reply-To` and `References` headers in email metadata.
2. `get_email_thread(message_id)` → All emails in the same thread, chronologically.
3. `search_threads(query)` → Grouped thread results.
4. Computed `thread_id` or `conversation_id` field on cached emails.

---

## Requirement 4: Email Domains and Addresses Involved

**Current Capability: ⚠️ PARTIALLY SUPPORTED (genuine tool gap)**

**What works:**
- Each email returns `from_address`, `from_name`, `to_addresses[]`, `cc_addresses[]`.
- `search_cached_emails` supports `from:user@example.com` syntax.

**Confirmed issues (not connection-related):**
- **No bulk aggregation.** Reporting on all unique domains/addresses requires paging through 19,669 emails in batches of 50 (394 API calls).
- **No domain-based search** (e.g., "all from @rochesterregional.org").
- Embedded addresses in forwarded body text are not indexed.

**Tools Needed:**
1. `get_email_address_report(folder)` → Aggregated unique addresses/domains with counts.
2. `search_by_domain(domain, folder)` → Filter by sender/recipient domain.
3. Bulk export for address data.

---

## Requirement 5: Unread Email Synopsis

**Current Capability: ❌ NOT SUPPORTED (confirmed genuine bug + tool gap)**

**Confirmed after OAuth refresh:**
- `get_folder_stats` **still reports 0 unread** even with a live IMAP connection. The web UI shows all emails as unread. This is **not a stale cache issue** — it's a genuine bug in how flags are read or counted.
- The `flags` field on individual emails still shows duplicate values: `["Seen", "Seen"]`.

**Confirmed issues:**
- **Read/unread count is wrong** — persists after re-authentication. (Genuine bug.)
- **No filter for unread emails** — no `unread_only` or flag-based filter parameter. (Tool gap.)
- **No built-in summarization** for generating a 3-5 line synopsis. (Tool gap.)

**Tools Needed:**
1. **Bug fix: Read/unread flag sync.** Investigate why all emails report as "Seen" when the mail server has them as unseen. Likely the IMAP fetch is setting `\Seen` instead of using `BODY.PEEK[]`, or the flags aren't being read from the server at all.
2. **Bug fix: Deduplicate flag values** (`["Seen", "Seen"]` → `["Seen"]`).
3. `list_emails_by_flag(flag, folder)` → Filter by Seen/Unseen/Flagged.
4. `get_email_synopsis(uid, max_lines)` → AI-generated summary (or expose enough for external summarization).

---

## Requirement 6: Subfolder Structure

**Current Capability: ⚠️ PARTIALLY WORKING (tool works, but cache is INBOX-only)**

**Confirmed working after OAuth refresh:**
`list_folders_hierarchical` returns the full folder tree (50+ folders):

**Top-level folders:** Archive, Bckup23, Calendar, Clutter, Contacts, Deleted Items, Drafts, INBOX, Journal, Junk Email, Notes, Outbox, Sent Items, Tasks, and more.

**Key INBOX subfolders:**
- `INBOX/Accounting AMc`
- `INBOX/Contracts`
- `INBOX/Einstein`
- `INBOX/HWLworks`
- `INBOX/Jobs @MLee`
- `INBOX/LinkedIn`
- `INBOX/noreply@mleeh..`
- `INBOX/resumes` (with nested `INBOX/resumes/Imported`)

**Issue: Only INBOX is cached.**
All subfolders return empty results from `list_cached_emails` and "Folder not found" from `get_folder_stats`. The sync/cache process has only ever ingested the INBOX — no subfolders, no Sent Items, no Deleted Items.

**Tools Needed:**
1. **Multi-folder sync** — ability to sync/cache subfolders, not just INBOX. Either a "sync all folders" option or a `sync_folder(folder_name)` tool.
2. Alternatively, a configurable sync list in the RustyMail admin UI specifying which folders to cache.

---

## Summary Matrix

| Requirement | Status | Root Cause | Fix Type |
|---|---|---|---|
| Timesheet attachments | ❌ Not supported | Genuine tool gap | Feature development |
| Resume attachments | ❌ Not supported | Genuine tool gap | Feature development |
| Email threads | ⚠️ Partial | Genuine tool gap | Feature development |
| Domains/addresses report | ⚠️ Partial | Genuine tool gap | Feature development |
| Unread synopsis | ❌ Not supported | Genuine bug + tool gap | Bug fix + feature development |
| Subfolder listing | ⚠️ Partial | Cache only covers INBOX | Sync enhancement |

---

## Bug Reports

### BUG-001: OAuth Redirect Shows Raw JSON
- **Severity:** Low (UX)
- **Description:** After successful Microsoft 365 OAuth, user is redirected to a raw JSON response page instead of back to the RustyMail UI.
- **Expected:** Redirect to RustyMail account settings with a success notification.

### BUG-002: `has_attachments` Flag Inconsistency
- **Severity:** Medium
- **Description:** Same email returns different `has_attachments` values from `list_cached_emails` (true) vs. `get_email_by_uid` (false). Also returns `false` for emails that clearly have attachments per body text.
- **Likely cause:** MIME part parsing inconsistency or inline vs. true attachment distinction not handled.

### BUG-003: Read/Unread Count Always Zero
- **Severity:** High
- **Description:** `get_folder_stats` reports `unread: 0` for INBOX even though web UI shows all 19,669 emails as unread. Persists after OAuth re-authentication and confirmed live IMAP connection.
- **Likely cause:** IMAP FETCH using `BODY[]` instead of `BODY.PEEK[]` (marking messages as read on fetch), or `\Seen` flags not being read from server during sync.

### BUG-004: Duplicate Flag Values
- **Severity:** Low
- **Description:** Email `flags` array contains duplicates, e.g., `["Seen", "Seen"]` instead of `["Seen"]`.
- **Likely cause:** Flag parsing not deduplicating, or multiple sync passes appending flags.

### BUG-005: Subfolder Cache Not Populated
- **Severity:** High
- **Description:** Only INBOX emails are cached. All subfolders (including important ones like `INBOX/resumes`, `Sent Items`, `INBOX/Contracts`) return empty results from cached queries and "Folder not found" from stats.
- **Likely cause:** Sync process hardcoded to INBOX only.

---

## Recommended Priority Order

### Bug Fixes (high priority)
1. **BUG-003:** Fix read/unread flag sync — foundational for unread workflows.
2. **BUG-005:** Enable multi-folder sync — unlocks subfolder data (resumes, contracts, sent items).
3. **BUG-002:** Fix `has_attachments` consistency — prerequisite for attachment features.
4. **BUG-004:** Deduplicate flags.
5. **BUG-001:** OAuth redirect UX.

### Feature Development (by priority)
6. **Attachment metadata** on email responses — parse MIME parts during sync; store filename, MIME type, size.
7. **Flag-based email filtering** (`unread_only`, filter by IMAP flags).
8. **Attachment-type filtering** — find emails by attachment MIME type.
9. **Expose `In-Reply-To` / `References` headers + thread grouping.**
10. **Domain-based search and address aggregation** for reporting.
11. **Attachment download** — retrieve attachment content.
12. **AI synopsis generation** — can be done externally in the interim using body text + Claude API.
