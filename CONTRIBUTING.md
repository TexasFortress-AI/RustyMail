# Contributing to RustyMail

Thank you for your interest in contributing to RustyMail! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to Contribute

1. Fork the repository
2. Create a new branch for your feature or bugfix
3. Make your changes
4. Run tests and ensure they pass
5. Submit a pull request

## Development Setup

1. Install Rust (1.70 or later)
2. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/rustymail.git
   cd rustymail
   ```
3. Copy the environment file:
   ```bash
   cp .env.example .env
   ```
4. Install dependencies:
   ```bash
   cargo build
   ```

## Running Tests

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test integration_tests

# Run benchmarks
cargo bench
```

## Code Style

We follow the Rust community's standard coding style. Please run:

```bash
cargo fmt
cargo clippy
```

before submitting your changes.

## Documentation

- Code should be well-documented with comments
- Public APIs should have doc comments
- Update relevant documentation when making changes

## Pull Request Process

1. Update the README.md with details of changes if needed
2. Update the documentation in the `docs/` directory
3. The PR must pass all CI checks
4. The PR must be reviewed by at least one maintainer

## Commit Messages

Please follow these guidelines for commit messages:

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests after the first line

## Feature Requests and Bug Reports

Please use the GitHub issue tracker to submit feature requests and bug reports. Include:

- A clear description of the issue
- Steps to reproduce
- Expected behavior
- Actual behavior
- Environment details

## Security Issues

Please report security issues to security@example.com. Do not create a public issue.

## License

By contributing to RustyMail, you agree that your contributions will be licensed under the project's MIT License. 