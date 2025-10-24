// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// src/mcp/mod.rs

// Export key types and modules
pub mod adapters;
pub mod error_codes;
pub mod handler;
pub mod types;

// Re-export common types from submodules
pub use error_codes::ErrorCode; // Only export ErrorCode, not the map
pub use handler::McpHandler; // Only export the main trait
pub use types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpPortState};

// Re-export adapters if needed - Check if LegacyMcpHandler is needed outside mcp
// pub use adapters::{ 
//     legacy::LegacyMcpHandler,
//     sdk::SdkMcpAdapter,
// }; 