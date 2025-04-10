# RustyMail Tests

This directory contains all tests for the RustyMail project, organized by test type.

## Test Structure

### Unit Tests (`tests/unit/`)
- `api/` - API-related unit tests
- `imap/` - IMAP-related unit tests
- `transport/` - Transport layer unit tests
- `config/` - Configuration-related unit tests

### Integration Tests (`tests/integration/`)
- `dashboard/` - Dashboard-related integration tests
- `mcp/` - MCP-related integration tests

### End-to-End Tests (`tests/e2e/`)
- `rest.rs` - REST API end-to-end tests
- `live.rs` - Live server end-to-end tests

## Running Tests

Tests can be run using the scripts in the `scripts/` directory:

```bash
# Run all tests
./scripts/test.sh

# Run specific test types
./scripts/test-unit.sh
./scripts/test-integration.sh
./scripts/test-e2e.sh
```

## Test Features

Some tests require specific features to be enabled:

- `integration_tests` - For mock integration tests
- `live_tests` - For live server tests

Enable these features when running tests:

```bash
cargo test --features integration_tests
cargo test --features live_tests
``` 