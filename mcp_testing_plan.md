# MCP End-to-End Testing Plan

## 1. Goal

Achieve comprehensive end-to-end (E2E) test coverage for RustyMail's primary interaction ports:

1.  **MCP APIs (Stdio & SSE):** Ensure programmatic interfaces function correctly against various IMAP backends.
2.  **Interactive Diagnostic Console:** Ensure human-oriented console access functions correctly via its adapters (initially Stdio and SSE Dashboard) against various IMAP backends.
3.  Implement and test the **SSE Diagnostic Web Dashboard**, which includes one adapter for the Interactive Diagnostic Console and uses a **specified modern frontend stack**.

## 2. Overall Strategy

1.  **Ports and Adapters (Application Interaction):** Define core interaction ports and their adapters (`StdioMCPAdapter`, `SseMCPAdapter`, `StdioConsoleAdapter`, `SseDashboardConsoleAdapter`).
2.  **Ports and Adapters (IMAP Backend):** Use an abstract `ImapBackendAdapter` (`MockImapAdapter`, `GoDaddyImapAdapter`, etc.), selected via configuration (`IMAP_ADAPTER` env var).
3.  **Dedicated Test Suites:** Organize tests (`mcp_stdio_api_...`, `mcp_sse_api_...`, `diagnostic_console_...`, `dashboard_ui_...`).
4.  **SSE Diagnostic Dashboard:** Integrated web UI hosted by the SSE server process.
    *   Provides stats, client lists, and an interactive console.
    *   Includes UI elements to **display the active IMAP Backend Adapter** and potentially allow selection for console interaction context (with browser persistence).
5.  **Application Subprocess:** Launch `rustymail`, configured for the mode being tested and the selected *IMAP Backend adapter*.
6.  **Client Simulation:** Use appropriate clients (stdin/stdout, HTTP/SSE, browser automation) for each interface (MCP API, Console Adapters, Dashboard UI).
7.  **Gherkin for Scenarios:** Define test cases covering programmatic API, console interactions, and dashboard UI validation.
8.  **Test Runner:** Utilize `cucumber` (or similar).

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

## 4. Interactive Diagnostic Console Port

*   **Purpose:** Provide a human-friendly way to send raw JSON-RPC commands to the core MCP engine and see the raw JSON-RPC responses.
*   **Core Logic:** A shared component/loop that:
    *   Receives a command string (JSON-RPC request).
    *   Parses the string.
    *   Validates basic JSON-RPC structure.
    *   Dispatches to the *same* MCP tool registry/execution logic used by the programmatic MCP API ports.
    *   Receives the result/error.
    *   Formats the JSON-RPC response string.
    *   Returns the response string.
*   **Adapters:**
    *   **`StdioConsoleAdapter`:** Reads command lines from stdin, passes to Core Logic, prints response lines to stdout. Activated by a specific runtime flag (e.g., `rustymail --console`).
    *   **`SseDashboardConsoleAdapter`:**
        *   *Backend:* An Actix endpoint (e.g., `/api/dashboard/console/command`) receives the command string via POST request body.
        *   *Backend:* Passes command to Core Logic, gets response string.
        *   *Backend:* Returns response string in the POST response body.
        *   *Frontend:* UI Javascript sends command string via fetch POST, receives response string, displays it in the console output area.
*   **Note:** The console logic itself executes commands against the *server's currently configured IMAP backend adapter*. The UI adapter may provide context or display which adapter is active.

## 5. SSE Diagnostic Web Dashboard

### 5.1. Core Concepts

*   **Purpose:** Provide a real-time web UI for monitoring and interaction.
*   **Hosting:** Served by `rustymail` in SSE mode. The Rust backend serves the dashboard as static files generated from the React frontend.
*   **Features:**
    *   Statistics Display: Real-time metrics (connection counts, request rates, performance metrics).
    *   Client List: Interactive list of connected SSE clients with details.
    *   Interactive Console: UI Frontend interacting with `SseDashboardConsoleAdapter` backend.
    *   **IMAP Adapter Display/Selector:** UI element (e.g., dropdown) showing the *currently active* IMAP Backend Adapter used by the server. User selection within the UI should be **persisted in the browser** (e.g., via `localStorage`) to remember preference, though it doesn't dynamically change the running server's backend connection in the initial implementation.
    *   Help text/examples for console commands.
*   **Internal Communication:** 
    *   Accesses shared state (metrics, `SseState`) and the console adapter backend endpoint. 
    *   Backend needs to expose which IMAP Adapter is active.
    *   Uses SSE for real-time updates (e.g., stats and client list changes).
    *   Exposes endpoints like `/api/dashboard/stats`, `/api/dashboard/clients`, `/api/dashboard/config`.

### 5.2. Frontend-Backend Integration

*   **Type Safety & Validation Bridge:**
    *   Frontend uses Zod for client-side validation and type inference.
    *   Zod schemas are converted to JSON Schema using `zod-to-json-schema` for backend use.
    *   Backend uses the Rust `jsonschema` crate to validate against the generated JSON Schemas.
    *   API contract defined using OpenAPI, referencing the JSON Schemas.
    *   TypeScript types generated from OpenAPI spec for frontend type safety.

### 5.3. Technology Stack (Required)

*   **UI Framework:** React (with Vite, no Next.js)
*   **UI Components:** `shadcn/ui` with Tailwind CSS for styling
*   **State Management/Data Fetching:** React Query for fetching and caching, with SSE for real-time updates
*   **Forms:** React Hook Form + Conform + Zod (for validation)
*   **Tables:** React Table (TanStack Table)
*   **Animation:** Framer Motion (`motion`)
*   **Date Utilities:** `date-fns`
*   **AI Integration:** Vercel AI SDK (`ai`)
*   **URL State:** `nuqs` (for managing state in search params if needed)
*   **Charts:** Recharts (for visualizing metrics)
*   **User Onboarding:** NextStepJS
*   **(Implied):** TypeScript, Tailwind CSS

### 5.4. UI Specification (Steve Jobs-Inspired UX)

*   **Philosophy:** Minimalist, intuitive, and focused on user clarity. "It just works."
*   **Layout:**
    *   **Top Bar:** Clean, thin header showing app name and IMAP Adapter selector with subtle dropdown animation.
    *   **Main Section:** Three primary panels arranged in a balanced grid layout:
        *   **Stats Panel:** Card-like containers with bold, legible metrics and subtle line charts that update in real-time.
        *   **Client List Panel:** A React Table with crisp typography, showing connected clients with smooth scrolling and subtle hover effects.
        *   **Console Panel:** Terminal-like interface with monospaced font, split between input (bottom) and output (scrollable area above), with light syntax highlighting for JSON.
*   **Colors:** Monochromatic base (white/gray backgrounds) with focused accent colors for interactive elements and data visualization.
*   **Interactions:**
    *   Buttons are simple, rounded rectangles with subtle hover and press effects.
    *   Real-time updates fade in smoothly without disrupting user focus.
    *   Console commands execute with immediate visual feedback.
    *   The IMAP selector remembers user's preference across sessions.
*   **Typography:** Clean, sans-serif font with proper spacing and hierarchy.
*   **Whitespace:** Generous spacing between elements to create a sense of order and clarity.
*   **Accessibility:** High contrast, keyboard navigation, and appropriate ARIA attributes.

### 5.5. Implementation Details

*   **Frontend Implementation:**
    *   **Setup:** Use Vite to scaffold a React + TypeScript app.
    *   **Components:**
        *   `StatsDisplay`: Fetches initial data and listens to SSE events for updates, renders with Recharts.
        *   `ClientList`: Uses React Table to display connected clients, updates in real-time via SSE.
        *   `InteractiveConsole`: Form for command input with React Hook Form, history tracking, and response formatting.
        *   `ImapAdapterSelector`: Dropdown showing current adapter with persistence via localStorage.
    *   **Data Flow:**
        *   React Query for data fetching with automatic refetching.
        *   `EventSource` for SSE real-time updates.
        *   Form submission with validation via Zod.
    *   **Build Process:** 
        *   Define validation schemas with Zod.
        *   Generate JSON Schema for backend with `zod-to-json-schema`.
        *   Bundle with Vite for optimized static assets.
*   **Backend Support:**
    *   **Static File Serving:** Actix Web's `Files` service to serve the frontend.
    *   **API Endpoints:**
        *   `/api/dashboard/stats`: Returns current metrics (JSON).
        *   `/api/dashboard/clients`: Returns client list (JSON).
        *   `/api/dashboard/config`: Returns active IMAP adapter configuration.
        *   `/api/dashboard/console/command`: Accepts JSON-RPC commands, validates with `jsonschema`, returns responses.
    *   **SSE Events:** Push events for real-time updates (e.g., `stats_updated`, `clients_updated`).
    *   **Validation:** Use generated JSON Schemas with the `jsonschema` crate.

## 6. Test Abstraction Layer (IMAP Backend)

*   Tests should interact with a trait or facade representing the `ImapBackendAdapter`.
*   This facade will expose methods like:
    *   `get_imap_connection_details() -> ImapConfig` (host, port, user, pass, etc. for `rustymail`)
    *   `setup_scenario(&ScenarioContext)` (e.g., configure mock responses, ensure live account state)
    *   `teardown_scenario(&ScenarioContext)` (e.g., stop mock, clean up live account)
    *   `verify_interaction(&ExpectedInteraction)` (e.g., query mock, potentially NOOP for live)
*   The test runner or `World` struct in Cucumber will hold an instance of the selected adapter.

## 7. E2E Test Suites & Scenarios

### 7.1. `mcp_stdio_api_e2e_test.rs`

*   **Focus:** Testing the `StdioMCPAdapter` for programmatic use.
*   **Gherkin (`mcp_stdio_api.feature`):** Scenarios similar to the *original* Stdio examples (list, select, search, fetch, errors) focusing on structured request/response validation.

### 7.2. `mcp_sse_api_e2e_test.rs`

*   **Focus:** Testing the `SseMCPAdapter` for programmatic use.
*   **Gherkin (`mcp_sse_api.feature`):** Scenarios similar to the *original* SSE examples (connect, heartbeat, commands via POST, validating SSE `tool_result`/`tool_error` events).

### 7.3. `diagnostic_console_e2e_test.rs`

*   **Focus:** Testing the `Interactive Diagnostic Console Port` via its different adapters.
*   **Gherkin (`diagnostic_console.feature`):**

    ```gherkin
    Feature: Interactive Diagnostic Console E2E Tests

      # --- Testing via Stdio Adapter --- 
      Scenario: Execute listFolders via Stdio Console
        Given the selected IMAP backend adapter is configured for LIST
        And the RustyMail process is started in stdio console mode using the adapter configuration
        When the command string '{"jsonrpc": "2.0", "id": "con-1", "method": "imap/listFolders"}' is sent via stdin
        Then a response line containing the JSON-RPC result for id "con-1" with folders should be received via stdout
        And the adapter specific verification confirms the LIST interaction

      Scenario: Execute failing command via Stdio Console
        Given the selected IMAP backend adapter is configured for SELECT "BadFolder" to fail
        And the RustyMail process is started in stdio console mode using the adapter configuration
        When the command string '{"jsonrpc": "2.0", "id": "con-2", "method": "imap/selectFolder", "params": {"folder_name": "BadFolder"}}' is sent via stdin
        Then a response line containing the JSON-RPC error for id "con-2" with code -32010 should be received via stdout
        And the adapter specific verification confirms the failed SELECT interaction

      # --- Testing via SSE Dashboard Adapter --- 
      Scenario: Execute listFolders via SSE Dashboard Console Backend
        Given the selected IMAP backend adapter is configured for LIST
        And the RustyMail SSE process (with dashboard) is started using the adapter configuration
        When an HTTP POST request with body '{"jsonrpc": "2.0", "id": "dash-1", "method": "imap/listFolders"}' is sent to "/api/dashboard/console/command"
        Then the response status should be 200 OK
        And the response body should be a JSON-RPC result for id "dash-1" containing folders
        And the adapter specific verification confirms the LIST interaction

      Scenario: Execute failing command via SSE Dashboard Console Backend
        Given the selected IMAP backend adapter is configured for SELECT "BadFolder" to fail
        And the RustyMail SSE process (with dashboard) is started using the adapter configuration
        When an HTTP POST request with body '{"jsonrpc": "2.0", "id": "dash-2", "method": "imap/selectFolder", "params": {"folder_name": "BadFolder"}}' is sent to "/api/dashboard/console/command"
        Then the response status should be 200 OK
        And the response body should be a JSON-RPC error for id "dash-2" with code -32010
        And the adapter specific verification confirms the failed SELECT interaction
    ```

### 7.4. `dashboard_ui_e2e_test.rs`

*   **Focus:** Testing UI elements and interactions of the SSE dashboard (requires browser automation).
*   **Gherkin (`dashboard_ui.feature`):**

    ```gherkin
    Feature: SSE Diagnostic Dashboard UI E2E Tests

      Background:
        Given the selected IMAP backend adapter is configured for basic connection
        And the RustyMail SSE process (with dashboard) is started using the adapter configuration
        And the user navigates to the dashboard page

      Scenario: Dashboard is accessible and has title
        Then the page title should be "RustyMail SSE Dashboard"

      Scenario: Dashboard displays the correct active IMAP Adapter
        # Assuming server started with Mock adapter for this test
        Then the IMAP Adapter selector should display "Mock"

      Scenario: Dashboard persists selected IMAP Adapter preference
        # This tests UI persistence, not changing the live backend
        Given the IMAP Adapter selector shows "Mock"
        When the user selects "GoDaddy" in the IMAP Adapter selector
        Then the IMAP Adapter selector should display "GoDaddy"
        When the user reloads the dashboard page
        Then the IMAP Adapter selector should still display "GoDaddy"

      Scenario: Dashboard shows initial client count as 0
        Then the dashboard should display "0" connected clients in the stats panel

      Scenario: Dashboard client count updates after SSE client connects
        When an SSE client connects
        Then eventually the dashboard should display "1" connected client in the stats panel
        
      Scenario: Interactive console sends command and displays response
        When the user enters '{"jsonrpc": "2.0", "id": "ui-1", "method": "imap/listFolders"}' in the console input
        And the user clicks the "Send" button
        Then the console output should eventually contain a JSON-RPC result for id "ui-1" with folders

      Scenario: Stats display updates in real-time via SSE
        When the server pushes an SSE event with updated stats
        Then the stats panel should display the updated metrics
        
      Scenario: Dashboard displays correct typography and spacing
        Then all text elements should use the specified sans-serif font
        And all panels should have consistent padding and spacing
        
      Scenario: Dashboard animations work correctly
        When the user hovers over an interactive element
        Then the element should display the correct hover effect
        When an SSE event updates the stats
        Then the updated values should transition smoothly
    ```

## 8. Gherkin Integration Options (Rust)

*   **Primary Option:** [`cucumber`](https://crates.io/crates/cucumber)
    *   The `World` struct will hold the adapter, subprocess handles, HTTP/SSE clients, and potentially a browser driver (`fantoccini::Client` or similar).
    *   Requires adding browser automation crates (`fantoccini`, `webdriver`) and managing WebDriver instances (e.g., geckodriver, chromedriver) if testing UI interactions.

## 9. Implementation Checklist (Revised)

**Phase 1: Foundation & Abstraction**

*   [ ] Define `ImapBackendAdapter` Trait.
*   [ ] Implement `MockImapAdapter` (incl. Mock Server).
*   [ ] Refactor `main.rs`/`cli.rs` (for config flexibility, add `--console` mode flag).
*   [ ] Implement IMAP Adapter Selection Logic.
*   [ ] **Define & Implement Core Diagnostic Console Logic:** Component that takes command string -> executes -> returns response string.
*   [ ] Add Test Utilities Crate (Optional).

**Phase 2: Stdio Adapters**

*   [ ] **Implement `StdioMCPAdapter`:** For programmatic MCP API.
*   [ ] **Implement `StdioConsoleAdapter`:** For interactive console via stdin/stdout (using core console logic).

**Phase 3: SSE Adapters & Dashboard**

*   [ ] **Implement `SseMCPAdapter`:** For programmatic MCP API (POST endpoint, SSE events).
*   [ ] **Design Dashboard API:** Define Actix routes/handlers (incl. `/api/dashboard/console/command`).
*   [ ] **Implement `SseDashboardConsoleAdapter` Backend:** Actix handler using core console logic.
*   [ ] **Instrument Application:** Add metric collection.
*   [ ] **Implement Dashboard Backend Logic:** Handlers for UI, stats, clients.
*   [ ] **Implement Dashboard Frontend UI with Steve Jobs-inspired UX:**
    *   [ ] Setup React with Vite, TypeScript, Tailwind CSS.
    *   [ ] Define Zod schemas and generate JSON Schema with `zod-to-json-schema`.
    *   [ ] Install required libraries: `shadcn/ui`, `react-hook-form`, `conform`, `zod`, `@tanstack/react-table`, `@tanstack/react-query`, `framer-motion`, `date-fns`, `ai`, `nuqs`, `recharts`, `nextstepjs`.
    *   [ ] Design system setup: Colors, typography, spacing, animations.
    *   [ ] Create core components: `StatsDisplay`, `ClientList`, `InteractiveConsole`, `ImapAdapterSelector`.
    *   [ ] Implement real-time updates with SSE (`EventSource`).
    *   [ ] Implement IMAP Adapter display/selector with persistence (`localStorage`).
    *   [ ] Build with Vite for optimized static assets.

**Phase 4: E2E Tests (Mock Adapter)**

*   [ ] Setup Test Files (`mcp_stdio_api_...`, `mcp_sse_api_...`, `diagnostic_console_...`, `dashboard_ui_...`).
*   [ ] Add Dependencies (`reqwest`, SSE client, browser automation).
*   [ ] **Implement Stdio API Tests:** (`mcp_stdio_api_e2e_test.rs`)
*   [ ] **Implement SSE API Tests:** (`mcp_sse_api_e2e_test.rs`)
*   [ ] **Implement Diagnostic Console Tests:** (`diagnostic_console_e2e_test.rs` - testing both Stdio and SSE adapters).
*   [ ] **Implement Dashboard UI Tests:**
    *   [ ] Add tests for basic accessibility.
    *   [ ] Add tests for IMAP adapter display and persistence.
    *   [ ] Add tests for stats/client list display.
    *   [ ] Add tests for console UI interaction (if needed beyond console logic tests).

**Phase 5: `GoDaddyImapAdapter` Implementation**

*   [ ] Implement `GoDaddyImapAdapter` struct.
*   [ ] Configuration Loading.
*   [ ] State Management.
*   [ ] **Run E2E Tests** (API & Console suites) with GoDaddy Adapter.
*   [ ] Refine Tests/Assertions.

**Phase 6: Gherkin Integration (Optional but Recommended)**

*   [ ] Add `cucumber` Dependency (+ browser automation).
*   [ ] Create/Update Feature Files.
*   [ ] Implement `World` Struct.
*   [ ] Implement Step Definitions (API, Console, UI steps).
*   [ ] Configure Test Execution.
*   [ ] Refactor/Expand Scenarios.

**Phase 7: (Future) Add More Adapters (Gmail, Outlook, REST Console)**

*   [ ] Implement new adapter structs (`GmailImapAdapter`, etc.).
*   [ ] Handle specific authentication (OAuth) and state management for each provider.
*   [ ] Run test suites against the new adapters.

## 9. SSE Diagnostic Web Dashboard Specification

### 9.1. Core Concepts

The SSE Diagnostic Web Dashboard provides a real-time UI for monitoring and interacting with the RustyMail SSE server. Key concepts include:

* **Real-time UI**: Dashboard updates dynamically as SSE events occur, with no need for manual refresh
* **Interactive Console**: Direct JSON-RPC command interface for testing and diagnostics
* **Persistent Preferences**: Dashboard remembers user preferences via browser localStorage
* **Non-intrusive**: Dashboard is optional and does not impact core SSE functionality
* **Validation Bridge**: Frontend and backend share compatible validation through Zod and JSON Schema

### 9.2. Hosting

The dashboard is served by the RustyMail SSE server itself at the `/dashboard` endpoint when the `--dashboard` flag is enabled. No separate deployment process is needed.

### 9.3. Features

* **Connection Stats**: Display of active connection counts, message rates, and performance metrics
* **Client Inspector**: Detailed view of connected clients with filtering capabilities
* **Command Console**: JSON-RPC interactive console with syntax highlighting and validation
* **Adapter Selector**: UI to switch between configured IMAP adapters for testing
* **Event Stream**: Real-time display of server events with filtering options
* **Notification System**: Alerts for important server state changes

### 9.4. Internal Communication

* Dashboard receives updates through an internal SSE connection to the server
* Commands from the console are sent via HTTP requests to the JSON-RPC endpoint
* State persistence uses localStorage, with no server-side state maintained for UI

### 9.5. Frontend-Backend Integration

* **Schema Definition**: Backend exposes JSON Schema for all commands via endpoint
* **TypeScript Types**: Frontend generates TypeScript interfaces from JSON Schema
* **Validation Bridge**: Frontend uses Zod schemas derived from TypeScript interfaces
* **OpenAPI**: API contract defined in OpenAPI spec accessible at `/api-docs`
* **Error Handling**: Consistent error format between frontend and backend

### 9.6. Technology Stack

#### Frontend
* **Framework**: React 18+ with Vite (not Next.js)
* **UI Components**: shadcn/ui with Tailwind CSS
* **State Management**: React Query for server state, Zustand for local state
* **Form Handling**: React Hook Form with Zod validation
* **HTTP Client**: Axios with interceptors for error handling
* **Code Editor**: Monaco Editor for JSON-RPC console
* **Data Visualization**: Recharts for metrics display
* **Animations**: Framer Motion for smooth transitions

#### Backend Integration
* **SSE Client**: EventSource for real-time updates
* **OpenAPI**: OpenAPI specification for API documentation
* **Type Generation**: json-schema-to-ts for schema conversion
* **Validation**: Zod for frontend validation, JSON Schema for backend

### 9.7. UI Specification

The dashboard UI follows these design principles inspired by Steve Jobs' UX philosophy:

* **Minimalist**: Clean, uncluttered interface with ample white space
* **Intuitive**: Self-explanatory UI elements that require minimal documentation
* **Focused**: Each view serves a distinct purpose without overwhelming features
* **Typography**: Clear hierarchical typography with sans-serif fonts
* **Color**: Restrained color palette with accent colors only for important information
* **Animation**: Subtle animations that enhance understanding of state changes
* **Consistency**: UI patterns repeated throughout the application

The layout consists of:
1. Top navigation bar with title and adapter selector
2. Stats summary cards with key metrics
3. Main content area with tabs for different features
4. Bottom status bar with connection information
5. Right sidebar with context-sensitive help (collapsible)

### 9.8. Implementation Details

#### Frontend Implementation
* Component structure follows atomic design principles
* Strict TypeScript with proper typing of all API responses
* Custom hooks for SSE connection and command execution
* Error boundaries for fault isolation
* Responsive design supporting desktop and tablet viewports
* Accessible components with ARIA attributes and keyboard navigation

#### Backend Support
* Static file serving for dashboard assets
* SSE endpoint with filtered events for UI consumption
* JSON Schema endpoint for automatic type generation
* CORS configuration to support development mode
* OpenAPI documentation generation 