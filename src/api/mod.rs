//! REST API implementation using Actix Web.

// pub mod mcp;
pub mod rest;
pub mod sse;
pub mod mcp_stdio;
pub mod mcp_sse;
// pub mod mcp_sse; // Removed - file does not exist, covered by sse.rs

// Comment out test modules as files were deleted
// pub mod rest_test;
// pub mod mcp_test;
// pub mod sse_test;

// Placeholder for common API errors or types if necessary
pub mod error {
    // Define common API errors here
}

// pub mod sse; // Will be added later 