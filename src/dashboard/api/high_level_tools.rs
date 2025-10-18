// src/dashboard/api/high_level_tools.rs
// High-level MCP tools for AI-first email management
// Exposes only 10-12 tools to reduce context pollution

use serde_json::{json, Value};
use log::{debug, error};
use crate::dashboard::services::DashboardState;

/// Get high-level MCP tools in JSON-RPC format
/// Returns only the essential tools for AI agents (browsing, drafting, configuration)
pub fn get_mcp_high_level_tools_jsonrpc_format() -> Vec<Value> {
    vec![
        // === Agentic/Action Tools (3) ===
        json!({
            "name": "process_email_instructions",
            "description": "Execute complex email workflows using natural language instructions. The AI agent will use available email tools to complete the task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "Natural language instruction describing the email task to perform (e.g., 'Move all unread emails from John to Archive folder')"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["instruction", "account_id"]
            }
        }),
        json!({
            "name": "draft_reply",
            "description": "Generate a draft reply to an existing email using AI",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "email_uid": {
                        "type": "integer",
                        "description": "UID of the email to reply to"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder containing the email (e.g., INBOX)"
                    },
                    "instruction": {
                        "type": "string",
                        "description": "Optional instructions for the reply (e.g., 'polite decline', 'confirm meeting')"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["email_uid", "folder", "account_id"]
            }
        }),
        json!({
            "name": "draft_email",
            "description": "Generate a draft email from scratch using AI",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "Recipient email address"
                    },
                    "subject": {
                        "type": "string",
                        "description": "Email subject"
                    },
                    "context": {
                        "type": "string",
                        "description": "Context or instructions for the email content"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["to", "subject", "context", "account_id"]
            }
        }),

        // === Discovery/Browsing Tools (6 read-only) ===
        json!({
            "name": "list_accounts",
            "description": "List all configured email accounts",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "list_folders_hierarchical",
            "description": "List folders with hierarchical structure for an account",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        json!({
            "name": "list_cached_emails",
            "description": "List emails in a folder with pagination",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (e.g., INBOX)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of emails to return (default: 50)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of emails to skip (default: 0)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        json!({
            "name": "get_email_by_uid",
            "description": "Get full email content by UID",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uid": {
                        "type": "integer",
                        "description": "Email UID"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Folder containing the email (e.g., INBOX)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["uid", "account_id"]
            }
        }),
        json!({
            "name": "search_cached_emails",
            "description": "Search cached emails by subject, sender, or date",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder to search in (optional, searches all if not provided)"
                    },
                    "subject": {
                        "type": "string",
                        "description": "Search by subject (partial match)"
                    },
                    "from_address": {
                        "type": "string",
                        "description": "Search by sender email address"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default: 50)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["account_id"]
            }
        }),
        json!({
            "name": "get_folder_stats",
            "description": "Get statistics for a folder (total emails, unread count, etc.)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Folder name (e.g., INBOX)"
                    },
                    "account_id": {
                        "type": "string",
                        "description": "REQUIRED. Email address of the account (e.g., user@example.com)"
                    }
                },
                "required": ["folder", "account_id"]
            }
        }),

        // === Configuration Tools (3) ===
        json!({
            "name": "get_model_configurations",
            "description": "Get current AI model configurations for tool-calling and drafting",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "set_tool_calling_model",
            "description": "Configure the AI model used for processing email instructions and tool routing",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Provider name (e.g., 'ollama', 'openai', 'anthropic')"
                    },
                    "model_name": {
                        "type": "string",
                        "description": "Model name (e.g., 'qwen2.5:7b', 'gpt-4')"
                    },
                    "base_url": {
                        "type": "string",
                        "description": "Optional base URL for the provider API (e.g., 'http://localhost:11434' for Ollama)"
                    },
                    "api_key": {
                        "type": "string",
                        "description": "Optional API key for commercial providers"
                    }
                },
                "required": ["provider", "model_name"]
            }
        }),
        json!({
            "name": "set_drafting_model",
            "description": "Configure the AI model used for drafting emails",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Provider name (e.g., 'ollama', 'openai', 'anthropic')"
                    },
                    "model_name": {
                        "type": "string",
                        "description": "Model name (e.g., 'llama3.3:70b', 'gpt-4')"
                    },
                    "base_url": {
                        "type": "string",
                        "description": "Optional base URL for the provider API (e.g., 'http://localhost:11434' for Ollama)"
                    },
                    "api_key": {
                        "type": "string",
                        "description": "Optional API key for commercial providers"
                    }
                },
                "required": ["provider", "model_name"]
            }
        }),
    ]
}

/// Execute a high-level MCP tool
/// Routes tool calls to appropriate handlers
pub async fn execute_high_level_tool(
    state: &DashboardState,
    tool_name: &str,
    arguments: Value,
) -> Value {
    debug!("Executing high-level tool: {} with args: {:?}", tool_name, arguments);

    match tool_name {
        // Configuration tools (implemented)
        "get_model_configurations" => {
            handle_get_model_configurations(state).await
        }
        "set_tool_calling_model" => {
            handle_set_tool_calling_model(state, arguments).await
        }
        "set_drafting_model" => {
            handle_set_drafting_model(state, arguments).await
        }

        // Browsing tools (delegate to existing handlers)
        "list_accounts" |
        "list_folders_hierarchical" |
        "list_cached_emails" |
        "get_email_by_uid" |
        "search_cached_emails" |
        "get_folder_stats" => {
            // Delegate to existing low-level handler
            crate::dashboard::api::handlers::execute_mcp_tool_inner(state, tool_name, arguments).await
        }

        // Agentic/drafting tools
        "process_email_instructions" => {
            handle_process_email_instructions(state, arguments).await
        }
        "draft_reply" => {
            handle_draft_reply(state, arguments).await
        }
        "draft_email" => {
            handle_draft_email(state, arguments).await
        }

        _ => {
            error!("Unknown high-level tool: {}", tool_name);
            json!({
                "success": false,
                "error": format!("Unknown tool: {}", tool_name)
            })
        }
    }
}

// === Configuration Tool Handlers ===

async fn handle_get_model_configurations(state: &DashboardState) -> Value {
    use crate::dashboard::services::ai::model_config;

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    match model_config::get_all_model_configs(pool).await {
        Ok(configs) => {
            json!({
                "success": true,
                "data": configs
            })
        }
        Err(e) => {
            error!("Failed to get model configurations: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to get model configurations: {}", e)
            })
        }
    }
}

async fn handle_set_tool_calling_model(state: &DashboardState, arguments: Value) -> Value {
    use crate::dashboard::services::ai::model_config::{ModelConfiguration, set_model_config};

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    let provider = match arguments.get("provider").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: provider"
        }),
    };

    let model_name = match arguments.get("model_name").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: model_name"
        }),
    };

    let mut config = ModelConfiguration::new("tool_calling", provider, model_name);

    if let Some(base_url) = arguments.get("base_url").and_then(|v| v.as_str()) {
        config = config.with_base_url(base_url);
    }

    if let Some(api_key) = arguments.get("api_key").and_then(|v| v.as_str()) {
        config = config.with_api_key(api_key);
    }

    match set_model_config(pool, &config).await {
        Ok(_) => {
            json!({
                "success": true,
                "data": {
                    "message": "Tool-calling model configured successfully",
                    "config": config
                }
            })
        }
        Err(e) => {
            error!("Failed to set tool-calling model: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to set tool-calling model: {}", e)
            })
        }
    }
}

async fn handle_set_drafting_model(state: &DashboardState, arguments: Value) -> Value {
    use crate::dashboard::services::ai::model_config::{ModelConfiguration, set_model_config};

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    let provider = match arguments.get("provider").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: provider"
        }),
    };

    let model_name = match arguments.get("model_name").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: model_name"
        }),
    };

    let mut config = ModelConfiguration::new("drafting", provider, model_name);

    if let Some(base_url) = arguments.get("base_url").and_then(|v| v.as_str()) {
        config = config.with_base_url(base_url);
    }

    if let Some(api_key) = arguments.get("api_key").and_then(|v| v.as_str()) {
        config = config.with_api_key(api_key);
    }

    match set_model_config(pool, &config).await {
        Ok(_) => {
            json!({
                "success": true,
                "data": {
                    "message": "Drafting model configured successfully",
                    "config": config
                }
            })
        }
        Err(e) => {
            error!("Failed to set drafting model: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to set drafting model: {}", e)
            })
        }
    }
}

// === Agentic Tool Handlers ===

async fn handle_process_email_instructions(state: &DashboardState, arguments: Value) -> Value {
    use crate::dashboard::services::ai::agent_executor::AgentExecutor;

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    let instruction = match arguments.get("instruction").and_then(|v| v.as_str()) {
        Some(i) => i,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: instruction"
        }),
    };

    let _account_id = match arguments.get("account_id").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: account_id"
        }),
    };

    debug!("Processing email instruction: {}", instruction);

    // Get all low-level MCP tools for the sub-agent
    let low_level_tools = crate::dashboard::api::handlers::get_mcp_tools_jsonrpc_format();

    // Also include drafting tools so the sub-agent can draft emails
    let drafting_tools = vec![
        json!({
            "name": "draft_reply",
            "description": "Generate a draft reply to an email",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "email_uid": {"type": "integer"},
                    "folder": {"type": "string"},
                    "instruction": {"type": "string"},
                    "account_id": {"type": "string"}
                },
                "required": ["email_uid", "folder", "account_id"]
            }
        }),
        json!({
            "name": "draft_email",
            "description": "Generate a draft email from scratch",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {"type": "string"},
                    "subject": {"type": "string"},
                    "context": {"type": "string"},
                    "account_id": {"type": "string"}
                },
                "required": ["to", "subject", "context", "account_id"]
            }
        }),
    ];

    // Combine all available tools
    let mut all_tools = low_level_tools;
    all_tools.extend(drafting_tools);

    // Execute with agent executor
    let executor = AgentExecutor::new();
    match executor.execute_with_tools(pool, state, instruction, all_tools).await {
        Ok(result) => {
            json!({
                "success": result.success,
                "data": {
                    "response": result.final_response,
                    "actions": result.actions_taken,
                    "iterations": result.iterations,
                    "error": result.error
                }
            })
        }
        Err(e) => {
            error!("Failed to execute email instructions: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to execute: {}", e)
            })
        }
    }
}

async fn handle_draft_reply(state: &DashboardState, arguments: Value) -> Value {
    use crate::dashboard::services::ai::email_drafter::{EmailDrafter, DraftReplyRequest};

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    let email_uid = match arguments.get("email_uid").and_then(|v| v.as_u64()) {
        Some(u) => u as u32,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: email_uid"
        }),
    };

    let folder = match arguments.get("folder").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: folder"
        }),
    };

    let account_id = match arguments.get("account_id").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => return json!({
            "success": false,
            "error": "Missing required parameter: account_id"
        }),
    };

    let instruction = arguments.get("instruction").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Fetch the original email
    let email_args = json!({
        "uid": email_uid,
        "folder": folder,
        "account_id": account_id
    });

    let email_result = crate::dashboard::api::handlers::execute_mcp_tool_inner(
        state,
        "get_email_by_uid",
        email_args
    ).await;

    if !email_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        return json!({
            "success": false,
            "error": "Failed to fetch original email"
        });
    }

    let email_data = match email_result.get("data") {
        Some(d) => d,
        None => return json!({
            "success": false,
            "error": "Email data not found in response"
        }),
    };

    // Extract email fields
    let original_from = email_data.get("from_address").and_then(|v| v.as_str()).unwrap_or("unknown");
    let original_subject = email_data.get("subject").and_then(|v| v.as_str()).unwrap_or("(no subject)");
    let original_body = email_data.get("body_text").and_then(|v| v.as_str()).unwrap_or("");

    let request = DraftReplyRequest {
        original_from: original_from.to_string(),
        original_subject: original_subject.to_string(),
        original_body: original_body.to_string(),
        instruction,
    };

    let drafter = EmailDrafter::new();
    match drafter.draft_reply(pool, request.clone()).await {
        Ok(draft) => {
            // Save the draft to the Drafts folder
            let account_email = account_id.to_string();

            // Construct reply subject (add "Re: " if not already present)
            let reply_subject = if original_subject.starts_with("Re: ") {
                original_subject.to_string()
            } else {
                format!("Re: {}", original_subject)
            };

            match state.smtp_service.save_draft(
                &account_email,
                &request.original_from,
                &reply_subject,
                &draft
            ).await {
                Ok(_) => {
                    json!({
                        "success": true,
                        "data": {
                            "draft": draft,
                            "saved_to": "Drafts folder"
                        }
                    })
                }
                Err(e) => {
                    error!("Draft generated but failed to save to Drafts folder: {}", e);
                    json!({
                        "success": true,
                        "data": {
                            "draft": draft,
                            "warning": format!("Draft generated but not saved to folder: {}", e)
                        }
                    })
                }
            }
        }
        Err(e) => {
            error!("Failed to draft reply: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to draft reply: {}", e)
            })
        }
    }
}

async fn handle_draft_email(state: &DashboardState, arguments: Value) -> Value {
    use crate::dashboard::services::ai::email_drafter::{EmailDrafter, DraftEmailRequest};

    let pool = match state.cache_service.db_pool.as_ref() {
        Some(p) => p,
        None => return json!({
            "success": false,
            "error": "Database not initialized"
        }),
    };

    let to = match arguments.get("to").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return json!({
            "success": false,
            "error": "Missing required parameter: to"
        }),
    };

    let subject = match arguments.get("subject").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return json!({
            "success": false,
            "error": "Missing required parameter: subject"
        }),
    };

    let context = match arguments.get("context").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return json!({
            "success": false,
            "error": "Missing required parameter: context"
        }),
    };

    let account_id = match arguments.get("account_id").and_then(|v| v.as_str()) {
        Some(a) => a.to_string(),
        None => return json!({
            "success": false,
            "error": "Missing required parameter: account_id"
        }),
    };

    let request = DraftEmailRequest {
        to,
        subject,
        context,
    };

    let drafter = EmailDrafter::new();
    match drafter.draft_email(pool, request.clone()).await {
        Ok(draft) => {
            // Save the draft to the Drafts folder
            match state.smtp_service.save_draft(
                &account_id,
                &request.to,
                &request.subject,
                &draft
            ).await {
                Ok(_) => {
                    json!({
                        "success": true,
                        "data": {
                            "draft": draft,
                            "saved_to": "Drafts folder"
                        }
                    })
                }
                Err(e) => {
                    error!("Draft generated but failed to save to Drafts folder: {}", e);
                    json!({
                        "success": true,
                        "data": {
                            "draft": draft,
                            "warning": format!("Draft generated but not saved to folder: {}", e)
                        }
                    })
                }
            }
        }
        Err(e) => {
            error!("Failed to draft email: {}", e);
            json!({
                "success": false,
                "error": format!("Failed to draft email: {}", e)
            })
        }
    }
}
