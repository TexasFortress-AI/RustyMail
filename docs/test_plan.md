# Test Plan for Unified REST + MCP Server

---

## **Principles**

- **Test-Driven Development (TDD)**
- **Integration tests use real IMAP server (no mocks)**
- **Unit tests for core logic**
- **Integration tests for REST and MCP interfaces**
- **Cross-interface consistency tests**
- **Security and performance tests**
- **Mocks allowed only in unit tests, forbidden in integration tests**

---

## **Test Types**

### **Unit Tests (`tests/unit/`)**

- **ImapClient behaviors**
  - List folders
  - Create/delete/rename folders
  - Select folder
  - Search emails
  - Fetch emails
  - Move emails
  - Logout
  - Error handling (invalid creds, network errors, etc.)
- **Business logic**
  - Email parsing
  - Folder stats
  - Data transformations
  - Error conversions
- **MCP protocol helpers**
  - JSON-RPC framing/parsing
  - Error code mapping
  - Capability negotiation logic
  - Prompt handling (if applicable)

---

### **Integration Tests (`tests/integration/`)**

#### **REST API (`rest_api.rs`)**

- List folders
- Create/delete/rename folders
- List emails
- List unread emails
- Get email by UID
- Move email
- Folder stats
- Error cases (bad requests, auth failures, etc.)

#### **MCP stdio server (`mcp_stdio.rs`)**

- Initialize connection
- Capability negotiation
- List tools/resources
- Call tools (list folders, fetch email, move email, etc.)
- Read resources (email content, folder list)
- Error cases and error code mapping
- Streaming responses (if applicable)
- Prompt handling (if applicable)

#### **MCP SSE server (`mcp_sse.rs`)**

- Initialize connection
- Capability negotiation
- List tools/resources
- Call tools
- Read resources
- Error cases and error code mapping
- Streaming responses
- Prompt handling (if applicable)

---

### **Cross-Interface Tests**

- Consistency of results between REST and MCP
- Error propagation consistency
- Performance comparisons

---

### **Security Tests**

- Authentication/authorization (if applicable)
- Input validation
- Error leakage (no sensitive info)
- Rate limiting (if applicable)

---

### **Performance Tests**

- Latency benchmarks
- Throughput under load
- Resource usage

---

## **Test Environment**

- Use **real IMAP server** (test account)
- Use **test containers** if possible
- No mocks in integration tests
- Isolate side effects (create/delete test folders/emails)
- Document IMAP test account setup and teardown

---

## **Implementation Notes**

- Write **failing tests first**
- Use `#[ignore]` or `todo!()` for unimplemented tests
- Gradually implement features to pass tests
- Maintain **high coverage** (aim >90% for core logic)
- Automate tests in **CI pipeline** (GitHub Actions, etc.)
- Run tests on **every pull request**

---

# END OF UPDATED TEST PLAN
