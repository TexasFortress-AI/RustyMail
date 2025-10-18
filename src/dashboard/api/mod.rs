// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// This file is no longer needed as its logic was moved to main.rs
// and dashboard/api/routes.rs

// Dashboard API module

pub mod accounts;
pub mod routes;
pub mod sse;
pub mod models;
pub mod handlers;
pub mod errors;
pub mod middleware;
pub mod config;
pub mod health;
pub mod attachments;
pub mod high_level_tools;

// Re-export main types needed elsewhere
pub use routes::configure as init_routes;
pub use sse::SseManager;

// The init function previously here is no longer needed, 
// its logic moved to main.rs
