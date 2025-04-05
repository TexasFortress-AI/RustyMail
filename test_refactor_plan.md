# Test Structure Refactoring Plan

## 1. Audit & Inventory Phase
- [ ] Create a backup branch of the current state
- [ ] Document all existing test files and their purposes
- [ ] Identify duplicate test files:
  * `integration_tests.rs` appears in root and `/src`
  * `lib.rs` appears in multiple locations
- [ ] Run all tests and create a coverage report as baseline

## 2. Clean Up Phase
- [ ] Remove the redundant `src` directory inside `tests/`
  * Merge `tests/src/integration_tests.rs` with root `tests/integration_tests.rs`
- [ ] Consolidate duplicate `lib.rs` files
- [ ] Remove empty or redundant test files

## 3. Reorganization Phase
- [ ] Create a new structure following Rust best practices:
  ```
  tests/
  ├── common/           # Shared test utilities and helpers
  ├── integration/      # All integration tests
  │   ├── api/         # API-related integration tests
  │   └── imap/        # IMAP-related integration tests
  ├── unit/            # Unit tests
  │   ├── api/         # API-related unit tests
  │   └── imap/        # IMAP-related unit tests
  ├── mocks/           # All mock implementations
  └── performance/     # Performance/benchmark tests
  ```

## 4. Migration Phase
- [ ] Move tests to their new locations:
  * Move `performance_tests.rs` to `tests/performance/`
  * Reorganize `unit_tests/*` into appropriate subdirectories
  * Move mock tests into `tests/mocks/`
  * Consolidate integration tests into `tests/integration/`

## 5. Cleanup & Validation Phase
- [ ] Update any test helper imports to reflect new structure
- [ ] Update any test module declarations
- [ ] Run complete test suite to verify nothing was broken
- [ ] Compare new test coverage report with baseline
- [ ] Update any CI/CD configurations if needed

## 6. Documentation Phase
- [ ] Update README with new test structure
- [ ] Document test organization and conventions
- [ ] Add comments explaining the purpose of each test directory

## 7. Review & Verification
- [ ] Verify all tests still pass
- [ ] Check for any remaining duplicate tests
- [ ] Ensure test coverage hasn't decreased
- [ ] Review error messages and test output for clarity 