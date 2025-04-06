# RustyMail

![RustyMail](docs/images/RustyMail.jpg)

[![Rust](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml/badge.svg)](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/rustymail)](https://crates.io/crates/rustymail)
[![Documentation](https://docs.rs/rustymail/badge.svg)](https://docs.rs/rustymail)

A high-performance, type-safe IMAP API server written in Rust. RustyMail provides multiple interfaces to IMAP servers:

1. **REST API** (✅ Implemented)
   - RESTful HTTP endpoints
   - JSON request/response
   - Traditional web API interface
   - Ideal for web applications

2. **MCP Stdio Server** (⏳ Coming Soon)
   - JSON-RPC 2.0 over stdin/stdout
   - Local CLI integration
   - Synchronous command execution
   - Perfect for IDE extensions and local tools

3. **MCP SSE Server** (⏳ Coming Soon)
   - HTTP POST for client requests
   - Server-Sent Events for responses
   - Real-time streaming capabilities
   - Great for real-time applications

## Features

- 🚀 **High Performance**: Built with Rust for maximum speed and efficiency
- 🔒 **Type Safety**: Leverages Rust's type system for reliable code
- 📧 **Full IMAP Support**: Access all your email folders and messages
- 🔄 **Multiple Interfaces**: REST API, MCP stdio, and MCP SSE
- 🔐 **Secure**: TLS support and proper authentication
- 📊 **Monitoring**: Built-in metrics and logging
- 🧪 **Comprehensive Testing**: Unit, integration, and performance tests
- 📚 **Well Documented**: Detailed API documentation and examples

## Quick Start

### Prerequisites

- Rust 1.70 or later
- OpenSSL or equivalent SSL library
- An IMAP server to connect to

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/rangersdo/rustymail.git
   cd rustymail
   ```

2. Copy the environment file and configure it:
   ```bash
   cp .env.example .env
   # Edit .env with your IMAP server details
   ```

3. Build and run:
   ```bash
   cargo build --release
   cargo run --release
   ```

## Interface Usage

### 1. REST API

The REST API server starts on `http://localhost:8080` by default.

```bash
# List folders
curl -X GET http://localhost:8080/folders \
  -H "Authorization: Basic $(echo -n 'username:password' | base64)"

# List emails in INBOX
curl -X GET http://localhost:8080/emails/INBOX \
  -H "Authorization: Basic $(echo -n 'username:password' | base64)"
```

### 2. MCP Stdio Server

Use the `--mcp-stdio` flag to start in stdio mode:

```bash
# Start in MCP stdio mode
cargo run --release -- --mcp-stdio

# Example JSON-RPC request (via stdin)
{"jsonrpc": "2.0", "method": "list_folders", "params": {}, "id": 1}
```

### 3. MCP SSE Server

Use the `--mcp-sse` flag to start in SSE mode:

```bash
# Start in MCP SSE mode
cargo run --release -- --mcp-sse

# Connect and receive events
curl -N http://localhost:8081/events

# Send commands (in another terminal)
curl -X POST http://localhost:8081/command \
  -H "Content-Type: application/json" \
  -d '{"method": "list_folders", "params": {}}'
```

## Configuration

Create a `.env` file in the project root:

```env
# IMAP Settings
IMAP_HOST=imap.example.com
IMAP_PORT=993
IMAP_USERNAME=your_username
IMAP_PASSWORD=your_password

# REST API Settings
REST_HOST=0.0.0.0
REST_PORT=8080

# MCP SSE Settings
SSE_HOST=0.0.0.0
SSE_PORT=8081

# General Settings
LOG_LEVEL=info
INTERFACE=rest  # Options: rest, stdio, sse
```

See `.env.example` for all available configuration options.

## Documentation

- [API Reference](docs/API.md) - REST API documentation
- [MCP Protocol](docs/MCP.md) - MCP protocol specification
- [Error Codes](docs/ERRORS.md) - Error code reference
- [Usage Examples](docs/EXAMPLES.md) - Code examples for all interfaces
- [Deployment Guide](docs/DEPLOYMENT.md) - Deployment instructions

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Test specific interface
cargo test --test rest_api_tests
cargo test --test mcp_stdio_tests
cargo test --test mcp_sse_tests

# Run benchmarks
cargo bench
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

If you encounter any issues or have questions, please:

1. Check the [documentation](docs/)
2. Search for existing issues
3. Create a new issue if needed

## Authors

- Steve Olson - Initial work - [Steve's GitHub](https://github.com/rangersdo)
- Contact: [steve@texasfortress.ai](mailto:steve@texasfortress.ai)

## Sponsors

Sponsored by Texas Fortress AI.

[Become a sponsor](https://github.com/sponsors/rangersdo) 