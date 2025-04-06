# RustyMail

![RustyMail](docs/images/RustyMail.jpg)

[![Rust](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml/badge.svg)](https://github.com/rangersdo/rustymail/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/rustymail)](https://crates.io/crates/rustymail)
[![Documentation](https://docs.rs/rustymail/badge.svg)](https://docs.rs/rustymail)

A high-performance, type-safe IMAP API server written in Rust. RustyMail provides a RESTful interface to IMAP servers, making it easy to integrate email functionality into your applications.

## Features

- 🚀 **High Performance**: Built with Rust for maximum speed and efficiency
- 🔒 **Type Safety**: Leverages Rust's type system for reliable code
- 📧 **Full IMAP Support**: Access all your email folders and messages
- 🔄 **Async Operations**: Non-blocking I/O for better performance
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

The server will start on `http://localhost:8080` by default.

## Configuration

Create a `.env` file in the project root with the following variables:

```env
IMAP_HOST=imap.example.com
IMAP_PORT=993
IMAP_USERNAME=your_username
IMAP_PASSWORD=your_password
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
```

See `.env.example` for all available configuration options.

## API Documentation

The API documentation is available in the `docs` directory:

- [API Reference](docs/API.md)
- [Error Codes](docs/ERRORS.md)
- [Usage Examples](docs/EXAMPLES.md)
- [Deployment Guide](docs/DEPLOYMENT.md)

## Examples

### List Folders

```bash
curl -X GET http://localhost:8080/folders \
  -H "Authorization: Basic $(echo -n 'username:password' | base64)"
```

### List Emails in a Folder

```bash
curl -X GET http://localhost:8080/emails/INBOX \
  -H "Authorization: Basic $(echo -n 'username:password' | base64)"
```

See [EXAMPLES.md](docs/EXAMPLES.md) for more examples in various programming languages.

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test integration_tests

# Run benchmarks
cargo bench
```

### Building Documentation

```bash
# Build and open documentation
cargo doc --open
```

## Performance

RustyMail is designed for high performance. Here are some benchmark results:

```
folder_operations/list_folders
                        time:   [472.50 ms 554.99 ms 663.12 ms]

folder_operations/folder_stats
                        time:   [511.27 ms 686.22 ms 894.69 ms]

email_operations/list_emails
                        time:   [496.81 ms 675.59 ms 861.86 ms]
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [imap](https://crates.io/crates/imap) - IMAP protocol implementation
- [actix-web](https://actix.rs/) - Web framework
- [tokio](https://tokio.rs/) - Async runtime
- [tracing](https://crates.io/crates/tracing) - Structured logging

## Support

If you encounter any issues or have questions, please:

1. Check the [documentation](docs/)
2. Search for existing issues
3. Create a new issue if needed

## Roadmap

- TBD

## Security

Please report any security vulnerabilities to security@example.com.

## Authors

- Your Name - Initial work - [YourGitHub](https://github.com/yourusername)

## Sponsors

Support this project by becoming a sponsor. Your logo will show up here with a link to your website.

[Become a sponsor](https://github.com/sponsors/yourusername) 