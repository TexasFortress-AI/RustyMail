# MCP End-to-End Testing Plan v.2.2

## 1. Goal

Achieve comprehensive end-to-end (E2E) test coverage for RustyMail's primary interaction ports:

1.  **MCP Library Refactor:** Migrate to the official Rust SDK from [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) for standardized protocol implementation.
2.  **MCP APIs (Stdio & SSE):** Ensure programmatic interfaces function correctly against various IMAP backends.
3.  **RIG-Powered AI Chatbot:** Implement and test a natural language interface to RustyMail's MCP client that can connect to both SSE and stdio MCP servers.
4.  **SSE Diagnostic Web Dashboard:** Implement and test a modern web dashboard that includes statistics, client management, and the AI Chatbot.

## 2. Overall Strategy

1.  **MCP Library Refactor:** Replace custom MCP implementation with the official [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) to ensure standard compliance, maintainability, and benefit from community updates.
2.  **Ports and Adapters (Application Interaction):** Define core interaction ports and their adapters (`StdioMCPAdapter`, `SseMCPAdapter`).
3.  **Ports and Adapters (IMAP Backend):** Use an abstract `ImapBackendAdapter` (`MockImapAdapter`, `GoDaddyImapAdapter`, etc.), selected via configuration (`IMAP_ADAPTER` env var).
4.  **Dedicated Test Suites:** Organize tests (`mcp_stdio_api_...`, `mcp_sse_api_...`, `ai_chatbot_...`, `dashboard_ui_...`).
5.  **RIG Integration:** Integrate RIG with the AI chatbot for LLM inference using openrouter/deepseek-v3-free as the initial provider.
6.  **SSE Diagnostic Dashboard:** Integrated web UI hosted by the SSE server process.
    *   Provides stats, client lists, and the AI Chatbot interface.
    *   Includes UI elements to **display the active IMAP Backend Adapter** and potentially allow selection for console interaction context (with browser persistence).
7.  **Application Subprocess:** Launch `rustymail`, configured for the mode being tested and the selected *IMAP Backend adapter*.
8.  **Client Simulation:** Use appropriate clients (stdin/stdout, HTTP/SSE, browser automation) for each interface (MCP API, AI Chatbot, Dashboard UI).
9.  **Gherkin for Scenarios:** Define test cases covering programmatic API, chatbot interactions, and dashboard UI validation.
10. **Test Runner:** Utilize `cucumber` (or similar).

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

## 4. MCP Library Refactor

### 4.1. Purpose

Migrate the existing custom MCP implementation to the official [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) to ensure standard compliance, maintainability, and benefit from community updates.

### 4.2. Integration Strategy

* **Dependency Management:** Add rust-sdk as a direct dependency in Cargo.toml
* **Transport Migration:** Replace custom transport implementations with sdk-provided ones:
  * `TokioChildProcess` for stdio interactions
  * SSE transport for web-based communication
* **Service Definition:** Refactor existing MCP service definitions using rust-sdk tooling:
  * Use `toolbox` and `tool` macros to define MCP tools
  * Implement handlers compliant with rust-sdk interfaces
* **Type Mapping:** Update type definitions to match rust-sdk requirements

### 4.3. Benefits

* **Standards Compliance:** Ensures adherence to the latest MCP specification
* **Reduced Maintenance Burden:** Leverage community-maintained implementation
* **Improved Interoperability:** Easier integration with other MCP-compliant systems
* **Enhanced Features:** Access to additional capabilities provided by the SDK

## 5. RIG-Powered AI Chatbot

### 5.1. Purpose

Provide a natural language conversational interface to RustyMail's MCP client, allowing users to perform email operations through chat-based interactions with an AI assistant.

### 5.2. Core Architecture

*   **Chatbot Engine:** 
    *   Natural language processing and understanding powered by RIG.
    *   Context-aware conversation management.
    *   Integration with MCP clients (both SSE and stdio).
*   **Core Logic:** A centralized component that:
    *   Receives natural language queries from the user.
    *   Processes with RIG LLM to understand user intent.
    *   Generates appropriate MCP tool calls to retrieve or manipulate email data.
    *   Formats responses in natural, conversational language.
    *   Maintains conversation history and context for follow-up questions.

### 5.3. RIG Integration

*   **LLM Provider:** Initial implementation with openrouter/deepseek-v3-free (configurable via env).
*   **Tool Definitions:** JSON schema definitions of available MCP tools for LLM to invoke.
*   **Context Management:** Track conversation history and user preferences.
*   **Configuration:** API key stored in .env file with configurable model parameters.

### 5.4. Features

*   **Natural Language Understanding:** Interprets email-related queries.
*   **Context Awareness:** Remembers previous messages in conversation.
*   **Operation Translation:** Converts natural language to appropriate MCP operations.
*   **Response Formatting:** Presents MCP responses in human-readable conversational form.
*   **Error Handling:** Clear explanations of errors in user-friendly language.
*   **Multi-Transport Support:** Can connect to both SSE and stdio MCP servers.

### 5.5. Adapters

*   **`StdioAIChatbotAdapter`:** Reads natural language queries from stdin, processes via Core Logic, prints conversational responses to stdout.
*   **`SseAIChatbotAdapter`:**
    *   *Backend:* An Actix endpoint (e.g., `/api/dashboard/chatbot/query`) receives natural language queries via POST request body.
    *   *Backend:* Passes input to Core Logic, gets response.
    *   *Backend:* Returns conversational response in the POST response body.
    *   *Frontend:* UI component sends query via fetch POST, receives response, displays it in a chat interface.

### 5.6. Security

*   **Authentication:** Requirements for sensitive operations.
*   **Data Privacy:** No storage of email content beyond session context.
*   **Permission Levels:** Configurable access controls.

## 6. SSE Diagnostic Web Dashboard

### 6.1. Core Concepts

*   **Purpose:** Provide a real-time web UI for monitoring and interaction.
*   **Hosting:** Served by `rustymail` in SSE mode. The Rust backend serves the dashboard as static files generated from the React frontend.
*   **Features:**
    *   **Statistics Display:** Real-time metrics (connection counts, request rates, performance metrics).
    *   **Client List:** Interactive list of connected SSE clients with details.
    *   **AI Chatbot Interface:** A conversational UI for email operations via natural language.
    *   **IMAP Adapter Display/Selector:** UI element showing the *currently active* IMAP Backend Adapter used by the server. User selection within the UI should be **persisted in the browser** (e.g., via `localStorage`).
    *   **Help Text/Examples:** Context-sensitive help for chatbot interaction.
*   **Internal Communication:** 
    *   Accesses shared state (metrics, `SseState`) and the chatbot adapter backend endpoint. 
    *   Backend needs to expose which IMAP Adapter is active.
    *   Uses SSE for real-time updates (e.g., stats and client list changes).
    *   Exposes endpoints like `/api/dashboard/stats`, `/api/dashboard/clients`, `/api/dashboard/config`.

### 6.2. Frontend-Backend Integration

*   **Type Safety & Validation Bridge:**
    *   Frontend uses Zod for client-side validation and type inference.
    *   Zod schemas are converted to JSON Schema using `zod-to-json-schema` for backend use.
    *   Backend uses the Rust `jsonschema` crate to validate against the generated JSON Schemas.
    *   API contract defined using OpenAPI, referencing the JSON Schemas.
    *   TypeScript types generated from OpenAPI spec for frontend type safety.

### 6.3. Technology Stack (Required)

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

### 6.4. UI Specification (Steve Jobs-Inspired UX)

*   **Philosophy:** Minimalist, intuitive, and focused on user clarity. "It just works."
*   **Layout:**
    *   **Top Bar:** Clean, thin header showing app name and IMAP Adapter selector with subtle dropdown animation.
    *   **Main Section:** Three primary panels arranged in a balanced grid layout:
        *   **Stats Panel:** Card-like containers with bold, legible metrics and subtle line charts that update in real-time.
        *   **Client List Panel:** A React Table with crisp typography, showing connected clients with smooth scrolling and subtle hover effects.
        *   **AI Chatbot Panel:** Chat-like interface with clear distinction between user and AI messages, featuring:
            *   Message history with timestamps
            *   User-friendly input area with send button
            *   Elegant loading animations during processing
            *   Formatted message display for improved readability
*   **Colors:** Monochromatic base (white/gray backgrounds) with focused accent colors for interactive elements and data visualization.
*   **Interactions:**
    *   Buttons are simple, rounded rectangles with subtle hover and press effects.
    *   Real-time updates fade in smoothly without disrupting user focus.
    *   Chat messages appear with subtle animations.
    *   The IMAP selector remembers user's preference across sessions.
*   **Typography:** Clean, sans-serif font with proper spacing and hierarchy.
*   **Whitespace:** Generous spacing between elements to create a sense of order and clarity.
*   **Accessibility:** High contrast, keyboard navigation, and appropriate ARIA attributes.

### 6.5. Implementation Details

*   **Frontend Implementation:**
    *   **Setup:** Use Vite to scaffold a React + TypeScript app.
    *   **Components:**
        *   `StatsDisplay`: Fetches initial data and listens to SSE events for updates, renders with Recharts.
        *   `ClientList`: Uses React Table to display connected clients, updates in real-time via SSE.
        *   `AIChatbot`: Chat interface component with message history, input area, and message display.
        *   `ImapAdapterSelector`: Dropdown showing current adapter with persistence via localStorage.
    *   **Data Flow:**
        *   React Query for data fetching with automatic refetching.
        *   `EventSource` for SSE real-time updates.
        *   Form submission with validation via Zod.
        *   AI interactions using Vercel AI SDK.
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
        *   `/api/dashboard/chatbot/query`: Accepts natural language queries, processes with LLM, returns conversational responses.
    *   **SSE Events:** Push events for real-time updates (e.g., `stats_updated`, `clients_updated`).
    *   **Validation:** Use generated JSON Schemas with the `jsonschema` crate.
    *   **RIG Integration:** Configure RIG for LLM inference with appropriate tool definitions.

## 7. Test Abstraction Layer (IMAP Backend)

*   Tests should interact with a trait or facade representing the `ImapBackendAdapter`.
*   This facade will expose methods like:
    *   `get_imap_connection_details() -> ImapConfig` (host, port, user, pass, etc. for `rustymail`)
    *   `setup_scenario(&ScenarioContext)` (e.g., configure mock responses, ensure live account state)
    *   `teardown_scenario(&ScenarioContext)` (e.g., stop mock, clean up live account)
    *   `verify_interaction(&ExpectedInteraction)` (e.g., query mock, potentially NOOP for live)
*   The test runner or `World` struct in Cucumber will hold an instance of the selected adapter.

## 8. E2E Test Suites & Scenarios

### 8.1. `mcp_stdio_api_e2e_test.rs`

*   **Focus:** Testing the `StdioMCPAdapter` for programmatic use.
*   **Gherkin (`mcp_stdio_api.feature`):** Scenarios similar to the *original* Stdio examples (list, select, search, fetch, errors) focusing on structured request/response validation.

### 8.2. `mcp_sse_api_e2e_test.rs`

*   **Focus:** Testing the `SseMCPAdapter` for programmatic use.
*   **Gherkin (`mcp_sse_api.feature`):** Scenarios similar to the *original* SSE examples (connect, heartbeat, commands via POST, validating SSE `tool_result`/`tool_error` events).

### 8.3. `ai_chatbot_e2e_test.rs`

*   **Focus:** Testing the AI Chatbot via its different adapters.
*   **Gherkin (`ai_chatbot.feature`):**

    ```gherkin
    Feature: AI Chatbot E2E Tests

      # --- Testing via Stdio Adapter --- 
      Scenario: Query inbox via natural language in Stdio Chatbot
        Given the selected IMAP backend adapter is configured with test emails
        And the RustyMail process is started in stdio chatbot mode using the adapter configuration
        When the natural language query "How many emails are in my inbox?" is sent via stdin
        Then a response line with a conversational answer containing the correct number of emails should be received via stdout
        And the adapter specific verification confirms the proper IMAP interactions

      Scenario: Follow-up query maintains context in Stdio Chatbot
        Given the selected IMAP backend adapter is configured with test emails from "example@email.com"
        And the RustyMail process is started in stdio chatbot mode using the adapter configuration
        When the natural language query "Find emails from example@email.com" is sent via stdin
        And then the natural language query "When was the most recent one sent?" is sent via stdin
        Then a response line containing the date of the most recent email should be received via stdout
        And the adapter specific verification confirms the conversation context was maintained

      # --- Testing via SSE Dashboard Adapter --- 
      Scenario: Query inbox via natural language in Dashboard Chatbot
        Given the selected IMAP backend adapter is configured with test emails
        And the RustyMail SSE process (with dashboard) is started using the adapter configuration
        When an HTTP POST request with body '{"query": "How many emails are in my inbox?"}' is sent to "/api/dashboard/chatbot/query"
        Then the response status should be 200 OK
        And the response body should contain a conversational answer with the correct number of emails
        And the adapter specific verification confirms the proper IMAP interactions

      Scenario: Maintain conversation context in Dashboard Chatbot
        Given the selected IMAP backend adapter is configured with test emails from "example@email.com"
        And the RustyMail SSE process (with dashboard) is started using the adapter configuration
        When an HTTP POST request with body '{"query": "Find emails from example@email.com"}' is sent to "/api/dashboard/chatbot/query"
        And then an HTTP POST request with body '{"query": "When was the most recent one sent?"}' is sent to "/api/dashboard/chatbot/query"
        Then the response status should be 200 OK
        And the response body should contain a date that matches the most recent email from "example@email.com"
        And the adapter specific verification confirms the conversation context was maintained
    ```

### 8.4. `dashboard_ui_e2e_test.rs`

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
        
      Scenario: AI Chatbot responds to user queries
        When the user enters "How many emails are in my inbox?" in the chatbot input
        And the user clicks the "Send" button
        Then the chatbot output should eventually contain a conversational answer with the inbox count
        
      Scenario: AI Chatbot maintains conversation context
        When the user enters "Find emails from example@email.com" in the chatbot input
        And the user clicks the "Send" button
        And the user enters "Which one is the most recent?" in the chatbot input
        And the user clicks the "Send" button
        Then the chatbot output should eventually identify the most recent email from "example@email.com"
        
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
        
      Scenario: Chatbot messages animate smoothly
        When the user sends a message in the chatbot
        Then the message should appear with a smooth animation
        And the AI response should appear with a typing indicator followed by message display
    ```

## 9. Gherkin Integration Options (Rust)

*   **Primary Option:** [`cucumber`](https://crates.io/crates/cucumber)
    *   The `World` struct will hold the adapter, subprocess handles, HTTP/SSE clients, and potentially a browser driver (`fantoccini::Client` or similar).
    *   Requires adding browser automation crates (`fantoccini`, `webdriver`) and managing WebDriver instances (e.g., geckodriver, chromedriver) if testing UI interactions.

## 10. Implementation Checklist (Revised)

**Phase 1: MCP Library Refactor**

*   [ ] Add rust-sdk dependency to Cargo.toml
*   [ ] Migrate custom transport implementations to sdk-provided ones
*   [ ] Refactor existing MCP service definitions using rust-sdk tooling
*   [ ] Update type definitions to match rust-sdk requirements
*   [ ] Create compatibility layer if needed for smooth transition
*   [ ] Implement tests to verify refactored functionality

**Phase 2: Foundation & Adapters**

*   [ ] Define `ImapBackendAdapter` Trait.
*   [ ] Implement `MockImapAdapter` (incl. Mock Server).
*   [ ] Refactor `main.rs`/`cli.rs` (for config flexibility).
*   [ ] Implement IMAP Adapter Selection Logic.
*   [ ] Add Test Utilities Crate (Optional).
*   [ ] **Implement `StdioMCPAdapter`:** For programmatic MCP API.
*   [ ] **Implement `SseMCPAdapter`:** For programmatic MCP API (POST endpoint, SSE events).

**Phase 3: RIG Integration & AI Chatbot**

*   [ ] Set up RIG integration for LLM inference.
*   [ ] Configure openrouter/deepseek-v3-free as initial provider.
*   [ ] Implement environment-based configuration (API keys in .env).
*   [ ] Create core chatbot logic:
    *   [ ] Natural language processing pipeline
    *   [ ] MCP tool definitions for LLM
    *   [ ] Conversation context management
    *   [ ] Response formatting
*   [ ] Implement `StdioAIChatbotAdapter` for terminal interaction
*   [ ] Implement `SseAIChatbotAdapter` for web dashboard integration
*   [ ] Develop security and permissions system

**Phase 4: Dashboard Implementation**

*   [ ] **Design Dashboard API:** Define Actix routes/handlers.
*   [ ] **Instrument Application:** Add metric collection.
*   [ ] **Implement Dashboard Backend Logic:** Handlers for UI, stats, clients.
*   [ ] **Implement Dashboard Frontend UI with Steve Jobs-inspired UX:**
    *   [ ] Setup React with Vite, TypeScript, Tailwind CSS.
    *   [ ] Define Zod schemas and generate JSON Schema with `zod-to-json-schema`.
    *   [ ] Install required libraries: `shadcn/ui`, `react-hook-form`, `conform`, `zod`, `@tanstack/react-table`, `@tanstack/react-query`, `framer-motion`, `date-fns`, `ai`, `nuqs`, `recharts`, `nextstepjs`.
    *   [ ] Design system setup: Colors, typography, spacing, animations.
    *   [ ] Create core components: `StatsDisplay`, `ClientList`, `AIChatbot`, `ImapAdapterSelector`.
    *   [ ] Implement real-time updates with SSE (`EventSource`).
    *   [ ] Implement IMAP Adapter display/selector with persistence (`localStorage`).
    *   [ ] Create chat interface with message history display.
    *   [ ] Integrate Vercel AI SDK for frontend chatbot interactions.
    *   [ ] Build with Vite for optimized static assets.

**Phase 5: E2E Tests**

*   [ ] Setup Test Files.
*   [ ] Add Dependencies (`reqwest`, SSE client, browser automation).
*   [ ] **Implement Stdio API Tests:** (`mcp_stdio_api_e2e_test.rs`)
*   [ ] **Implement SSE API Tests:** (`mcp_sse_api_e2e_test.rs`)
*   [ ] **Implement AI Chatbot Tests:** (`ai_chatbot_e2e_test.rs`)
    *   [ ] Add tests for natural language understanding
    *   [ ] Add tests for email operations through conversation
    *   [ ] Add tests for context maintenance
    *   [ ] Add tests for security features
*   [ ] **Implement Dashboard UI Tests:**
    *   [ ] Add tests for basic accessibility
    *   [ ] Add tests for IMAP adapter display and persistence
    *   [ ] Add tests for stats/client list display
    *   [ ] Add tests for chatbot UI interaction
    *   [ ] Add tests for animations and transitions

**Phase 6: `GoDaddyImapAdapter` Implementation**

*   [ ] Implement `GoDaddyImapAdapter` struct.
*   [ ] Configuration Loading.
*   [ ] State Management.
*   [ ] **Run E2E Tests** (API, Chatbot, UI suites) with GoDaddy Adapter.
*   [ ] Refine Tests/Assertions.

**Phase 7: Gherkin Integration (Optional but Recommended)**

*   [ ] Add `cucumber` Dependency (+ browser automation).
*   [ ] Create/Update Feature Files.
*   [ ] Implement `World` Struct.
*   [ ] Implement Step Definitions (API, Chatbot, UI steps).
*   [ ] Configure Test Execution.
*   [ ] Refactor/Expand Scenarios.

**Phase 8: (Future) Add More Adapters & Features**

*   [ ] Implement new adapter structs (`GmailImapAdapter`, etc.).
*   [ ] Handle specific authentication (OAuth) and state management for each provider.
*   [ ] Run test suites against the new adapters.
*   [ ] Expand AI capabilities with advanced LLM features.

## 11. SSE Diagnostic Web Dashboard Specification

### 11.1. Core Concepts

The SSE Diagnostic Web Dashboard provides a real-time UI for monitoring and interacting with the RustyMail SSE server. Key concepts include:

* **Real-time UI**: Dashboard updates dynamically as SSE events occur, with no need for manual refresh
* **AI Chatbot**: Natural language interface for interacting with email system
* **Persistent Preferences**: Dashboard remembers user preferences via browser localStorage
* **Non-intrusive**: Dashboard is optional and does not impact core SSE functionality
* **Validation Bridge**: Frontend and backend share compatible validation through Zod and JSON Schema

### 11.2. Hosting

The dashboard is served by the RustyMail SSE server itself at the `/dashboard` endpoint when the `--dashboard` flag is enabled. No separate deployment process is needed.

### 11.3. Features

* **Connection Stats**: Display of active connection counts, message rates, and performance metrics
* **Client Inspector**: Detailed view of connected clients with filtering capabilities
* **AI Chatbot**: Conversational email assistant with natural language understanding
* **Adapter Selector**: UI to switch between configured IMAP adapters for testing
* **Event Stream**: Real-time display of server events with filtering options
* **Notification System**: Alerts for important server state changes

### 11.4. Internal Communication

* Dashboard receives updates through an internal SSE connection to the server
* Chatbot queries are sent via HTTP requests to dedicated endpoints
* State persistence uses localStorage, with no server-side state maintained for UI
* RIG handles LLM inference for natural language processing

### 11.5. Frontend-Backend Integration

* **Schema Definition**: Backend exposes JSON Schema for all commands via endpoint
* **TypeScript Types**: Frontend generates TypeScript interfaces from JSON Schema
* **Validation Bridge**: Frontend uses Zod schemas derived from TypeScript interfaces
* **OpenAPI**: API contract defined in OpenAPI spec accessible at `/api-docs`
* **Error Handling**: Consistent error format between frontend and backend 