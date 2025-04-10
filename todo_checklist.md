## Phase 3: Fix Duplicate Definitions

### 1. Fix Duplicate Type Definitions
- [x] Fix duplicate `Arc` definition in `src/api/mcp_stdio.rs` (E0252)
  - Time: 10 minutes
  - Notes: Removed duplicate import of `Arc` from `std::sync`

- [x] Fix duplicate `StoreOperation` definition in `src/imap/session.rs` (E0255)
  - Time: 15 minutes
  - Notes: Updated `store_flags` method to use `FlagOperation` instead of local `StoreOperation` enum

- [x] Fix duplicate `append` method in `src/imap/session.rs` (E0201)
  - Time: 20 minutes
  - Notes: Updated `ImapSession::append` to properly delegate to `AsyncImapOps::append` with empty flags

- [x] Fix duplicate `move_email` method in `src/imap/session.rs` (E0201)
  - Time: 25 minutes
  - Notes: Updated `ImapSession::move_email` to handle folder selection and delegate to `AsyncImapOps::move_email`

- [ ] Fix duplicate `fetch_emails` method in `src/imap/session.rs` (E0201)
  - Time: Pending
  - Notes: Need to update to handle search criteria and delegate to `AsyncImapOps::fetch_emails`

### 2. Fix Duplicate Module Definitions
- [ ] Fix duplicate `crate::mcp` module structure
  - Time: Pending
  - Notes: Need to consolidate MCP protocol implementation

### 3. Fix Duplicate Error Code Definitions
- [x] Fix error code imports in `src/api/mcp_sse.rs`
  - Time: 15 minutes
  - Notes: Consolidated error code imports from `crate::mcp::error_codes`

### 4. Fix Duplicate JSON-RPC Type Definitions
- [ ] Fix duplicate JSON-RPC type imports
  - Time: Pending
  - Notes: Need to consolidate JSON-RPC type definitions

## Task Log
- **Completed**: Fixed duplicate `StoreOperation` definition in `src/imap/session.rs`
  - **Time**: 15 minutes
  - **Notes**: Updated `store_flags` method to use `FlagOperation` instead of local `StoreOperation` enum

- **Completed**: Fixed duplicate `append` method in `src/imap/session.rs`
  - **Time**: 20 minutes
  - **Notes**: Updated `ImapSession::append` to properly delegate to `AsyncImapOps::append` with empty flags

- **Completed**: Fixed duplicate `move_email` method in `src/imap/session.rs`
  - **Time**: 25 minutes
  - **Notes**: Updated `ImapSession::move_email` to handle folder selection and delegate to `AsyncImapOps::move_email` 