// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Unit tests for RustyMail
// This module organizes all unit tests

pub mod api;
pub mod imap;
pub mod transport;
pub mod config;
pub mod session_manager;
// pub mod dashboard_client_management; // Disabled
pub mod dashboard_config;
// pub mod dashboard_events; // Disabled
// pub mod dashboard_health; // Disabled
// pub mod nlp_processor_tests; // Disabled
// pub mod dashboard_api_handlers; // Disabled
// pub mod dashboard_sse_streaming; // Disabled
pub mod hardcoded_detection;
pub mod ai_service_tests;
pub mod cache_service_tests;
pub mod account_service_tests;
pub mod smtp_service_tests;
pub mod attachment_tests;
pub mod imap_keepalive_tests;
pub mod oauth_tests;
pub mod rmcp_sdk_tests;