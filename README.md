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

- **REST API (default)**:
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

Server runs at `http://localhost:8080`

Example requests:

```bash
# List folders
curl -X GET http://localhost:8080/folders -H "Authorization: Basic $(echo -n 'user:pass' | base64)"

# List emails in INBOX
curl -X GET http://localhost:8080/emails/INBOX -H "Authorization: Basic $(echo -n 'user:pass' | base64)"
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

Connect:

```bash
curl -N http://localhost:8081/api/v1/sse/connect
```

Send commands:

```bash
curl -X POST http://localhost:8081/api/v1/sse/command \
  -H "Content-Type: application/json" \
  -d '{"command":"imap/listFolders","params":{}}'
```

---

## MCP Protocol Specification

RustyMail implements the Model Context Protocol (MCP) over stdio and SSE.

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
const es = new EventSource('http://localhost:8081/api/v1/sse/connect');
es.onmessage = e => console.log('Received:', JSON.parse(e.data));

fetch('http://localhost:8081/api/v1/sse/command', {
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
REST_PORT=8080

# MCP SSE
SSE_HOST=0.0.0.0
SSE_PORT=8081

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
   http://localhost:3000/dashboard
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

## Sponsors

Sponsored by Texas Fortress AI.

[Become a sponsor](https://github.com/sponsors/rangersdo)