# RustyMail

[![CI](https://github.com/TexasFortress-AI/RustyMail/actions/workflows/ci.yml/badge.svg)](https://github.com/TexasFortress-AI/RustyMail/actions/workflows/ci.yml)
[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

An email assistant that runs entirely on your own computer. RustyMail connects to all your
mailboxes, sorts the daily flood into a few clear folders, finds the people waiting on you,
drafts replies, and lets AI assistants work with your mail through the
[Model Context Protocol](https://modelcontextprotocol.io) (MCP). Every AI model it uses runs
locally: your email is never sent to an AI company.

**Overview and recent progress:** [RustyMail status page](https://claude.ai/artifact/XiVcLdJces4PR61EuouPxy)

---

## What it does

- **Sorts your inbox.** `rustymail-triage` proposes a folder for each message: Accounting,
  Legal, Credit, Alerts, Noise, Spam, Needs Reply, plus Lists and Repos for mailing lists and
  code-host notifications. Nothing moves until you review the proposals and run `--apply`,
  and every move is verified on the server.
- **Finds the people waiting on you.** Needs Reply holds real requests from real people; cold
  pitches and mass "ghost job" mail go to Spam. Anyone you have written to, from any of your
  accounts, is a correspondent and is never filed as spam.
- **Is careful with money and paperwork.** A payment request, contract or alert from a sender
  you have never dealt with waits in the INBOX for you to verify it.
- **Works with the mail you have.** Gmail, Microsoft 365 (OAuth) and standard IMAP/SMTP
  servers, several accounts side by side. A local cache keeps search fast and keeps read/unread
  status in step with your other mail apps.
- **Plugs into AI tools.** 37 low-level and 21 high-level MCP tools, over stdio (Claude Desktop,
  Claude Code) or Streamable HTTP, and a web dashboard with a built-in assistant that can
  search, summarise and draft.

## Privacy and security

- **Local models only.** Cloud AI providers are refused in the code, and every model URL must
  be a loopback address (`src/dashboard/services/ai/local_only.rs`, `src/triage/model.rs`).
  See [Local models](docs/LOCAL_MODELS.md).
- **Prompt injection.** Anyone can send you an email, and an email can contain orders aimed at
  the AI. RustyMail's own AI (the dashboard assistant and the `process_email_instructions`
  agent) may only read, search and draft; sending, deleting, moving, flagging and renaming
  are shown to you with a **Confirm** button and run only when you click it. Tool results are
  fenced as data, and the models are told never to follow instructions inside them. Triage
  keeps any message that tries to instruct the sorter in the INBOX. The allowlist is
  `src/dashboard/services/ai/tool_policy.rs`; a new tool needs the owner until it is added there.
- **Dashboard and API access** need the API key (header) or a session cookie from signing in
  with it; the key can live in the macOS Keychain instead of a file. Stored mail-account
  passwords are encrypted when `ENCRYPTION_MASTER_KEY` is set.

---

## Quick start

### Prerequisites

- **Rust 1.89+** ([rustup](https://rustup.rs))
- **Node.js 18+** for the dashboard
- **PM2** (optional) for process management: `npm install -g pm2`
- A local model server for the AI features: [Ollama](https://ollama.com), optionally behind
  model_serve. See [Local models](docs/LOCAL_MODELS.md).

### Install

```bash
git clone https://github.com/TexasFortress-AI/RustyMail.git
cd RustyMail

cp .env.example .env
# Edit .env: ports, local model settings, triage accounts. For the API key see below.

cargo build --release
(cd frontend/rustymail-app-main && npm install && npm run build)
```

`cargo build --release` builds every binary:

| Binary | Purpose |
|---|---|
| `rustymail-server` | Backend: dashboard, dashboard API and MCP over HTTP (default port 9437) |
| `rustymail-mcp-stdio` | MCP stdio adapter, low-level tools (for Claude Desktop and similar clients) |
| `rustymail-mcp-stdio-high-level` | MCP stdio adapter, high-level tools |
| `rustymail-sync` | Mail sync into the local cache; the server starts it periodically |
| `rustymail-triage` | Inbox triage: propose, report, apply |

### API key

Every MCP client and the dashboard must present one shared API key. The server refuses to
start without one.

1. **Generate a key** (64 random hex characters):
   ```bash
   openssl rand -hex 32
   ```

2. **Store it.**
   - **macOS (recommended): in the Keychain**, so it's in no file at all. Leave
     `RUSTYMAIL_API_KEY` out of `.env`; when it's unset, the server reads the Keychain item
     `apikey:RUSTYMAIL_API_KEY` at startup. Paste the key at the hidden prompt:
     ```bash
     read -rs "?Paste key: " K; echo   # zsh; in bash: read -rsp "Paste key: " K; echo
     security add-generic-password -U -a "$USER" -s apikey:RUSTYMAIL_API_KEY \
       -l apikey:RUSTYMAIL_API_KEY -T /usr/bin/security -w "$K"
     echo "stored ${#K} chars"; unset K
     ```
     Don't use the bare `security add-generic-password ... -w` prompt: it silently cuts input
     off at 128 characters. Set `RUSTYMAIL_API_KEY_KEYCHAIN_SERVICE` to use a different item name.
   - **Other systems:** set `RUSTYMAIL_API_KEY=<key>` in `.env`. Keep `.env` out of git (it
     already is) and readable only by you (`chmod 600 .env`).

3. **Sign in to the dashboard.** Open http://127.0.0.1:9437/ and paste the key. The browser
   gets an HttpOnly session cookie and never stores the key. You sign in again after
   `DASHBOARD_SESSION_TTL_HOURS` (default 12), after a backend restart, or after signing out.
   To copy the key on macOS:
   ```bash
   security find-generic-password -a "$USER" -s apikey:RUSTYMAIL_API_KEY -w | pbcopy
   ```

4. **Give it to MCP clients.** On macOS, have the client read it from the Keychain when it
   starts the adapter (see [MCP clients](#mcp-clients)), so the key isn't written into the
   client's config file. Elsewhere, set `RUSTYMAIL_API_KEY` in the client's `env` block.

5. **Rotate it:** generate a new key, store it as in step 2 (`-U` updates the existing item),
   run `pm2 restart rustymail-backend`, sign in to the dashboard again, and restart your MCP
   clients.

### Run

```bash
# With PM2 (restarts on crashes; the backend also serves the built dashboard)
pm2 start frontend/rustymail-app-main/ecosystem.config.cjs
pm2 save          # keep the process list across reboots
pm2 logs          # follow the logs
pm2 restart rustymail-backend

# Or directly
./target/release/rustymail-server
```

Then open http://127.0.0.1:9437/ and sign in with the API key.

### Add your mail accounts

Use the dashboard's **Accounts** tab: it auto-discovers server settings for most providers and
handles Microsoft 365 sign-in (OAuth). Accounts are stored in `config/accounts.json`; see
`config/accounts.json.example` for the format if you prefer to write it by hand. Set
`ENCRYPTION_MASTER_KEY` in `.env` to store their passwords encrypted.

---

## Inbox triage

```bash
./target/release/rustymail-triage              # propose buckets for new INBOX mail
./target/release/rustymail-triage --report     # review the proposals (with dashboard links)
./target/release/rustymail-triage --apply      # move what you approved, verifying each move
```

Proposing never changes the mailbox. `--apply` moves only the buckets in
`TRIAGE_APPLY_BUCKETS` (Needs Reply stays in the INBOX unless you add it), only model proposals
at or above `TRIAGE_MIN_CONFIDENCE`, and nothing is deleted or expunged. Useful options:
`--account`, `--since-days`, `--limit`, `--dry-run`; run `--help` for all of them.

How a message is decided:

1. **Rules first:** failed sender authentication is Spam, expired one-time codes go to Trash,
   debt-collector notices are Credit, mailing lists and code-host notifications go to Lists and
   Repos, and your own sender rules apply (`TRIAGE_SENDER_RULES_FILE`, see
   `config/triage_senders.example.json`).
2. **A local decision model** classifies the rest from the sender, subject and a short
   excerpt; when it is unsure, a larger local model reads more of the message.
3. **Guards** overrule a model where facts it can't see say otherwise: a correspondent is never
   Spam or Noise, a trusted-looking document from a new sender waits for you, Needs Reply is
   only for unanswered, recent mail, and mail addressing the sorter stays put.

Settings are in the "Email triage" section of `.env.example`. Folder names follow each
server's `\Junk`, `\Trash`, `\Sent` and `\Drafts` markings; `TRIAGE_FOLDER_MAP_FILE` overrides
them per account. [Local models](docs/LOCAL_MODELS.md) explains which model does what.

---

## MCP clients

RustyMail offers two tool sets:

- **Low-level** (`rustymail-mcp-stdio`, 37 tools): direct access to folders, messages,
  flags, moves, attachments and sending.
- **High-level** (`rustymail-mcp-stdio-high-level`, 21 tools): fewer, broader tools for AI
  assistants, including drafting (`draft_reply`, `draft_email`) and the agent
  (`process_email_instructions`).

Both connect to the running backend. The same tools are served over Streamable HTTP at
`http://127.0.0.1:9437/mcp` (add `?variant=high-level` for the high-level set).

### Claude Desktop

Config file: `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS),
`%APPDATA%\Claude\claude_desktop_config.json` (Windows), `~/.config/Claude/claude_desktop_config.json` (Linux).

```json
{
  "mcpServers": {
    "rustymail": {
      "command": "/bin/sh",
      "args": [
        "-c",
        "RUSTYMAIL_API_KEY=\"$(/usr/bin/security find-generic-password -a \"$(id -un)\" -s apikey:RUSTYMAIL_API_KEY -w)\" exec /absolute/path/to/RustyMail/target/release/rustymail-mcp-stdio"
      ],
      "env": {
        "MCP_BACKEND_URL": "http://127.0.0.1:9437/mcp",
        "MCP_TIMEOUT": "30"
      }
    },
    "rustymail-high-level": {
      "command": "/bin/sh",
      "args": [
        "-c",
        "RUSTYMAIL_API_KEY=\"$(/usr/bin/security find-generic-password -a \"$(id -un)\" -s apikey:RUSTYMAIL_API_KEY -w)\" exec /absolute/path/to/RustyMail/target/release/rustymail-mcp-stdio-high-level"
      ],
      "env": {
        "MCP_BACKEND_URL": "http://127.0.0.1:9437/mcp",
        "MCP_TIMEOUT": "120"
      }
    }
  }
}
```

This reads the key from the macOS Keychain each time the adapter starts, so the config file
holds no secret. On other systems, set `"command"` to the binary's path, drop `args`, and add
`"RUSTYMAIL_API_KEY": "<your key>"` to `env`. Use a 120-second timeout for the high-level set:
drafting and the agent wait on a local model.

Every tool takes a required `account_id`: the email address of the account to act on. There is
no "current account"; `list_accounts` lists them.

### Tools

In the last column, **model may run** means RustyMail's own AI (the dashboard assistant and the
agent) can call the tool on its own; **owner only** means it can only propose it, for you to
confirm. An external MCP client such as Claude Desktop can call any tool, subject to its own
approval prompts, so don't set it to approve sends and deletes automatically.

<details>
<summary><strong>Low-level tools (37)</strong></summary>

| Tool | What it does | RustyMail's own AI |
|---|---|---|
| `list_accounts` | List the configured email accounts | model may run |
| `list_folders` | List an account's folders | model may run |
| `list_folders_hierarchical` | List folders as a tree | model may run |
| `create_folder` | Create a folder | owner only |
| `rename_folder` | Rename a folder | owner only |
| `delete_folder` | Delete a folder | owner only |
| `sync_emails` | Sync a folder (or all) into the local cache | owner only |
| `list_cached_emails` | List cached messages in a folder | model may run |
| `get_email_by_uid` | Get a cached message by UID | model may run |
| `get_email_by_index` | Get a cached message by position | model may run |
| `fetch_emails_with_mime` | Fetch a message with its MIME parts | model may run |
| `count_emails_in_folder` | Count a folder's messages | model may run |
| `get_folder_stats` | Folder totals, read and unread | model may run |
| `search_cached_emails` | Search cached messages | model may run |
| `search_by_domain` | Find mail by sender or recipient domain | model may run |
| `search_by_attachment_type` | Find attachments by MIME type | model may run |
| `filter_emails_by_subject` | Filter by subject patterns, sender, recipient, date | model may run |
| `list_emails_by_flag` | Filter by flags (unread, flagged, answered...) | model may run |
| `get_address_report` | Top senders and domains for an account | model may run |
| `get_email_synopsis` | A short synopsis of a message | model may run |
| `batch_get_synopsis` | Synopses of several messages in one call | model may run |
| `get_email_thread` | Every message in a conversation | model may run |
| `list_email_attachments` | List a message's attachments | model may run |
| `get_attachment_content` | One attachment's content (base64) | model may run |
| `download_email_attachments` | Save a message's attachments to disk | owner only |
| `cleanup_attachments` | Delete saved attachments | owner only |
| `export_folder_metadata` | Export a folder's metadata to a temp file | model may run |
| `export_evidence` | Export messages and attachments to an evidence folder | owner only |
| `mark_as_read` | Mark messages read | owner only |
| `mark_as_unread` | Mark messages unread | owner only |
| `atomic_move_message` | Move one message, verified | owner only |
| `atomic_batch_move` | Move several messages, verified | owner only |
| `mark_as_deleted` | Flag messages deleted | owner only |
| `undelete_messages` | Remove the deleted flag | owner only |
| `delete_messages` | Delete messages (only those named) | owner only |
| `expunge` | Expunge a folder's deleted messages | owner only |
| `send_email` | Send mail by SMTP | owner only |

</details>

<details>
<summary><strong>High-level tools (21)</strong></summary>

| Tool | What it does | RustyMail's own AI |
|---|---|---|
| `process_email_instructions` | The agent: answers questions and prepares work from plain language; reads, searches and drafts only | model may run |
| `draft_reply` | Draft a reply to a message (saved to Drafts, never sent) | model may run |
| `draft_email` | Draft a new message (saved to Drafts, never sent) | model may run |
| `list_accounts`, `list_folders_hierarchical`, `list_cached_emails`, `get_email_by_uid`, `search_cached_emails`, `get_folder_stats`, `get_email_synopsis`, `get_email_thread`, `search_by_domain`, `list_emails_by_flag`, `get_address_report` | As in the low-level set | model may run |
| `sync_emails` | Sync into the local cache | owner only |
| `get_model_configurations` | Show the agent and drafting models | model may run |
| `set_tool_calling_model`, `set_drafting_model` | Choose the agent and drafting models (local only) | owner only |
| `list_jobs`, `get_job_status` | Background agent jobs | model may run |
| `cancel_job` | Cancel a background job | owner only |

</details>

The agent runs as a background job: `process_email_instructions` returns a `jobId`; poll
`get_job_status` for its answer.

---

## Dashboard

The backend serves the dashboard at http://127.0.0.1:9437/ (`DASHBOARD_PATH`). It has email
browsing and search across accounts, a compose window, account setup, background jobs, model
settings, an MCP tool panel, and the assistant. When the assistant proposes something only you
can do, the proposal appears under its reply with the exact action and a **Confirm** button.

Frontend development:

```bash
cd frontend/rustymail-app-main
npm install
npm run dev     # Vite dev server with hot reload (DASHBOARD_PORT, default 9439), proxies /api
npm run build   # build for the backend to serve; reload the page to pick it up
```

The frontend reads the project root `.env` through `dotenv-cli`; it has no `.env` of its own.

---

## Configuration

All settings are environment variables in `.env`; `.env.example` lists and explains each one.
The main groups:

| Group | Variables |
|---|---|
| Ports | `REST_PORT` (default 9437), `SSE_PORT`, `DASHBOARD_PORT` |
| Database | `CACHE_DATABASE_URL` (default `sqlite:data/email_cache.db`); migrations run at startup |
| Access | `RUSTYMAIL_API_KEY` (or the Keychain), `DASHBOARD_SESSION_TTL_HOURS`, `ENCRYPTION_MASTER_KEY` |
| Local models | `MODEL_SERVE_BASE_URL`, `MODEL_SERVE_MODEL`, or `OLLAMA_BASE_URL`, `OLLAMA_MODEL` |
| Triage | `TRIAGE_ACCOUNTS`, `TRIAGE_DECISION_*`, `TRIAGE_MODEL_*`, `TRIAGE_MIN_CONFIDENCE`, `TRIAGE_APPLY_BUCKETS` and more |
| Sync | `CACHE_SYNC_INTERVAL_SECONDS` |
| Dashboard | `DASHBOARD_PATH` (the built frontend the backend serves) |

The legacy REST API under `/api/v1` ([reference](docs/REST-API.md)) is off unless
`REST_API_V1_ENABLED=true`. The dashboard API (`/api/dashboard/...`) and MCP are always on and
need the API key.

---

## Architecture

```
  Claude Desktop / Claude Code         Web dashboard (browser)
          │ MCP stdio                         │ HTTP + session cookie
          ▼                                   ▼
  rustymail-mcp-stdio(-high-level) ───► rustymail-server ◄────► IMAP / SMTP servers
                                        port 9437: dashboard, /api/dashboard, /mcp
                                              │
                                              ▼
                           SQLite cache (data/email_cache.db) + in-memory LRU
                                              ▲
                           rustymail-sync (periodic: new mail, deletions, flags)

  rustymail-triage ──► local models (Ollama / model_serve, loopback only) ◄── dashboard AI
```

- **Cache.** Reads come from SQLite (with an in-memory LRU on top), so the mail server is
  touched only for sync and changes. Sync fetches new mail, drops mail deleted elsewhere, and
  refreshes flags; with CONDSTORE it asks only for what changed.
- **Accounts** live in `config/accounts.json`, mirrored into the database.
- **Moves are verified:** a move reports success only once the message is found in the
  destination. Deletes expunge only the messages named.

---

## Development

```bash
cargo test                  # unit and integration tests (tests/unit, tests/integration)
cargo clippy --all-targets  # lint
cargo fmt --all             # format
```

Tests that start a live server or touch real mail are marked `#[ignore]` and run only with
`-- --ignored`. CI runs formatting, clippy, the dependency audit and the full test suite on
every pull request. When you add an MCP tool, classify it in
`src/dashboard/services/ai/tool_policy.rs` (a test fails until you do), and see `CLAUDE.md`
for the project's conventions.

More documentation:

- [Local models](docs/LOCAL_MODELS.md): which model does what, Ollama or model_serve
- [MCP account tools](docs/ACCOUNT_TOOLS_QUICKSTART.md)
- [Legacy REST API](docs/REST-API.md) and [examples](docs/REST-EXAMPLES.md)
- [Profiling](docs/PROFILING.md)

## License

[Mozilla Public License 2.0](LICENSE)
