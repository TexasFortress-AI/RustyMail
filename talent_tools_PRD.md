# Rustymail — New Tool Specifications
**Requested by:** AI Agent / Claude Integration Team  
**Purpose:** Reduce context window consumption during large-scale email processing workflows  
**Background:** During a bulk email extraction task (1,009 sent emails → Excel spreadsheet), the AI agent's context window filled rapidly because every tool result — including full HTML email bodies, repeated pagination output, and raw metadata — was returned inline into context. These three tools address the root causes by pushing data to disk, enabling server-side filtering, and supporting multi-UID synopsis batching.

---

## Tool 1: `export_folder_metadata`

### Summary
Export lightweight metadata (no body content) for all emails in a folder directly to a file on disk, returning only the file path to the agent. This is the highest-priority tool — it enables the agent to reference a full folder inventory without ever loading it into context.

### Motivation
Currently, enumerating a 1,009-email folder requires ~21 paginated `list_cached_emails` calls, each returning full body text and HTML fragments. This consumes enormous context. The agent needs a way to say "give me a map of this folder" and get back a file path, not a wall of data.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `account` | string | Yes | Email account identifier (e.g. `jobs@mleehealthcare.ai`) |
| `folder` | string | Yes | Folder path to export (e.g. `Sent Items`) |
| `output_path` | string | No | Destination file path on disk. Defaults to a system temp path if omitted. |
| `format` | enum | No | Output format: `json` (default) or `csv` |
| `fields` | array of strings | No | Which fields to include. Defaults to all available metadata fields. |

### Available Metadata Fields
The following fields may be requested via the `fields` parameter. All are lightweight — **no body content of any kind is included.**

- `uid` — Unique message identifier
- `subject` — Email subject line
- `from_address` — Sender address
- `to_addresses` — List of recipient addresses
- `cc_addresses` — List of CC addresses
- `date` — Date/time sent or received (ISO 8601)
- `has_attachments` — Boolean
- `attachment_names` — List of attachment filenames (no content)
- `flags` — IMAP flags (e.g. `Seen`, `Flagged`)
- `size_bytes` — Approximate message size
- `message_id` — RFC 2822 Message-ID header (for threading)
- `in_reply_to` — Parent message ID (for threading)

### Returns
```json
{
  "status": "success",
  "account": "jobs@mleehealthcare.ai",
  "folder": "Sent Items",
  "total_exported": 1009,
  "output_path": "/tmp/rustymail_exports/sent_items_metadata.json",
  "format": "json",
  "fields_included": ["uid", "subject", "from_address", "to_addresses", "date", "has_attachments"]
}
```

### Notes
- The agent can then pass this file path to `bash_tool` or a Python script to filter, sort, and analyze the metadata entirely outside context.
- If `output_path` is specified by the agent, it should be somewhere the agent can read from (e.g. `/home/claude/` or `/tmp/`).
- Large folders (5,000+ emails) should stream to disk rather than buffer in memory.
- The tool should **never** return the file contents inline — only the path. This is the entire point.

---

## Tool 2: `batch_get_synopsis`

### Summary
Accept a list of UIDs and return compact, one-paragraph synopses for each — all in a single tool call. Unlike the existing `get_email_synopsis` (single UID only), this tool processes a batch, dramatically reducing round-trips and keeping individual results small.

### Motivation
Once the agent has identified candidate UIDs from the metadata file, it needs a quick way to triage them before committing to full body fetches. Currently, fetching even a synopsis requires one tool call per email. For 50–100 candidate UIDs, that's 50–100 context-filling round trips. A batch call collapses this into one.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `account` | string | Yes | Email account identifier |
| `folder` | string | Yes | Folder the UIDs belong to |
| `uids` | array of strings/integers | Yes | List of UIDs to retrieve synopses for. Max 50 per call. |
| `max_chars_per_synopsis` | integer | No | Hard cap on characters per synopsis. Default: 300. Max: 800. |

### Returns
```json
{
  "account": "jobs@mleehealthcare.ai",
  "folder": "Sent Items",
  "requested": 20,
  "returned": 20,
  "synopses": [
    {
      "uid": "7412",
      "subject": "FW: Wayne Holland for Histo Tech Position",
      "from_address": "mason@mleehealthcare.ai",
      "to_addresses": ["Aaron.Lieu@northside.com"],
      "date": "2024-03-14T10:22:00Z",
      "has_attachments": true,
      "synopsis": "Forwarded candidate submittal for Wayne Holland applying for a Histo Tech position at Northside. Includes phone (724-766-9542) and email (pathtechilc@hotmail.com). 44+ years of experience noted. Resume attached as PDF."
    },
    {
      "uid": "7389",
      "subject": "MLee Healthcare Candidate Submittal - Lucia Gaona-Susino",
      "from_address": "mason@mleehealthcare.ai",
      "to_addresses": ["Aaron.Lieu@northside.com"],
      "date": "2024-03-12T14:05:00Z",
      "has_attachments": true,
      "synopsis": "Candidate submittal for Lucia Gaona-Susino, Histology Technologist with 20+ years of experience. Contact: 917-446-1526, pili0568@yahoo.com. Resume attached."
    }
  ],
  "errors": []
}
```

### Notes
- Synopses should be AI-generated summaries of the first ~500 characters of body text after stripping HTML. They must never include full quoted reply chains.
- If a UID is not found or is unreadable, include it in the `errors` array with a reason — do not fail the entire batch.
- The 50-UID max per call is a suggested limit to keep response sizes predictable. The team may tune this based on average email size.
- Return order should match the input UID order for easy mapping.

---

## Tool 3: `filter_emails_by_subject`

### Summary
Perform a server-side subject-line filter across an entire folder using one or more keyword patterns, returning only matching UIDs and subjects — no body content. This replaces the need to paginate through an entire folder just to triage which emails are relevant.

### Motivation
The current `search_cached_emails` tool does full-text search but returns limited results and includes body content in responses, making it expensive. The agent needs a cheap, high-recall way to ask: *"Which of these 1,009 emails have subjects containing 'resume', 'submittal', 'candidate', or a person's name?"* — and get back a flat list of UIDs it can then act on.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `account` | string | Yes | Email account identifier |
| `folder` | string | Yes | Folder to search within |
| `subject_patterns` | array of strings | Yes | Keywords or substrings to match against subject lines. Case-insensitive. At least one required. |
| `match_mode` | enum | No | `any` (default) — return emails matching any pattern. `all` — must match all patterns. |
| `sender_filter` | string | No | Optional: restrict results to a specific sender address or domain. |
| `recipient_filter` | string | No | Optional: restrict results to emails sent to a specific address or domain. |
| `date_after` | string | No | ISO 8601 date. Only return emails on or after this date. |
| `date_before` | string | No | ISO 8601 date. Only return emails on or before this date. |
| `max_results` | integer | No | Cap on results returned. Default: 500. Use 0 for no limit. |

### Returns
```json
{
  "account": "jobs@mleehealthcare.ai",
  "folder": "Sent Items",
  "patterns_used": ["resume", "submittal", "candidate", "fw:", "applicant"],
  "match_mode": "any",
  "total_matched": 87,
  "results": [
    {
      "uid": "7412",
      "subject": "FW: Wayne Holland for Histo Tech Position",
      "from_address": "mason@mleehealthcare.ai",
      "to_addresses": ["Aaron.Lieu@northside.com"],
      "date": "2024-03-14T10:22:00Z",
      "has_attachments": true,
      "matched_patterns": ["fw:"]
    },
    {
      "uid": "7344",
      "subject": "MLee Healthcare Candidate Submittal - Lucia Gaona-Susino",
      "from_address": "mason@mleehealthcare.ai",
      "to_addresses": ["Aaron.Lieu@northside.com"],
      "date": "2024-03-12T14:05:00Z",
      "has_attachments": true,
      "matched_patterns": ["submittal", "candidate"]
    }
  ]
}
```

### Notes
- Matching is against the subject line only — not body content. This keeps it fast and the response small.
- `matched_patterns` in each result shows which of the input patterns triggered the match. Useful for the agent to understand why an email was included.
- This is intentionally distinct from the existing `search_cached_emails` — that tool does full-text search and returns body content. This tool is metadata-only and designed for triage.
- Results should be sorted by date descending by default (newest first).

---

## Suggested Implementation Order

| Priority | Tool | Reason |
|----------|------|--------|
| 1 | `export_folder_metadata` | Eliminates the biggest source of context bloat — the metadata pagination loop |
| 2 | `filter_emails_by_subject` | Enables fast triage without any body content loading |
| 3 | `batch_get_synopsis` | Reduces round-trips during the candidate confirmation pass |

---

## How These Three Tools Work Together

The optimized workflow with all three tools:

```
1. export_folder_metadata("Sent Items") 
      → writes 1,009-row JSON to disk, returns file path only
      → agent hands path to bash/Python: filter, count, spot-check
      [context cost: ~1 small JSON object]

2. filter_emails_by_subject(patterns=["resume","submittal","candidate","applicant","fw:"])
      → returns list of 87 matching UIDs + subjects only
      [context cost: ~87 short lines]

3. batch_get_synopsis(uids=[...87 UIDs in batches of 50...])
      → 2 calls return compact paragraph synopses for all 87 emails
      [context cost: ~87 × 300 chars = ~26k chars]

4. get_email_by_uid() — called only for confirmed candidates (maybe 30–40)
      → full body fetch, targeted
      [context cost: manageable, intentional]

5. Build Excel from extracted data.
```

Compare to the previous session's approach, which loaded full bodies for ~200+ emails across 21+ pagination calls before the context collapsed. These tools reduce that to **5 targeted calls** before any full-body fetching begins.

---

*Spec authored: 2026-03-03*  
*For questions, route to the AI integration team.*