use crate::imap::error::ImapError;
use crate::imap::ImapSessionFactory;
use crate::imap::session::{ImapSession, StoreOperation};
use crate::imap::types::{Folder, ModifyFlagsPayload, AppendEmailPayload, SearchCriteria, Flags, FlagOperation};
use crate::mcp::{McpPortState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};
use crate::mcp::{self, error_codes};
use serde_json::{json, Value};
use log::{error, info, debug, warn};
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;
use chrono::{DateTime, Utc};

// --- Define McpTool trait (ensure it's present or imported correctly) ---
// Assuming McpTool trait is defined elsewhere and imported
#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> &'static str;
    fn output_schema(&self) -> &'static str;
    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError>;
}

// Macro for deserializing params within tool execute methods
macro_rules! deserialize_params {
    ($params_val:expr, $param_struct:ident) => {{
        serde_json::from_value::< $param_struct >($params_val.clone())
            .map_err(|e| {
                let err_msg = format!("Invalid parameters: {}", e);
                JsonRpcError::invalid_params(err_msg)
            })
    }};
}

// --- Define Tool Structs Explicitly ---
pub struct McpListFoldersTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpCreateFolderTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpDeleteFolderTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpRenameFolderTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpSelectFolderTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpSearchEmailsTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpFetchEmailsTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpMoveEmailTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpStoreFlagsTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpAppendEmailTool {
    session_factory: Arc<ImapSessionFactory>,
}

pub struct McpExpungeFolderTool { session_factory: Arc<ImapSessionFactory> }

// --- Implement new() for each struct ---
impl McpListFoldersTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpCreateFolderTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpDeleteFolderTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpRenameFolderTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpSelectFolderTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpSearchEmailsTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpFetchEmailsTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpMoveEmailTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpStoreFlagsTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpAppendEmailTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }
impl McpExpungeFolderTool { pub fn new(session_factory: Arc<ImapSessionFactory>) -> Self { Self { session_factory } } }

// --- Implement McpTool::execute for each struct --- 
#[async_trait]
impl McpTool for McpListFoldersTool {
    fn name(&self) -> &'static str { "imap/listFolders" } 
    fn description(&self) -> &'static str { "Lists all folders." }
    fn input_schema(&self) -> &'static str { "{}" } 
    fn output_schema(&self) -> &'static str { "{}" }
    
    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, _params: Value) -> Result<Value, JsonRpcError> {
        let folders = session.list_folders().await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!(folders))
    }
}

#[async_trait]
impl McpTool for McpCreateFolderTool {
    fn name(&self) -> &'static str { "imap/createFolder" }
    fn description(&self) -> &'static str { "Creates a new IMAP folder." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] struct CreateFolderParams { name: String }
        let p: CreateFolderParams = deserialize_params!(params, CreateFolderParams)?;
        if p.name.trim().is_empty() { return Err(JsonRpcError::invalid_params("Folder name cannot be empty")); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        session.create_folder(&full_folder_name).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Folder created", "name": full_folder_name }))
    }
}

#[async_trait]
impl McpTool for McpExpungeFolderTool {
    fn name(&self) -> &'static str { "imap/expungeFolder" }
    fn description(&self) -> &'static str { "Expunges folder." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, _params: Value) -> Result<Value, JsonRpcError> { 
        session.expunge().await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Expunge successful" }))
    }
}

// --- Add missing impl blocks --- 
#[async_trait]
impl McpTool for McpDeleteFolderTool {
    fn name(&self) -> &'static str { "imap/deleteFolder" }
    fn description(&self) -> &'static str { "Deletes an IMAP folder." }
    fn input_schema(&self) -> &'static str { "{}" } 
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] struct DeleteFolderParams { name: String }
        let p: DeleteFolderParams = deserialize_params!(params, DeleteFolderParams)?;
        if p.name.trim().is_empty() { return Err(JsonRpcError::invalid_params("Folder name cannot be empty")); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        session.delete_folder(&full_folder_name).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Folder deleted", "name": full_folder_name }))
    }
}

#[async_trait]
impl McpTool for McpRenameFolderTool {
    fn name(&self) -> &'static str { "imap/renameFolder" }
    fn description(&self) -> &'static str { "Renames an IMAP folder." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] struct RenameFolderParams { from_name: String, to_name: String }
        let p: RenameFolderParams = deserialize_params!(params, RenameFolderParams)?;
        if p.from_name.trim().is_empty() || p.to_name.trim().is_empty() { return Err(JsonRpcError::invalid_params("Folder names cannot be empty")); }
        let full_from_name = if p.from_name.eq_ignore_ascii_case("INBOX") { p.from_name.clone() } else { format!("INBOX.{}", p.from_name) };
        let full_to_name = if p.to_name.eq_ignore_ascii_case("INBOX") { p.to_name.clone() } else { format!("INBOX.{}", p.to_name) };
        session.rename_folder(&full_from_name, &full_to_name).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Folder renamed", "from_name": full_from_name, "to_name": full_to_name }))
    }
}

#[async_trait]
impl McpTool for McpSelectFolderTool {
    fn name(&self) -> &'static str { "imap/selectFolder" }
    fn description(&self) -> &'static str { "Selects a folder." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] struct SelectFolderParams { name: String }
        let p: SelectFolderParams = deserialize_params!(params, SelectFolderParams)?;
        if p.name.trim().is_empty() { return Err(JsonRpcError::invalid_params("Folder name cannot be empty")); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        let mailbox_info = session.select_folder(&full_folder_name).await.map_err(JsonRpcError::from_imap_error)?;
        state.selected_folder = Some(full_folder_name.clone());
        Ok(json!({ 
            "folder_name": full_folder_name,
            "mailbox_info": mailbox_info
        }))
    }
}

#[async_trait]
impl McpTool for McpSearchEmailsTool {
    fn name(&self) -> &'static str { "imap/searchEmails" }
    fn description(&self) -> &'static str { "Searches emails." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        let folder_name = state.selected_folder.as_ref().ok_or_else(|| JsonRpcError::internal_error("Folder must be selected first"))?;
        #[derive(::serde::Deserialize)] struct SearchEmailsParams { criteria: SearchCriteria }
        let p: SearchEmailsParams = deserialize_params!(params, SearchEmailsParams)?;
        let uids = session.search_emails(&p.criteria).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "uids": uids }))
    }
}

#[async_trait]
impl McpTool for McpFetchEmailsTool {
    fn name(&self) -> &'static str { "imap/fetchEmails" }
    fn description(&self) -> &'static str { "Fetches emails by UID." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        let folder_name = state.selected_folder.as_ref().ok_or_else(|| JsonRpcError::internal_error("Folder must be selected first"))?;
        #[derive(::serde::Deserialize)] struct FetchEmailsParams { uids: Vec<u32>, fetch_body: Option<bool>, limit: Option<u32> }
        let p: FetchEmailsParams = deserialize_params!(params, FetchEmailsParams)?;
        if p.uids.is_empty() { return Err(JsonRpcError::invalid_params("UID list cannot be empty")); }
        let fetch_body = p.fetch_body.unwrap_or(false);
        let limit = p.limit.unwrap_or(100); // Use a default limit
        let criteria = SearchCriteria::UidSet(p.uids); // Create UidSet criteria
        let emails = session.fetch_emails(&criteria, limit, fetch_body).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(serde_json::to_value(emails).map_err(|e| JsonRpcError::internal_error(format!("Serialization Error: {}", e)))?)
    }
}

#[async_trait]
impl McpTool for McpMoveEmailTool {
    fn name(&self) -> &'static str { "imap/moveEmails" }
    fn description(&self) -> &'static str { "Moves emails by UID." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] 
        struct MoveEmailParams { uids: Vec<u32>, destination_folder: String }
        let p: MoveEmailParams = deserialize_params!(params, MoveEmailParams)?;
        if p.uids.is_empty() { return Err(JsonRpcError::invalid_params("UID list cannot be empty")); }
        if p.destination_folder.trim().is_empty() { return Err(JsonRpcError::invalid_params("Destination folder cannot be empty")); }

        let full_destination_folder = if p.destination_folder.eq_ignore_ascii_case("INBOX") { p.destination_folder.clone() } else { format!("INBOX.{}", p.destination_folder) };
        let source_folder = state.selected_folder.as_ref().ok_or_else(|| JsonRpcError::internal_error("Source folder not selected in state"))?.clone();

        session.move_email(&source_folder, p.uids.clone(), &full_destination_folder).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Emails moved", "uids": p.uids, "source_folder": source_folder, "destination_folder": full_destination_folder }))
    }
}

#[async_trait]
impl McpTool for McpStoreFlagsTool {
    fn name(&self) -> &'static str { "imap/storeFlags" }
    fn description(&self) -> &'static str { "Stores flags for emails." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        let folder_name = state.selected_folder.as_ref().ok_or_else(|| JsonRpcError::internal_error("Folder must be selected first"))?;
        #[derive(::serde::Deserialize)] struct StoreFlagsParams { uids: Vec<u32>, operation: FlagOperation, flags: Flags }
        let p: StoreFlagsParams = deserialize_params!(params, StoreFlagsParams)?;
        if p.uids.is_empty() { return Err(JsonRpcError::invalid_params("UID list cannot be empty")); }
        if p.flags.is_empty() { return Err(JsonRpcError::invalid_params("Flags list cannot be empty")); }
        
        let store_op = match p.operation {
            FlagOperation::Add => StoreOperation::Add,
            FlagOperation::Remove => StoreOperation::Remove,
            FlagOperation::Set => StoreOperation::Set,
        };
        session.store_flags(p.uids.clone(), store_op, p.flags.items).await.map_err(JsonRpcError::from_imap_error)?;
        Ok(json!({ "message": "Flags stored successfully" }))
    }
}

#[async_trait]
impl McpTool for McpAppendEmailTool {
    fn name(&self) -> &'static str { "imap/appendEmail" }
    fn description(&self) -> &'static str { "Appends an email." }
    fn input_schema(&self) -> &'static str { "{}" }
    fn output_schema(&self) -> &'static str { "{}" }

    async fn execute(&self, session: Arc<dyn ImapSession>, _state: &mut McpPortState, params: Value) -> Result<Value, JsonRpcError> { 
        #[derive(::serde::Deserialize)] 
        struct AppendEmailParams { folder: String, content: String, flags: Option<Flags>, date: Option<DateTime<Utc>> }
        let p: AppendEmailParams = deserialize_params!(params, AppendEmailParams)?;
        if p.folder.trim().is_empty() { return Err(JsonRpcError::invalid_params("Folder name cannot be empty")); }
        if p.content.trim().is_empty() { return Err(JsonRpcError::invalid_params("Content cannot be empty")); }

        let bytes = general_purpose::STANDARD.decode(&p.content)
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid base64 content: {}", e)))?;
        
        let full_folder_name = if p.folder.eq_ignore_ascii_case("INBOX") { p.folder.clone() } else { format!("INBOX.{}", p.folder) };
        
        session.append(&full_folder_name, bytes).await.map_err(JsonRpcError::from_imap_error)?;

        Ok(json!({ "message": "Email appended successfully", "folder": full_folder_name }))
    }
}

/// Defines a resource whose state can be read via MCP.
#[async_trait]
pub trait McpResource: Send + Sync {
    /// The unique name identifying this resource.
    fn name(&self) -> &str;
    
    /// A brief description of the resource.
    fn description(&self) -> &str;

    /// Reads the current state of the resource.
    async fn read(&self) -> Result<Value, JsonRpcError>;
}

pub fn create_mcp_tool_registry(session_factory: Arc<ImapSessionFactory>) -> Arc<HashMap<String, Arc<dyn McpTool>>> {
    let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
    
    let tools: Vec<Arc<dyn McpTool>> = vec![
        Arc::new(McpListFoldersTool::new(session_factory.clone())),
        Arc::new(McpCreateFolderTool::new(session_factory.clone())),
        Arc::new(McpDeleteFolderTool::new(session_factory.clone())),
        Arc::new(McpRenameFolderTool::new(session_factory.clone())),
        Arc::new(McpSelectFolderTool::new(session_factory.clone())),
        Arc::new(McpSearchEmailsTool::new(session_factory.clone())),
        Arc::new(McpFetchEmailsTool::new(session_factory.clone())),
        Arc::new(McpMoveEmailTool::new(session_factory.clone())),
        Arc::new(McpStoreFlagsTool::new(session_factory.clone())),
        Arc::new(McpAppendEmailTool::new(session_factory.clone())),
        Arc::new(McpExpungeFolderTool::new(session_factory.clone())),
    ];
    
    for tool in tools {
        tool_registry.insert(tool.name().to_string(), tool);
    }
    
    Arc::new(tool_registry)
} 