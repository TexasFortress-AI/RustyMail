# IMAP API Rust Implementation Plan

## Overview
This document outlines the plan to implement the IMAP API service in Rust, maintaining full compatibility with the existing Python implementation. The Rust version will offer improved performance and memory safety while preserving the exact API interface.

## Core Requirements
- Maintain 100% API compatibility with the Python implementation
- Match or exceed current performance metrics
- Implement comprehensive error handling
- Provide detailed logging
- Include full test coverage

## Technology Stack
- **Web Framework**: [Actix-web](https://actix.rs/) (v4.3.1) for REST API functionality
- **IMAP Library**: [imap](https://crates.io/crates/imap) (v2.4.1) with [native-tls](https://crates.io/crates/native-tls) (v0.2.11) for IMAP protocol communication
- **Email Parsing**: [mail-parser](https://crates.io/crates/mail-parser) (v0.9.1) and [lettre](https://crates.io/crates/lettre) (v0.10.4) for MIME handling
- **JSON Processing**: [serde](https://serde.rs/) (v1.0.164) and [serde_json](https://docs.rs/serde_json/) (v1.0.99) for serialization/deserialization
- **Logging**: [tracing](https://crates.io/crates/tracing) (v0.1.37) with [tracing-subscriber](https://crates.io/crates/tracing-subscriber) (v0.3.17) for structured logging
- **HTTP Client for Testing**: [reqwest](https://crates.io/crates/reqwest) (v0.11.18)
- **Testing Framework**: Rust's built-in testing with [criterion](https://crates.io/crates/criterion) (v0.5.1) for benchmarking
- **HTML Templating**: [tera](https://crates.io/crates/tera) (v1.19.0) for HTML templates
- **Configuration**: [config](https://crates.io/crates/config) (v0.13.3) for configuration management
- **Error Handling**: [thiserror](https://crates.io/crates/thiserror) (v1.0.40) for error definitions
- **Async Runtime**: [tokio](https://crates.io/crates/tokio) (v1.28.2) for async execution
- **UUID Generation**: [uuid](https://crates.io/crates/uuid) (v1.3.3) for Message-ID generation
- **Date Formatting**: [chrono](https://crates.io/crates/chrono) (v0.4.26) for date/time handling
- **Base64 Encoding/Decoding**: [base64](https://crates.io/crates/base64) (v0.21.2) for attachment handling

## Implementation Checklist

### 1. Project Setup (1-2 days) - ✅ COMPLETED
- [x] Create Rust project with Cargo
- [x] Set up project structure
  - [x] src/main.rs - Entry point
  - [x] src/lib.rs - Core functionality
  - [x] src/api/ - API endpoints
  - [x] src/imap/ - IMAP client interface
  - [x] src/models/ - Data models
  - [x] src/utils/ - Helper functions
  - [x] src/config.rs - Configuration
  - [x] src/error.rs - Error definitions
  - [x] tests/ - Test suite
- [x] Configure dependencies in Cargo.toml
  ```toml
  [package]
  name = "imap-api"
  version = "0.1.0"
  edition = "2021"
  authors = ["Your Name <your.email@example.com>"]
  description = "REST API for IMAP email operations"
  
  [dependencies]
  actix-web = "4.3.1"
  imap = "2.4.1"
  native-tls = "0.2.11"
  mail-parser = "0.9.1"
  lettre = { version = "0.10.4", features = ["builder", "smtp-transport"] }
  serde = { version = "1.0.164", features = ["derive"] }
  serde_json = "1.0.99"
  tracing = "0.1.37"
  tracing-subscriber = { version = "0.3.17", features = ["env-filter"] }
  tracing-actix-web = "0.7.4"
  tera = "1.19.0"
  tokio = { version = "1.28.2", features = ["full"] }
  config = { version = "0.13.3", features = ["toml", "yaml"] }
  thiserror = "1.0.40"
  uuid = { version = "1.3.3", features = ["v4", "fast-rng"] }
  chrono = "0.4.26"
  base64 = "0.21.2"
  
  [dev-dependencies]
  criterion = "0.5.1"
  reqwest = { version = "0.11.18", features = ["json"] }
  mockito = "1.1.0"
  tokio-test = "0.4.2"
  
  [[bench]]
  name = "api_benchmarks"
  harness = false
  ```
- [x] Implement basic logging configuration
- [ ] Set up CI/CD pipeline for testing

### 2. Core IMAP Functionality (4-7 days) - ✅ COMPLETED
- [x] Implement IMAP connection handling
  - [x] Connection pooling for performance
  - [x] Reconnection logic
  - [x] Error handling and timeouts
- [x] Create secure authentication using TLS
- [x] Build folder management operations
  - [x] List folders
  - [x] Create folders
  - [x] Delete folders
  - [x] Rename folders
- [x] Implement email operations
  - [x] List emails in a folder
  - [x] Fetch email content (plain text and HTML)
  - [x] Search emails (by criteria)
  - [x] Move emails between folders
- [x] Add email creation and appending to folders

### 3. REST API Implementation (3-5 days) - ✅ COMPLETED
- [x] Set up basic web server
  - [x] Configure middleware for CORS, logging, etc.
  - [x] Set up error handling
  - [x] Implement rate limiting
- [x] Implement API routing
- [x] Create JSON serialization/deserialization for requests/responses
- [x] Implement error handling middleware
- [x] Add request validation
- [x] Implement HTML templating for root page

### 4. API Endpoints (5-7 days) - ✅ COMPLETED
All endpoints have been implemented as per the specification in the handlers.rs file.

#### Base Endpoints
- [x] `GET /` - Homepage with documentation (HTML)
- [x] `GET /api-docs` - JSON API documentation

#### Folder Management
- [x] `GET /folders` - List all folders
- [x] `POST /folders` - Create a new folder
- [x] `DELETE /folders/<folder>` - Delete an empty folder
- [x] `PUT /folders/<folder>/rename` - Rename a folder
- [x] `GET /folders/<folder>/stats` - Get folder statistics

#### Email Management
- [x] `GET /emails/<folder>` - List all emails in a folder
- [x] `GET /emails/<folder>/<uid>` - Get a specific email by UID
- [x] `GET /emails/<folder>/unread` - List unread emails in a folder
- [x] `POST /emails/move` - Move an email between folders
- [x] `POST /emails/<folder>` - Create a new email in a folder

### 5. Helper Functions (2-3 days) - ✅ COMPLETED
- [x] Implement email header decoding
- [x] Create email body extraction
- [x] Build MIME handling for email creation
- [x] Add support for attachments
- [x] Implement message ID generation with UUID and timestamp

### 6. Testing Framework (4-6 days) - ✅ COMPLETED
- [x] Create unit tests for IMAP client 
  - [x] Core IMAP client functions are tested with mocks
- [x] Implement integration tests 
  - [x] Updated to use real IMAP server instead of mocks
  - [x] Configured to handle server-specific issues
  - [x] Basic operations: folder creation, listing, renaming, and deletion
  - [x] Email listing and validation
- [x] Enhanced error handling for inconsistent IMAP server behavior
  - [x] Added retry mechanism for folder deletion
  - [x] Improved error messaging for debugging
- [x] Add benchmark tests - ✅ COMPLETED
  - [x] Implemented performance measurement for API endpoints
  - [x] Created benchmark infrastructure using criterion
  - [x] Measured performance of key endpoints
- [x] Document test approach in code comments

### 7. Documentation (2-3 days) - 🟡 IN PROGRESS
- [ ] Generate API documentation
- [ ] Create usage examples
- [ ] Document error codes and responses
- [ ] Write deployment instructions

## Current Status Summary

### Completed:
1. Project Setup
2. Core IMAP Functionality 
3. REST API Implementation
4. API Endpoints Implementation
5. Helper Functions
6. Integration Tests (using real IMAP server)
7. Performance Tests (benchmarks)

### In Progress:
1. Documentation

### Not Started:
1. CI/CD pipeline integration

## Updated Test Strategy

### Unit Tests
- Using mocks for isolated component testing
- Focus on testing the IMAP client interface and utility functions
- Isolate API handlers for focused testing without external dependencies

### Integration Tests
- Successfully implemented with real IMAP server
- Tests use the server's actual behavior for folder operations
- Folder names are prefixed with "INBOX." to comply with server requirements
- Includes proper cleanup with retry mechanism to handle server-side delays
- Tests verify:
  - Folder listing
  - Folder creation and deletion
  - Folder renaming
  - Email listing

### Performance Tests
- Successfully implemented benchmark framework using criterion
- Benchmarked core operations:
  - Folder listing operation: average 555ms per request
  - Folder stats operation: average 686ms per request
  - Email listing operation: average 676ms per request

#### Benchmark Results
```
folder_operations/list_folders
                        time:   [472.50 ms 554.99 ms 663.12 ms]

folder_operations/folder_stats
                        time:   [511.27 ms 686.22 ms 894.69 ms]

email_operations/list_emails
                        time:   [496.81 ms 675.59 ms 861.86 ms]
```

## Next Steps

1. **Documentation** (2-3 days)
   - Generate API documentation with cargo doc
   - Create usage examples
   - Document error codes and responses
   - Write deployment instructions

2. **CI/CD Setup** (1-2 days)
   - Set up GitHub Actions or other CI/CD pipeline
   - Configure automated testing
   - Set up code coverage reporting

## Updated Timeline
- Documentation: 2-3 days
- CI/CD and final polishing: 2-4 days

Total remaining work: 4-7 days

## Risk Reassessment
- Real IMAP server testing has been successful, eliminating a major risk point
- Benchmarking shows reasonable performance for a prototype implementation
- Dealing with IMAP server quirks was the main challenge, which has been addressed
- The remaining risks are mostly in the documentation and deployment areas
- The transition from mock-based to real IMAP server tests has improved confidence in the implementation

## Data Models

Define the core data structures for the application:

### 1. Configuration Model
```rust
// src/models/config.rs
#[derive(Debug, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub imap: ImapConfig,
    pub server: ServerConfig,
    pub log_level: String,
}
```

### 2. Folder Models
```rust
// src/models/folder.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    pub has_children: bool,
    pub flags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderListResponse {
    pub folders: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderCreateRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderCreateResponse {
    pub message: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderRenameRequest {
    pub new_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderRenameResponse {
    pub message: String,
    pub old_name: String,
    pub new_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderDeleteResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderStats {
    pub name: String,
    pub total_messages: u32,
    pub unread_messages: u32,
    pub size_bytes: u64,
    pub first_message_date: Option<String>,
    pub last_message_date: Option<String>,
}
```

### 3. Email Models
```rust
// src/models/email.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct EmailSummary {
    pub uid: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub message_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailBody {
    pub text_plain: Option<String>,
    pub text_html: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailDetail {
    pub uid: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub body: EmailBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailListResponse {
    pub emails: Vec<EmailSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMoveRequest {
    pub uid: String,
    pub source_folder: String,
    pub dest_folder: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMoveResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content: String, // base64 encoded
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailCreateRequest {
    pub subject: String,
    pub body: EmailBody,
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailCreateResponse {
    pub message: String,
    pub uid: String,
    pub message_id: String,
}
```

### 4. Error Models
```rust
// src/models/error.rs
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct FolderNotEmptyError {
    pub error: String,
    pub message_count: u32,
}
```

## Error Handling

Define a comprehensive error handling system:

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImapApiError {
    #[error("IMAP connection error: {0}")]
    ConnectionError(String),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Folder operation error: {0}")]
    FolderError(String),
    
    #[error("Folder not found: {0}")]
    FolderNotFound(String),
    
    #[error("Folder not empty: {0} messages")]
    FolderNotEmpty(u32),
    
    #[error("Email operation error: {0}")]
    EmailError(String),
    
    #[error("Email not found: {0}")]
    EmailNotFound(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    #[error("TLS error: {0}")]
    TlsError(String),

    #[error("MIME error: {0}")]
    MimeError(String),
}

// Implement From trait for common error types
impl From<imap::Error> for ImapApiError {
    fn from(err: imap::Error) -> Self {
        match err {
            imap::Error::No(msg) => ImapApiError::FolderError(msg),
            imap::Error::Bad(msg) => ImapApiError::EmailError(msg),
            imap::Error::Append => ImapApiError::EmailError("Failed to append message".into()),
            imap::Error::Connection(e) => ImapApiError::ConnectionError(e.to_string()),
            imap::Error::Parse(e) => ImapApiError::ParseError(e.to_string()),
            imap::Error::Validate(e) => ImapApiError::ValidationError(e.to_string()),
            _ => ImapApiError::InternalError(format!("{}", err)),
        }
    }
}

impl From<native_tls::Error> for ImapApiError {
    fn from(err: native_tls::Error) -> Self {
        ImapApiError::TlsError(err.to_string())
    }
}

impl From<mail_parser::MailParserError> for ImapApiError {
    fn from(err: mail_parser::MailParserError) -> Self {
        ImapApiError::ParseError(err.to_string())
    }
}
```

## IMAP Client Interface

```rust
// src/imap/client.rs
use crate::error::ImapApiError;
use crate::models::{Folder, EmailSummary, EmailDetail, FolderStats};
use imap::Session;
use native_tls::TlsStream;
use std::net::TcpStream;
use tracing::{debug, info, error};

pub struct ImapClient {
    host: String,
    port: u16,
    username: String,
    password: String,
}

impl ImapClient {
    pub fn new(host: String, port: u16, username: String, password: String) -> Self {
        Self { host, port, username, password }
    }
    
    pub fn connect(&self) -> Result<Session<TlsStream<TcpStream>>, ImapApiError> {
        debug!("Connecting to IMAP server {}:{}...", self.host, self.port);
        
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| ImapApiError::TlsError(e.to_string()))?;
            
        let client = imap::connect(
            (self.host.as_str(), self.port),
            &self.host,
            &tls
        )?;
        
        debug!("Authenticating as {}...", self.username);
        let session = client.login(&self.username, &self.password)?;
        info!("Successfully connected to IMAP server");
        
        Ok(session)
    }
    
    // Implement methods for each IMAP operation, e.g.:
    pub fn list_folders(&self) -> Result<Vec<String>, ImapApiError> {
        let mut session = self.connect()?;
        let list = session.list(None, Some("*"))?;
        
        let folders = list.iter()
            .filter_map(|item| {
                let name = item.name();
                if let Ok(folder_name) = std::str::from_utf8(name) {
                    Some(folder_name.to_string())
                } else {
                    None
                }
            })
            .collect();
            
        session.logout()?;
        Ok(folders)
    }
    
    // Implement other methods:
    // - create_folder
    // - delete_folder
    // - rename_folder
    // - get_folder_stats
    // - list_emails
    // - get_email
    // - list_unread_emails
    // - move_email
    // - create_email
}
```

## Risk Assessment
- Documentation completeness
- CI/CD pipeline configuration
- Potential real-world performance optimizations
- IMAP server-specific quirks and edge cases 