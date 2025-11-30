// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! REST API implementation using Actix Web.

// pub mod mcp;
pub mod auth;
pub mod errors;  // New comprehensive error module
pub mod openapi_docs;  // OpenAPI documentation
pub mod rest;
pub mod validation;
// pub mod sse;
pub mod mcp_sse;
pub mod mcp_http;  // MCP Streamable HTTP transport

// pub mod sse; // Will be added later 