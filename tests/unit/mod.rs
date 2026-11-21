// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Unit tests for RustyMail
// This module organizes all unit tests

pub mod api;
pub mod api_key_source_tests;
pub mod api_key_tests;
pub mod config;
pub mod dashboard_model_defaults_tests;
pub mod dashboard_static_site_tests;
pub mod imap;
pub mod model_serve_provider_tests;
pub mod rest_v1_flag_tests;
pub mod session_manager;
pub mod targeted_expunge_tests;
pub mod transport;
// pub mod dashboard_client_management; // Disabled
pub mod dashboard_config;
// pub mod dashboard_events; // Disabled
// pub mod dashboard_health; // Disabled
// pub mod nlp_processor_tests; // Disabled
// pub mod dashboard_api_handlers; // Disabled
// pub mod dashboard_sse_streaming; // Disabled
pub mod account_connect_tests;
pub mod account_service_tests;
pub mod ai_local_only_tests;
pub mod ai_service_tests;
pub mod attachment_tests;
pub mod backoff_tests;
pub mod cache_prune_tests;
pub mod cache_service_tests;
pub mod child_reaper_tests;
pub mod hardcoded_detection;
pub mod imap_keepalive_tests;
pub mod mail_provider_tests;
pub mod mcp_uids_schema_tests;
pub mod message_id_tests;
pub mod new_tools_tests;
pub mod oauth_tests;
pub mod rmcp_sdk_tests;
pub mod smtp_service_tests;
pub mod special_folders_tests;
pub mod triage_codes_tests;
pub mod triage_counterparty_tests;
pub mod triage_decision_tests;
pub mod triage_guards_tests;
pub mod triage_injection_tests;
pub mod triage_lists_tests;
pub mod triage_model_tests;
pub mod triage_plan_tests;
pub mod triage_rules_tests;
pub mod triage_second_pass_tests;
pub mod triage_store_tests;
pub mod triage_subject_rules_tests;
