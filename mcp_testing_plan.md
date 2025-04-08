# MCP End-to-End Testing Plan

## 1. Goal

Achieve comprehensive end-to-end (E2E) test coverage for the RustyMail MCP stdio and MCP SSE interfaces, ensuring they function correctly when interacting with the application as a whole, validated against various IMAP backend implementations.

## 2. Overall Strategy

1.  **Dedicated Test Suites:** Create new test files within the `tests/` directory:
    *   `mcp_stdio_e2e_test.rs`
    *   `mcp_sse_e2e_test.rs`
2.  **Ports and Adapters for IMAP Backend:** Structure the tests to interact with an abstract "IMAP Backend Port". Implement specific "Adapters" for different IMAP server targets:
    *   **`MockImapAdapter` (Default):** Runs against a configurable, in-process mock IMAP server for fast, deterministic testing (ideal for development, standard CI).
    *   **`GoDaddyImapAdapter`:** Runs against a live GoDaddy IMAP server for real-world validation.
    *   **(Future Adapters):** Structure allows easy addition of `GmailImapAdapter`, `OutlookImapAdapter`, etc.
3.  **Adapter Selection:** Use a configuration mechanism (e.g., environment variable `IMAP_ADAPTER=Mock|GoDaddy|Gmail`, feature flags) to select which adapter the test suite should run against.
4.  **Application Subprocess:** Launch the `rustymail` binary as a separate process, configured to use the appropriate MCP interface and connect to the IMAP server details provided by the *selected adapter*.
5.  **Client Simulation:**
    *   **Stdio:** Interact directly with the subprocess's standard input and output streams.
    *   **SSE:** Use HTTP and SSE client libraries to connect to the running subprocess, send commands, and monitor the event stream.
6.  **Gherkin for Scenarios:** Define test cases using Gherkin syntax (`.feature` files). Steps interacting with the backend will delegate to the selected adapter for setup, teardown, and potentially verification.
7.  **Test Runner:** Utilize a Rust test runner capable of executing Gherkin scenarios (e.g., `cucumber`) and managing the adapter selection and state.

## 3. IMAP Backend Adapters

### 3.1. `MockImapAdapter`

*   **Purpose:** Provide a fast, reliable, and controllable IMAP backend for development and CI.
*   **Implementation:** Wraps the Mock IMAP Server logic.
    *   **Mock Server:** A background TCP server understanding basic IMAP commands, allowing tests to set expectations and define responses.
*   **Configuration:** Self-contained, starts the mock server on an available port.
*   **Setup/Teardown:** The adapter handles starting/stopping the mock server and configuring its behavior per test.
*   **Verification:** Allows tests to query the mock server post-execution to confirm expected commands were received.

### 3.2. `GoDaddyImapAdapter`

*   **Purpose:** Validate RustyMail against the specific behavior of GoDaddy's live IMAP service.
*   **Implementation:** Does not start a server, but manages configuration and interaction with the live service.
*   **Configuration:** Requires connection details (host, port, user, pass, TLS settings) for a **dedicated GoDaddy test account**, loaded via environment variables or a config file.
*   **Setup/Teardown:** Handles ensuring the live test account is in a known state before a test run (e.g., deleting specific emails/folders) and cleaning up afterward. This is crucial and potentially complex.
*   **Verification:** Primarily relies on asserting MCP responses. Direct verification on the server might be possible but adds complexity.

### 3.3. (Future Adapters: Gmail, Outlook, etc.)

*   Similar structure to `GoDaddyImapAdapter` but tailored to the specific authentication (e.g., OAuth for Gmail), configuration, and state management needs of those providers.

## 4. Test Abstraction Layer

*   Tests should interact with a trait or facade representing the `ImapBackendAdapter`.
*   This facade will expose methods like:
    *   `get_imap_connection_details() -> ImapConfig` (host, port, user, pass, etc. for `rustymail`)
    *   `setup_scenario(&ScenarioContext)` (e.g., configure mock responses, ensure live account state)
    *   `teardown_scenario(&ScenarioContext)` (e.g., stop mock, clean up live account)
    *   `verify_interaction(&ExpectedInteraction)` (e.g., query mock, potentially NOOP for live)
*   The test runner or `World` struct in Cucumber will hold an instance of the selected adapter.

## 5. MCP Stdio E2E Tests (`mcp_stdio_e2e_test.rs`)

*   **Test Structure (Adapter-Driven):**
    *   Gherkin steps / `#[test]` functions.
    *   **Setup:**
        1.  Select the `ImapBackendAdapter` based on configuration.
        2.  Call `adapter.setup_scenario(...)`.
        3.  Get connection details from `adapter.get_imap_connection_details()`.
        4.  Prepare `rustymail` command-line arguments using these details.
        5.  Launch `rustymail` subprocess.
        6.  Get `stdin`/`stdout` handles.
    *   **Interaction:**
        1.  Construct JSON-RPC requests.
        2.  Write requests to `stdin`.
        3.  Read and parse responses from `stdout`.
        4.  Assert response correctness (ID, result/error).
        5.  Call `adapter.verify_interaction(...)` if needed (likely only for mock).
    *   **Teardown:**
        1.  Terminate `rustymail` subprocess.
        2.  Call `adapter.teardown_scenario(...)`.
*   **Expanded Gherkin Examples (`mcp_stdio.feature`):**

    ```gherkin
    Feature: MCP Stdio Interface E2E Tests

      Background:
        Given the selected IMAP backend adapter is configured for basic connection # Adapter setup
        And the RustyMail stdio process is started using the adapter configuration

      Scenario: Successfully list folders
        Given the adapter is configured for LIST operation
        When a "imap/listFolders" request with id 1 is sent via stdio
        Then a JSON-RPC response with id 1 should be received via stdio
        And the response result should contain folders
        And the adapter specific verification confirms the LIST interaction

      Scenario: Select an existing folder
        Given the adapter is configured for SELECT "INBOX" operation
        When a "imap/selectFolder" request with id 2 and params {"folder_name": "INBOX"} is sent via stdio
        Then a JSON-RPC response with id 2 should be received via stdio
        And the response result should indicate "INBOX" is selected
        And the adapter specific verification confirms the SELECT interaction

      Scenario: Select a non-existent folder
        Given the adapter is configured for SELECT "NonExistent" operation to fail
        When a "imap/selectFolder" request with id 3 and params {"folder_name": "NonExistent"} is sent via stdio
        Then a JSON-RPC response with id 3 should be received via stdio
        And the response should contain an error with code -32010 # IMAP_OPERATION_FAILED
        And the adapter specific verification confirms the failed SELECT interaction

      Scenario: Search emails in selected folder
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for SEARCH in "INBOX" returning UIDs 1, 2
        When a "imap/selectFolder" request with id 4 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 4 is received
        And a "imap/searchEmails" request with id 5 and params {"criteria": "ALL"} is sent via stdio
        Then a JSON-RPC response with id 5 should be received via stdio
        And the response result should contain UIDs 1, 2
        And the adapter specific verification confirms the SEARCH interaction

      Scenario: Attempt search without selecting folder
        When a "imap/searchEmails" request with id 6 and params {"criteria": "ALL"} is sent via stdio
        Then a JSON-RPC response with id 6 should be received via stdio
        # Expecting a specific MCP/Tool error indicating no folder selected
        And the response should contain an error indicating "No folder selected"

      Scenario: Fetch email details (no body)
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for FETCH UID 1 HEADERS/FLAGS in "INBOX"
        When a "imap/selectFolder" request with id 7 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 7 is received
        And a "imap/fetchEmails" request with id 8 and params {"uids": [1], "fetch_body": false} is sent via stdio
        Then a JSON-RPC response with id 8 should be received via stdio
        And the response result should contain email details for UID 1 without body
        And the adapter specific verification confirms the FETCH interaction

      Scenario: Fetch email details (with body)
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for FETCH UID 1 BODY in "INBOX"
        When a "imap/selectFolder" request with id 9 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 9 is received
        And a "imap/fetchEmails" request with id 10 and params {"uids": [1], "fetch_body": true} is sent via stdio
        Then a JSON-RPC response with id 10 should be received via stdio
        And the response result should contain email details for UID 1 with body content
        And the adapter specific verification confirms the FETCH interaction

      Scenario: Create a new folder
        Given the adapter is configured for CREATE "MyNewFolder" operation
        When a "imap/createFolder" request with id 11 and params {"folder_name": "MyNewFolder"} is sent via stdio
        Then a JSON-RPC response with id 11 should be received via stdio
        And the response result should indicate success
        And the adapter specific verification confirms the CREATE interaction

      Scenario: Append email to folder
        Given the adapter is configured for APPEND to "INBOX" operation
        When a "imap/appendEmail" request with id 12 and params {"folder_name": "INBOX", "payload": "BASE64_ENCODED_EMAIL_DATA", "flags": ["\Seen"]} is sent via stdio
        Then a JSON-RPC response with id 12 should be received via stdio
        And the response result should indicate success
        And the adapter specific verification confirms the APPEND interaction

      Scenario: Add flags to email
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for STORE UID 1 +FLAGS (\Flagged) in "INBOX"
        When a "imap/selectFolder" request with id 13 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 13 is received
        And a "imap/modifyFlags" request with id 14 and params {"uids": [1], "operation": "add", "flags": ["\Flagged"]} is sent via stdio
        Then a JSON-RPC response with id 14 should be received via stdio
        And the response result should indicate success
        And the adapter specific verification confirms the STORE interaction

      Scenario: Move email to another folder
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for MOVE UID 1 from "INBOX" to "Archive" operation
        When a "imap/selectFolder" request with id 15 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 15 is received
        And a "imap/moveEmails" request with id 16 and params {"uids": [1], "destination_folder": "Archive"} is sent via stdio
        Then a JSON-RPC response with id 16 should be received via stdio
        And the response result should indicate success
        And the adapter specific verification confirms the MOVE interaction

      Scenario: Expunge folder
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for EXPUNGE in "INBOX" operation
        When a "imap/selectFolder" request with id 17 and params {"folder_name": "INBOX"} is sent via stdio
        And a successful response for id 17 is received
        And a "imap/expungeFolder" request with id 18 is sent via stdio
        Then a JSON-RPC response with id 18 should be received via stdio
        And the response result should indicate success (e.g., number of expunged messages)
        And the adapter specific verification confirms the EXPUNGE interaction

      Scenario: Handle invalid JSON request
        When the invalid JSON string "{\"jsonrpc\": \"2.0\"" is sent via stdio
        Then a JSON-RPC response should be received via stdio
        And the response should contain an error with code -32700 # PARSE_ERROR

      Scenario: Handle invalid JSON-RPC (missing method)
        When the JSON string "{\"jsonrpc\": \"2.0\", \"id\": 20}" is sent via stdio
        Then a JSON-RPC response with id 20 should be received via stdio
        And the response should contain an error with code -32600 # INVALID_REQUEST

      Scenario: Handle unknown method
        When a "unknown/method" request with id 21 is sent via stdio
        Then a JSON-RPC response with id 21 should be received via stdio
        And the response should contain an error with code -32601 # METHOD_NOT_FOUND
    ```

## 6. MCP SSE E2E Tests (`mcp_sse_e2e_test.rs`)

*   **Test Structure (Adapter-Driven):**
    *   Gherkin steps / `#[tokio::test]` functions.
    *   **Setup:**
        1.  Select the `ImapBackendAdapter`.
        2.  Call `adapter.setup_scenario(...)`.
        3.  Get connection details from `adapter.get_imap_connection_details()`.
        4.  Prepare `rustymail` args using these details and a unique HTTP port.
        5.  Launch `rustymail` subprocess.
        6.  Wait for HTTP server readiness.
        7.  Initialize HTTP and SSE clients.
    *   **Interaction:**
        1.  Connect SSE client, get `clientId`.
        2.  Send commands via HTTP POST.
        3.  Assert POST response.
        4.  Listen to SSE stream, collect events.
        5.  Assert sequence and content of relevant SSE events.
        6.  Call `adapter.verify_interaction(...)` if needed.
    *   **Teardown:**
        1.  Close SSE client.
        2.  Terminate `rustymail` subprocess.
        3.  Call `adapter.teardown_scenario(...)`.
*   **Expanded Gherkin Examples (`mcp_sse.feature`):**

    ```gherkin
    Feature: MCP SSE Interface E2E Tests

      Background:
        Given the selected IMAP backend adapter is configured for basic connection # Adapter setup
        And the RustyMail SSE process is started using the adapter configuration
        And an SSE client connects and receives a clientId

      Scenario: Client receives welcome and heartbeats
        Then an SSE "welcome" event with the clientId should be received
        And eventually an SSE "heartbeat" event should be received

      Scenario: Successfully list folders
        Given the adapter is configured for LIST operation
        When a "imap/listFolders" command is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_result" event should be received
        And the event data should contain folders
        And the adapter specific verification confirms the LIST interaction

      Scenario: Select folder successfully
        Given the adapter is configured for SELECT "INBOX" operation
        When a "imap/selectFolder" command with params {"folder_name": "INBOX"} is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_result" event should be received
        And the event data should indicate "INBOX" is selected
        And the adapter specific verification confirms the SELECT interaction

      Scenario: Select non-existent folder results in error event
        Given the adapter is configured for SELECT "NonExistent" operation to fail
        When a "imap/selectFolder" command with params {"folder_name": "NonExistent"} is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_error" event should be received
        And the event data should contain error code -32010 # IMAP_OPERATION_FAILED
        And the adapter specific verification confirms the failed SELECT interaction

      Scenario: Search emails successfully
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for SEARCH in "INBOX" returning UIDs 1, 2
        When a "imap/selectFolder" command with params {"folder_name": "INBOX"} is sent via POST for the client
        And the corresponding "tool_result" event is received
        And a "imap/searchEmails" command with params {"criteria": "ALL"} is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_result" event should be received
        And the event data should contain UIDs 1, 2
        And the adapter specific verification confirms the SEARCH interaction

      Scenario: Fetch emails successfully
        Given the adapter is configured for SELECT "INBOX" operation
        And the adapter is configured for FETCH UID 1 BODY in "INBOX"
        When a "imap/selectFolder" command with params {"folder_name": "INBOX"} is sent via POST for the client
        And the corresponding "tool_result" event is received
        And a "imap/fetchEmails" command with params {"uids": [1], "fetch_body": true} is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_result" event should be received
        And the event data should contain email details for UID 1 with body content
        And the adapter specific verification confirms the FETCH interaction

      # --- Add similar scenarios for other IMAP operations (Create, Append, Store, Move, Expunge) ---
      # --- Examples: ---
      # Scenario: Create folder successfully
      # Scenario: Append email successfully
      # Scenario: Add flags successfully
      # Scenario: Move email successfully
      # Scenario: Expunge folder successfully

      Scenario: Handle unknown tool command
        When a "unknown/tool" command is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_error" event should be received
        And the event data should contain error code -32601 # METHOD_NOT_FOUND

      Scenario: Handle command with invalid parameters
        Given the adapter is configured for LIST operation
        When a "imap/listFolders" command with invalid params {"bad_param": 123} is sent via POST for the client
        Then the POST request should be accepted
        And an SSE "tool_error" event should be received
        And the event data should contain error code -32602 # INVALID_PARAMS

      Scenario: Handle command missing client_id in payload
        When a POST request to the command endpoint is sent with payload {"command": "imap/listFolders"} # No client_id
        Then the POST request should be rejected with status 400 # Bad Request

    ```

## 7. Gherkin Integration Options (Rust)

*   **Primary Option:** [`cucumber`](https://crates.io/crates/cucumber)
    *   The `World` struct will hold an instance of the selected `ImapBackendAdapter` trait object (`Box<dyn ImapBackendAdapter>`).
    *   Step definitions (`#[given]`, `#[when]`, `#[then]`) will call methods on `world.adapter`.
    *   Adapter selection logic happens once when the `World` is initialized.

## 8. Implementation Checklist (Revised)

**Phase 1: Foundation & Abstraction**

*   [ ] **Define `ImapBackendAdapter` Trait:** Specify methods (`get_imap_connection_details`, `setup_scenario`, `teardown_scenario`, `verify_interaction`).
*   [ ] **Implement `MockImapAdapter`:**
    *   [ ] Choose Mock IMAP Server Strategy.
    *   [ ] Implement Mock IMAP Server (or integrate crate).
    *   [ ] Implement the `MockImapAdapter` struct implementing the trait.
*   [ ] **Refactor `main.rs`/`cli.rs` (if needed):** Ensure easy configuration via arguments.
*   [ ] **Implement Adapter Selection Logic:** Read env var/flag to determine which adapter to instantiate.
*   [ ] **Add Test Utilities Crate (Optional).**

**Phase 2: Stdio E2E Tests (Adapter-Driven)**

*   [ ] **Setup Test File:** `tests/mcp_stdio_e2e_test.rs`.
*   [ ] **Adapter Integration:** Instantiate selected adapter.
*   [ ] **Subprocess Management:** Launch `rustymail` using adapter config.
*   [ ] **I/O Handling:** Implement stdin/stdout interaction.
*   [ ] **Basic Test Cases (using Mock Adapter initially):** Implement core scenarios calling adapter methods for setup/teardown/verification.

**Phase 3: SSE E2E Tests (Adapter-Driven)**

*   [ ] **Setup Test File:** `tests/mcp_sse_e2e_test.rs`.
*   [ ] **Add Dependencies:** `reqwest`, SSE client.
*   [ ] **Adapter Integration:** Instantiate selected adapter.
*   [ ] **Subprocess Management:** Launch `rustymail` using adapter config.
*   [ ] **Client Implementation:** SSE connection, POST commands, event listening.
*   [ ] **Basic Test Cases (using Mock Adapter initially):** Implement core scenarios calling adapter methods.

**Phase 4: `GoDaddyImapAdapter` Implementation**

*   [ ] **Implement `GoDaddyImapAdapter` Struct:** Implement the `ImapBackendAdapter` trait.
*   [ ] **Configuration Loading:** Implement logic to load GoDaddy test account details (env vars recommended).
*   [ ] **State Management:** Implement `setup_scenario` and `teardown_scenario` logic to manage the GoDaddy test account state (e.g., using `imap-flow` or another IMAP client directly within the adapter). This is non-trivial.
*   [ ] **Run Tests with GoDaddy Adapter:** Execute the existing Stdio/SSE test suites by selecting the `GoDaddyImapAdapter`.
*   [ ] **Refine Tests/Assertions:** Adjust tests or assertion logic if needed based on GoDaddy's specific behavior.

**Phase 5: Gherkin Integration (Optional but Recommended)**

*   [ ] **Add `cucumber` Dependency.**
*   [ ] **Create Feature Files:** Write `mcp_stdio.feature` and `mcp_sse.feature` based on the scenarios identified (examples provided above).
*   [ ] **Implement `World` Struct:** Hold the selected `Box<dyn ImapBackendAdapter>`.
*   [ ] **Implement Step Definitions:** Call methods on `world.adapter`.
*   [ ] **Configure Test Execution** via `cargo test`.
*   [ ] **Refactor/Expand Scenarios.**

**Phase 6: (Future) Add More Adapters (Gmail, Outlook)**

*   [ ] Implement new adapter structs (`GmailImapAdapter`, etc.).
*   [ ] Handle specific authentication (OAuth) and state management for each provider.
*   [ ] Run test suites against the new adapters. 