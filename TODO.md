# RustyMail Testing TODO

## Project Goal

Achieve comprehensive test coverage for the RustyMail IMAP server, including Unit, Live Integration (REST), End-to-End (REST), MCP, and SSE interfaces.

## Current Status (as of last interaction)

*   **Unit Tests:**
    *   33 tests passing consistently.
    *   Compiler warnings resolved (removed dead code in mocks).
    *   Coverage seems reasonable for core IMAP client/session logic using mocks.
*   **Live Integration Tests (`tests/rest_live_test.rs`):**
    *   Uses `#[cfg(feature = "live_tests")]` and requires live IMAP server credentials in `.env`.
    *   Run via `bash scripts/test_live.sh`.
    *   Configuration loading fixed to use `dotenv`.
    *   Tests implemented for: health, list_folders, create_folder, delete_folder, rename_folder, select_folder, search_emails, fetch_emails, move_email.
    *   **Last run was interrupted**, but compilation errors related to struct fields (`MailboxInfo`, `Email`) were fixed. Need confirmation run.
*   **End-to-End Tests (`tests/rest_e2e_test.rs`):**
    *   Uses `TestServer` struct to manage a local `rustymail-server` process.
    *   Requires compiled binary and `.env` file.
    *   Run via `bash scripts/test_e2e.sh`.
    *   Main test `run_rest_e2e_tests` modified to call helper functions for: list, create/delete, rename, select, search, fetch, move.
    *   Compiler warnings resolved (unused variables/helpers).
    *   **Last run was interrupted**, need confirmation run.
*   **Test Scripts (`scripts/*.sh`):**
    *   `test_all.sh` orchestrates running unit, live, and e2e tests.
    *   Scripts updated to handle feature flags and exit codes correctly.

## Immediate Next Steps

1.  **Confirm Current Tests Pass:** Run `bash scripts/test_all.sh` to completion. The last few runs were interrupted. This will confirm if the expanded E2E tests and newly implemented Live tests are truly passing after recent fixes.
2.  **Address Failures:** If the full run reveals failures in the E2E or Live tests, diagnose and fix them. Potential areas: logic errors in newly added tests, race conditions, state cleanup issues between tests.

## Remaining Tasks (Test Coverage Gaps)

*   **REST API - Flags:**
    *   Implement Live Test(s) for adding/setting/removing flags (`/folders/{folder}/emails/flags`).
    *   Implement E2E Test(s) for flag operations.
*   **REST API - Append:**
    *   Implement Live Test(s) for appending emails (`/folders/{folder}/emails/append`).
    *   Implement E2E Test(s) for appending emails.
*   **MCP Interface (`src/api/mcp.rs`):**
    *   Create E2E test suite for MCP over stdio (`tests/mcp_stdio_e2e_test.rs`?).
        *   Needs mechanism to spawn server with MCP stdio enabled.
        *   Needs client logic to send JSON-RPC requests over stdin and read responses from stdout.
        *   Test core operations (list, create, select, fetch, etc.).
    *   Create E2E test suite for MCP over SSE (if applicable, needs clarification).
*   **SSE Interface (`src/api/sse.rs`):**
    *   Create E2E test suite for SSE (`tests/sse_e2e_test.rs`?).
        *   Needs mechanism to spawn server with SSE enabled.
        *   Needs SSE client to connect and verify events are received for actions performed via another interface (e.g., REST).

## Key Context

*   **Credentials:** Live and E2E tests rely on IMAP credentials stored in a `.env` file at the project root.
*   **Test Execution:** Use the `bash scripts/test_*.sh` scripts for running specific test suites or all of them.
*   **Concurrency:** E2E and Live tests currently run sequentially within their respective main test functions (`run_rest_e2e_tests`, tests within `mod live_tests`). 