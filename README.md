# RustyMail

![RustyMail Architecture Diagram](docs/images/image%20(12).jpg)

[![Rust](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml/badge.svg)](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/rustymail)](https://crates.io/crates/rustymail)
[![Documentation](https://docs.rs/rustymail/badge.svg)](https://docs.rs/rustymail)

A high-performance, type-safe IMAP email client and API server written in Rust, with integrated web dashboard and Model Context Protocol (MCP) support.

---

## Features

- 🚀 **High Performance**: Built with Rust for speed and efficiency
- 🔒 **Type Safety**: Leverages Rust's type system for reliability
- 📧 **Full IMAP Support**: Access all folders and messages with caching
- 🔄 **Multiple Interfaces**: REST API, Web Dashboard, MCP Stdio, MCP HTTP
- 👥 **Multi-Account Support**: Manage multiple email accounts with file-based configuration
- 💾 **Smart Caching**: Two-tier cache (SQLite + in-memory LRU) for fast email access
- 🎨 **Modern Web UI**: React-based dashboard with real-time updates
- 🤖 **AI Integration**: Built-in chatbot with support for 10+ AI providers
- 🔐 **Secure**: TLS support and API key authentication
- 📊 **Monitoring**: Real-time metrics via SSE
- 🧪 **Comprehensive Testing**: Unit, integration, and E2E tests
- 🧩 **Extensible**: Easily add new MCP tools
- 🛠️ **MCP Protocol**: Full JSON-RPC 2.0 support over stdio and HTTP transports
- ⚡ **Process Management**: PM2 integration for reliable service management

---

## Quick Start

### Prerequisites

- **Rust 1.70+** - [Install from rust-lang.org](https://rust-lang.org)
- **Node.js 18+** (for dashboard) - [Install from nodejs.org](https://nodejs.org)
- **PM2** (optional, for process management) - `npm install -g pm2`
- **OpenSSL** or compatible SSL library
- An IMAP email account (Gmail, Outlook, etc.)

### Installation

```bash
git clone https://github.com/rangersdo/rustymail.git
cd rustymail

# Copy and configure environment variables
cp .env.example .env
# Edit .env with your configuration (ports, API keys, etc.)

# Build all components
cargo build --release --bin rustymail-server
cargo build --release --bin rustymail-mcp-stdio

# Build frontend dashboard
cd frontend/rustymail-app-main
npm install
npm run build
cd ../..
```

### Running with PM2 (Recommended)

PM2 provides reliable process management with auto-restart on crashes:

```bash
# Start all services (backend + frontend)
pm2 start ecosystem.config.js

# Save process list (survives reboots)
pm2 save

# View status
pm2 status

# View logs
pm2 logs

# Restart services
pm2 restart all

# Stop services
pm2 stop all
```

### Running Manually

- **Backend Server** (REST API + MCP HTTP, port 9437):
  ```bash
  ./target/release/rustymail-server
  ```

- **Frontend Dashboard** (port 9439):
  ```bash
  cd frontend/rustymail-app-main
  npm run dev
  ```

- **MCP Stdio Adapter** (for Claude Desktop integration):
  ```bash
  ./target/release/rustymail-mcp-stdio
  ```

### Quick Rebuild Command

Use the Claude Code slash command for complete rebuild:

```bash
/rebuild-all
```

This command stops services, rebuilds all components, and restarts with PM2.

---

## Interface Usage

### REST API

Server runs at `http://localhost:9437`

Example requests:

```bash
# List folders
curl -X GET http://localhost:9437/folders -H "Authorization: Basic $(echo -n 'user:pass' | base64)"

# List emails in INBOX
curl -X GET http://localhost:9437/emails/INBOX -H "Authorization: Basic $(echo -n 'user:pass' | base64)"
```

### MCP Stdio

```bash
cargo run --release -- --mcp-stdio
```

Send JSON-RPC requests via stdin:

```json
{"jsonrpc":"2.0","id":1,"method":"imap/listFolders","params":{}}
```

### MCP SSE

Start server:

```bash
cargo run --release -- --mcp-sse
```

Note: MCP Streamable HTTP transport is available on the same port as REST (9437)

**Main Endpoint:** `http://localhost:9437/mcp`
**Versioned Endpoint:** `http://localhost:9437/mcp/v1`

Connect via SSE (GET):

```bash
curl -N -H "Accept: text/event-stream" http://localhost:9437/mcp
```

Send JSON-RPC Requests (POST):

```bash
curl -X POST http://localhost:9437/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/list",
    "params": {}
  }'
```

---

## MCP Protocol Specification

RustyMail implements the Model Context Protocol (MCP) over stdio and Streamable HTTP transport (MCP 2025-03-26).

### JSON-RPC 2.0 Format

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": "unique-id",
  "method": "imap/listFolders",
  "params": {}
}
```

**Success Response:**

```json
{
  "jsonrpc": "2.0",
  "id": "unique-id",
  "result": { "folders": ["INBOX", "Sent"] }
}
```

**Error Response:**

```json
{
  "jsonrpc": "2.0",
  "id": "unique-id",
  "error": { "code": -32001, "message": "IMAP authentication failed" }
}
```

### Error Codes

- `-32700` Parse error
- `-32600` Invalid request
- `-32601` Method not found
- `-32602` Invalid params
- `-32603` Internal error
- `-32000` IMAP connection error
- `-32001` Authentication failure
- `-32002` Folder not found
- `-32003` Folder already exists
- `-32004` Email not found
- `-32010` IMAP operation failed

### Supported MCP Tools

#### Account Management
- `list_accounts` - List all configured email accounts
- `set_current_account` - Set the active account for operations

#### Folder Operations
- `list_folders` - List all folders for current account
- `list_folders_hierarchical` - Get folder tree structure

#### Email Operations (IMAP)
- `search_emails` - Search emails with IMAP queries
- `fetch_emails_with_mime` - Fetch emails with full MIME content
- `atomic_move_message` - Move single message atomically
- `atomic_batch_move` - Batch move multiple messages
- `mark_as_deleted` - Mark emails for deletion
- `delete_messages` - Permanently delete messages
- `undelete_messages` - Restore deleted messages
- `expunge` - Permanently remove deleted messages

#### Email Operations (Cache)
- `list_cached_emails` - List emails from local cache (fast)
- `get_email_by_uid` - Get specific email by UID
- `get_email_by_index` - Get email by index in folder
- `count_emails_in_folder` - Get email count for folder
- `get_folder_stats` - Get folder statistics (total, unread, size)
- `search_cached_emails` - Search cached emails (fast, local)

**Note:** Cache operations are significantly faster as they query the local SQLite database instead of the remote IMAP server.

---

## Claude Desktop Integration

RustyMail can be integrated with Claude Desktop as an MCP server, allowing Claude to access your emails directly.

### Setup

1. **Build the MCP stdio adapter** (if not already built):
   ```bash
   cargo build --release --bin rustymail-mcp-stdio
   ```

2. **Ensure backend server is running**:
   ```bash
   # Using PM2 (recommended)
   pm2 start ecosystem.config.js

   # Or manually
   ./target/release/rustymail-server
   ```

3. **Configure Claude Desktop** by editing your config file:
   - **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
   - **Linux**: `~/.config/Claude/claude_desktop_config.json`

4. **Add RustyMail MCP server** to the config:
   ```json
   {
     "mcpServers": {
       "rustymail": {
         "command": "/absolute/path/to/RustyMail/target/release/rustymail-mcp-stdio",
         "env": {
           "MCP_BACKEND_URL": "http://localhost:9437/mcp",
           "MCP_TIMEOUT": "30"
         }
       }
     }
   }
   ```

   **Important**: Replace `/absolute/path/to/RustyMail` with the actual full path to your RustyMail directory.

5. **Restart Claude Desktop** to load the MCP server

### Usage

Once configured, you can ask Claude to interact with your emails:

- "List my email folders"
- "Show me unread emails in my inbox"
- "Search for emails from john@example.com"
- "What are the statistics for my INBOX folder?"

Claude will use the MCP tools to access your email data through RustyMail.

### Architecture

```
┌─────────────────┐     JSON-RPC      ┌──────────────────────┐
│ Claude Desktop  │◄──── (stdio) ────►│  rustymail-mcp-stdio │
└─────────────────┘                   └──────────────────────┘
                                               │
                                               │ HTTP
                                               ▼
                                      ┌──────────────────┐
                                      │ Backend Server   │
                                      │ /mcp endpoint    │
                                      │ (Port 9437)      │
                                      └──────────────────┘
```

The stdio adapter acts as a thin proxy, forwarding JSON-RPC requests from Claude Desktop to the backend server's HTTP `/mcp` endpoint.

### SSE Example (JavaScript)

```js
const es = new EventSource('http://localhost:9437/api/v1/sse/connect');
es.onmessage = e => console.log('Received:', JSON.parse(e.data));

fetch('http://localhost:9437/api/v1/sse/command', {
  method: 'POST',
  headers: {'Content-Type': 'application/json'},
  body: JSON.stringify({ command: 'imap/listFolders', params: {} })
});
```

---

## Configuration

### Environment Variables

Edit `.env` file (see `.env.example` for full options):

```env
# Server Ports (use uncommon ports to avoid conflicts)
REST_PORT=9437         # Backend REST API
SSE_PORT=9438          # Server-Sent Events
DASHBOARD_PORT=9439    # Frontend dashboard

# Database
CACHE_DATABASE_URL=sqlite:data/email_cache.db

# Authentication
RUSTYMAIL_API_KEY=your-api-key-here

# Logging
LOG_LEVEL=info

# AI Providers (optional, for chatbot)
# OPENAI_API_KEY=sk-...
# ANTHROPIC_API_KEY=sk-ant-...
# OPENROUTER_API_KEY=sk-or-...
# (See .env.example for all 10+ supported providers)
```

### Account Configuration

Accounts are stored in `config/accounts.json` (file-based, not in .env):

```json
{
  "accounts": [
    {
      "id": "user@example.com",
      "name": "Work Email",
      "imap": {
        "host": "imap.example.com",
        "port": 993,
        "username": "user@example.com",
        "password": "your-password",
        "use_tls": true
      },
      "smtp": {
        "host": "smtp.example.com",
        "port": 587,
        "username": "user@example.com",
        "password": "your-password",
        "use_tls": true
      },
      "is_default": true
    }
  ]
}
```

Create this file in `config/accounts.json` to manage multiple email accounts.

---

## Documentation

- [REST API Reference](docs/REST-API.md)
- [System Info](docs/system_info.md)
- [Python Example](docs/python_test_example.py)
- [Additional Examples](docs/REST-EXAMPLES.md)

---

## Web Dashboard

RustyMail includes a modern React-based dashboard for managing emails and monitoring the server.

### Features

- 📧 **Email Management**: Browse, search, and manage emails across multiple accounts
- 🤖 **AI Chatbot**: Natural language interface with 10+ AI provider support
- 📊 **Real-time Monitoring**: Live server metrics via Server-Sent Events
- 👥 **Multi-Account**: Switch between configured email accounts
- 🎨 **Modern UI**: Built with React, TypeScript, and shadcn/ui components
- 🔄 **Smart Caching**: Displays cached emails for instant loading

### Accessing the Dashboard

1. Ensure services are running (see "Running with PM2" above)

2. Open your browser to:
   ```
   http://localhost:9439
   ```

3. The dashboard will automatically connect to the backend at port 9437

### Dashboard Development

For frontend developers working on the UI:

```bash
cd frontend/rustymail-app-main

# Install dependencies
npm install

# Start development server with hot-reload
npm run dev

# Build for production
npm run build
```

**Environment Variables**: The frontend automatically loads environment variables from the project root `.env` file via `dotenv-cli`. No separate frontend `.env` file is needed.

### Dashboard Architecture

- **Frontend**: React + TypeScript + Vite (port 9439)
- **Backend API**: Rust + Actix-web (port 9437)
- **Real-time Updates**: Server-Sent Events (SSE, port 9438)
- **State Management**: React Context API
- **UI Components**: shadcn/ui + Tailwind CSS

---

## Architecture

### System Overview

```
┌─────────────────┐      ┌──────────────────┐      ┌─────────────────┐
│  Web Dashboard  │◄────►│  Backend Server  │◄────►│  IMAP Servers   │
│   (Port 9439)   │      │   (Port 9437)    │      │  (Gmail, etc.)  │
└─────────────────┘      └──────────────────┘      └─────────────────┘
                                  │
                                  ▼
                         ┌─────────────────┐
                         │  SQLite Cache   │
                         │  + LRU Memory   │
                         └─────────────────┘
                                  │
                                  ▼
                         ┌─────────────────┐
                         │  Claude Desktop │
                         │   (MCP Stdio)   │
                         └─────────────────┘
```

### Two-Tier Caching System

RustyMail implements a sophisticated caching layer for optimal performance:

1. **SQLite Database** (`data/email_cache.db`)
   - Primary cache storage for emails and folder metadata
   - Persists across restarts
   - Stores: emails, folders, sync state, account data
   - Provides fast queries without hitting IMAP server

2. **In-Memory LRU Cache**
   - Performance optimization layer on top of SQLite
   - Caches frequently accessed emails and folders in RAM
   - Automatically populated from SQLite on cache misses
   - Cleared on restart

**Cache Flow**: Memory → SQLite → IMAP (fallback chain)

When accessing emails:
- First checks RAM cache (fastest)
- Falls back to SQLite query if not in RAM
- Only queries IMAP if not in cache at all
- Automatically populates caches on successful retrieval

This architecture ensures:
- ⚡ Sub-millisecond access to cached emails
- 💾 Minimal IMAP server load
- 🔄 Automatic cache synchronization
- 📊 Efficient handling of large mailboxes

### Multi-Account Support

Accounts are managed through `config/accounts.json`:
- File-based configuration (not environment variables)
- Support for multiple IMAP/SMTP accounts
- Per-account caching and folder management
- Account switching via MCP tools or REST API

---

## Development

### Run Tests

```bash
cargo test
```

### Run Benchmarks

```bash
cargo bench
```

### Lint

```bash
cargo clippy
```

## Contributing

Contributions welcome! Please see `CONTRIBUTING.md`.

## License

MIT License. See `LICENSE`.

## Authors

- Steve Olson - [GitHub](https://github.com/rangersdo)
- Contact: [steve@texasfortress.ai](mailto:steve@texasfortress.ai)

- Chris Odom - [GitHub](https://github.com/fellowtraveler)
- Contact: [chris@texasfortress.ai](mailto:chris@texasfortress.ai)
[![Tip in Crypto](https://tip.md/badge.svg)](https://tip.md/FellowTraveler)

## Sponsors

Sponsored by Texas Fortress AI.

[Become a sponsor](https://github.com/sponsors/rangersdo)
[Become a sponsor](https://github.com/sponsors/fellowtraveler)
