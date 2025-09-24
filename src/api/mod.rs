//! REST API implementation using Actix Web.

// pub mod mcp;
pub mod auth;
pub mod errors;  // New comprehensive error module
pub mod openapi_docs;  // OpenAPI documentation
pub mod rest;
pub mod validation;
pub mod sse;
pub mod mcp_stdio;
pub mod mcp_sse;

// pub mod sse; // Will be added later 