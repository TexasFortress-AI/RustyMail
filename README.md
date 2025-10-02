# RustyMail

![RustyMail Architecture Diagram](docs/images/image%20(12).jpg)

[![Rust](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml/badge.svg)](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/rustymail)](https://crates.io/crates/rustymail)
[![Documentation](https://docs.rs/rustymail/badge.svg)](https://docs.rs/rustymail)

A high-performance, type-safe IMAP API server written in Rust, supporting REST, MCP Stdio, and MCP SSE interfaces.

---

## Features

- 🚀 **High Performance**: Built with Rust for speed and efficiency
- 🔒 **Type Safety**: Leverages Rust's type system for reliability
- 📧 **Full IMAP Support**: Access all folders and messages
- 🔄 **Multiple Interfaces**: REST API, MCP stdio, MCP SSE
- 🔐 **Secure**: TLS support and authentication
- 📊 **Monitoring**: Metrics and logging
- 🧪 **Comprehensive Testing**: Unit, integration, and performance tests
- 🧩 **Extensible**: Easily add new MCP tools
- ⚡ **Real-time Streaming**: Via SSE interface
- 🛠️ **MCP Protocol**: Full JSON-RPC 2.0 support over multiple transports

---

## Quick Start

### Prerequisites

- Rust 1.70+
- OpenSSL or compatible SSL library
- An IMAP server account

### Installation

```bash
git clone https://github.com/rangersdo/rustymail.git
cd rustymail
cp .env.example .env
# Edit .env with your IMAP server details
cargo build --release
```

### Running

- **REST API (default)** (port 9437):
  ```bash
  cargo run --release
  ```
- **MCP Stdio**:
  ```bash
  cargo run --release -- --mcp-stdio
  ```
- **MCP SSE**:
  ```bash
  cargo run --release -- --mcp-sse
  ```

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

### Supported Methods

- `imap/listFolders`
- `imap/createFolder`
- `imap/deleteFolder`
- `imap/renameFolder`
- `imap/searchEmails`
- `imap/fetchEmails`
- `imap/moveEmail`

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

Edit `.env` file:

```env
# IMAP
IMAP_HOST=imap.example.com
IMAP_PORT=993
IMAP_USERNAME=your_username
IMAP_PASSWORD=your_password

# REST API
REST_HOST=0.0.0.0
REST_PORT=9437

# SSE endpoints (shares same port as REST)
# SSE is available at the same port as REST API

# General
LOG_LEVEL=info
INTERFACE=rest  # rest, stdio, sse
```

---

## Documentation

- [REST API Reference](docs/REST-API.md)
- [System Info](docs/system_info.md)
- [Python Example](docs/python_test_example.py)
- [Additional Examples](docs/REST-EXAMPLES.md)

---

## Dashboard

RustyMail includes a web-based dashboard for monitoring and interacting with the server. The dashboard provides:

- Real-time statistics about connections and server performance
- List of connected clients
- AI chatbot interface for natural language interaction
- Configuration information

### Dashboard Setup

To set up and build the dashboard:

1. Run the provided build script:
   ```bash
   ./scripts/build-dashboard.sh
   ```

2. Ensure your `.env` file includes the dashboard configuration:
   ```
   DASHBOARD_ENABLED=true
   DASHBOARD_PATH=./dashboard-static
   ```

3. Run the server with the REST/SSE interface enabled:
   ```bash
   cargo run --bin rustymail-server
   ```

4. Access the dashboard at:
   ```
   http://localhost:9439/dashboard
   ```

### Dashboard Development

For frontend developers, you can work on the dashboard UI separately:

1. Navigate to the frontend directory:
   ```bash
   cd frontend/rustymail-app-main
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Start the development server:
   ```bash
   npm run dev
   ```

4. Build for production:
   ```bash
   npm run build
   ```

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
