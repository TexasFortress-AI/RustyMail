# Test Inventory

## Root Level Test Files
- `tests/integration_tests.rs`: Integration tests at root level
- `tests/lib.rs`: Test library configuration
- `tests/performance_tests.rs`: Performance and benchmark tests

## Directory: tests/src/
- `integration_tests.rs`: Duplicate integration tests (to be merged)
- `lib.rs`: Duplicate library configuration (to be consolidated)

## Directory: tests/common/
- `config.rs`: Shared test configuration utilities
- `mod.rs`: Common module declarations

## Directory: tests/mock_tests/
- `common.rs`: Common mock utilities
- `integration_tests.rs`: Mock-specific integration tests
- `mock.rs`: Mock implementations
- `mod.rs`: Mock module declarations

## Directory: tests/unit_tests/
- `api_tests.rs`: Unit tests for API functionality
- `imap_client_tests.rs`: Unit tests for IMAP client

## Analysis of Duplicates and Issues:
1. Integration Tests Duplication:
   - Root `integration_tests.rs`
   - `src/integration_tests.rs`
   - `mock_tests/integration_tests.rs`

2. Library Configuration Duplication:
   - Root `lib.rs`
   - `src/lib.rs`

3. Structural Issues:
   - Redundant `src` directory inside tests
   - Mixed test organization (some in directories, some at root)
   - Unclear separation between mock and real integration tests

## Test Categories Found:
1. Unit Tests
   - API tests
   - IMAP client tests

2. Integration Tests
   - General integration tests
   - Mock-based integration tests

3. Performance Tests
   - Benchmark tests

4. Support Code
   - Common utilities
   - Mock implementations
   - Test configurations 