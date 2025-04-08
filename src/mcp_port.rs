use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use crate::imap::error::ImapError;
use crate::imap::client::ImapClientTrait;
use std::sync::Arc;
use std::collections::HashMap;
use crate::imap::types::{SearchCriteria, FlagOperation, Flags, AppendEmailPayload, MailboxInfo};
use serde::Deserialize;
use serde_json::json;

// Macro for deserializing params within tool execute methods
macro_rules! deserialize_params {
    ($params_val:expr, $param_struct:ident) => {{
        serde_json::from_value::< $param_struct >($params_val.clone())
            .map_err(|e| {
                let err_msg = format!("Invalid parameters: {}", e);
                McpPortError::InvalidParams(err_msg)
            })
    }};
}

// --- Tool Definitions using the Generic Macro Arm (Simplified Invocation) ---

mcp_tool!(McpListFoldersTool<C>, "imap/listFolders", "Lists all folders.", "{}", "{}",
    async fn execute(self, _params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> {
        let folders = self.imap_client.list_folders().await?;
        Ok(::serde_json::json!({ "folders": folders }))
    }
);

mcp_tool!(McpCreateFolderTool<C>, "imap/createFolder", "Creates a new IMAP folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct CreateFolderParams { name: String }
        let p: CreateFolderParams = deserialize_params!(params, CreateFolderParams)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        self.imap_client.create_folder(&full_folder_name).await?;
        Ok(::serde_json::json!({ "message": "Folder created", "name": full_folder_name }))
    }
);

mcp_tool!(McpDeleteFolderTool<C>, "imap/deleteFolder", "Deletes an IMAP folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct DeleteFolderParams { name: String }
        let p: DeleteFolderParams = deserialize_params!(params, DeleteFolderParams)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        self.imap_client.delete_folder(&full_folder_name).await?;
        Ok(::serde_json::json!({ "message": "Folder deleted", "name": full_folder_name }))
    }
);

mcp_tool!(McpRenameFolderTool<C>, "imap/renameFolder", "Renames an IMAP folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct RenameFolderParams { from_name: String, to_name: String }
        let p: RenameFolderParams = deserialize_params!(params, RenameFolderParams)?;
        if p.from_name.trim().is_empty() || p.to_name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder names cannot be empty".into())); }
        let full_from_name = if p.from_name.eq_ignore_ascii_case("INBOX") { p.from_name.clone() } else { format!("INBOX.{}", p.from_name) };
        let full_to_name = if p.to_name.eq_ignore_ascii_case("INBOX") { p.to_name.clone() } else { format!("INBOX.{}", p.to_name) };
        self.imap_client.rename_folder(&full_from_name, &full_to_name).await?;
        Ok(::serde_json::json!({ "message": "Folder renamed", "from_name": full_from_name, "to_name": full_to_name }))
    }
);

mcp_tool!(McpSelectFolderTool<C>, "imap/selectFolder", "Selects a folder, making it active for subsequent commands.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct SelectFolderParams { name: String }
        let p: SelectFolderParams = deserialize_params!(params, SelectFolderParams)?;
        if p.name.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        let full_folder_name = if p.name.eq_ignore_ascii_case("INBOX") { p.name.clone() } else { format!("INBOX.{}", p.name) };
        let mailbox_info = self.imap_client.select_folder(&full_folder_name).await?;
        Ok(::serde_json::json!({ 
            "folder_name": full_folder_name,
            "mailbox_info": mailbox_info // Assumes MailboxInfo is Serializable
        }))
    }
);

mcp_tool!(McpSearchEmailsTool<C>, "imap/searchEmails", "Searches emails in the currently selected folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct SearchEmailsParams { criteria: crate::imap::types::SearchCriteria }
        let p: SearchEmailsParams = deserialize_params!(params, SearchEmailsParams)?;
        let uids = self.imap_client.search_emails(p.criteria).await?;
        Ok(::serde_json::json!({ "uids": uids }))
    }
);

mcp_tool!(McpFetchEmailsTool<C>, "imap/fetchEmails", "Fetches emails by UID from the selected folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct FetchEmailsParams { uids: Vec<u32>, fetch_body: Option<bool> }
        let p: FetchEmailsParams = deserialize_params!(params, FetchEmailsParams)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        let fetch_body = p.fetch_body.unwrap_or(false);
        let emails = self.imap_client.fetch_emails(p.uids, fetch_body).await?;
        Ok(::serde_json::to_value(emails).map_err(|e| McpPortError::ToolError(format!("Serialization Error: {}", e)))?)
    }
);

mcp_tool!(McpMoveEmailTool<C>, "imap/moveEmails", "Moves emails by UID from the selected folder to a destination folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] 
        struct MoveEmailParams {
            uids: Vec<u32>, 
            destination_folder: String,
            source_folder: String,
        }
        let p: MoveEmailParams = deserialize_params!(params, MoveEmailParams)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        if p.destination_folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Destination folder cannot be empty".into())); }
        if p.source_folder.trim().is_empty() { return Err(McpPortError::ImapRequiresFolderSelection("Source folder missing in params".into())); }

        let full_destination_folder = if p.destination_folder.eq_ignore_ascii_case("INBOX") { p.destination_folder.clone() } else { format!("INBOX.{}", p.destination_folder) };

        self.imap_client.move_email(&p.source_folder, p.uids.clone(), &full_destination_folder).await?;
        Ok(::serde_json::json!({ "message": "Emails moved", "uids": p.uids, "destination_folder": full_destination_folder, "source_folder": p.source_folder }))
    }
);

mcp_tool!(McpStoreFlagsTool<C>, "imap/storeFlags", "Adds, removes, or sets flags for specified emails in the selected folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct StoreFlagsParams { uids: Vec<u32>, operation: crate::imap::types::FlagOperation, flags: crate::imap::types::Flags }
        let p: StoreFlagsParams = deserialize_params!(params, StoreFlagsParams)?;
        if p.uids.is_empty() { return Err(McpPortError::InvalidParams("UID list cannot be empty".into())); }
        if p.flags.items.is_empty() { return Err(McpPortError::InvalidParams("Flags list cannot be empty".into())); }
        
        self.imap_client.store_flags(p.uids, p.operation, p.flags).await?;
        Ok(::serde_json::json!({ "message": "Flags stored successfully" }))
    }
);

mcp_tool!(McpAppendEmailTool<C>, "imap/appendEmail", "Appends an email message to the specified folder.", "{}", "{}",
    async fn execute(self, params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        #[derive(::serde::Deserialize)] struct AppendEmailParams { folder: String, email: crate::imap::types::AppendEmailPayload }
        let p: AppendEmailParams = deserialize_params!(params, AppendEmailParams)?;
        if p.folder.trim().is_empty() { return Err(McpPortError::InvalidParams("Folder name cannot be empty".into())); }
        
        let full_folder_name = if p.folder.eq_ignore_ascii_case("INBOX") { p.folder.clone() } else { format!("INBOX.{}", p.folder) };

        self.imap_client.append(&full_folder_name, p.email).await?;
        Ok(::serde_json::json!({ "message": "Email appended", "folder": full_folder_name }))
    }
);

mcp_tool!(McpExpungeFolderTool<C>, "imap/expungeFolder", "Permanently removes emails marked \\Deleted from the selected folder.", "{}", "{}",
    async fn execute(self, _params: ::serde_json::Value) -> Result<::serde_json::Value, McpPortError> { 
        self.imap_client.expunge().await?;
        Ok(::serde_json::json!({ "message": "Expunge successful" }))
    }
);


// --- Restore McpPortError Enum Definition --- 
#[derive(Debug, Error)]
pub enum McpPortError {
    #[error("Tool execution failed: {0}")]
    ToolError(String),
    #[error("Resource access failed: {0}")]
    ResourceError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    #[error("Internal error: {message}")]
    InternalError { message: String },

    // --- IMAP Specific Errors --- 
    #[error("IMAP Connection Error: {0}")]
    ImapConnectionError(String),
    #[error("IMAP Authentication Error: {0}")]
    ImapAuthenticationError(String),
    #[error("IMAP Folder Not Found: {0}")]
    ImapFolderNotFound(String),
    #[error("IMAP Folder Already Exists: {0}")]
    ImapFolderExists(String),
    #[error("IMAP Email Not Found: {0}")]
    ImapEmailNotFound(String),
    #[error("IMAP Operation Failed: {0}")]
    ImapOperationFailed(String),
    #[error("IMAP Invalid Criteria: {0}")]
    ImapInvalidCriteria(String),
    #[error("IMAP Requires Folder Selection: {0}")]
    ImapRequiresFolderSelection(String),
}

/// Defines a capability or action that can be executed via MCP.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// The unique name identifying this tool.
    fn name(&self) -> &'static str;
    
    /// A brief description of what the tool does.
    fn description(&self) -> &'static str;
    
    /// Input schema description (e.g., JSON schema string)
    fn input_schema(&self) -> &'static str;
    
    /// Output schema description (e.g., JSON schema string)
    fn output_schema(&self) -> &'static str;
    
    /// Executes the tool with the given parameters.
    async fn execute(&self, params: Value) -> Result<Value, McpPortError>;
}

/// Defines a resource whose state can be read via MCP.
#[async_trait]
pub trait McpResource: Send + Sync {
    /// The unique name identifying this resource.
    fn name(&self) -> &str;
    
    /// A brief description of the resource.
    fn description(&self) -> &str;

    /// Reads the current state of the resource.
    async fn read(&self) -> Result<Value, McpPortError>;
}

pub fn create_mcp_tool_registry<C: ImapClientTrait + Send + Sync + 'static>(imap_client: Arc<C>) -> Arc<HashMap<String, Arc<dyn McpTool>>> {
    let mut tool_registry: HashMap<String, Arc<dyn McpTool>> = HashMap::new();
    
    // Instantiate the generic tool structs. The ::new method generated by the generic
    // mcp_tool! arm expects Arc<C>, which matches imap_client.clone().
    let tools: Vec<Arc<dyn McpTool>> = vec![
        Arc::new(McpListFoldersTool::<C>::new(imap_client.clone())),
        Arc::new(McpCreateFolderTool::<C>::new(imap_client.clone())),
        Arc::new(McpDeleteFolderTool::<C>::new(imap_client.clone())),
        Arc::new(McpRenameFolderTool::<C>::new(imap_client.clone())),
        Arc::new(McpSelectFolderTool::<C>::new(imap_client.clone())),
        Arc::new(McpSearchEmailsTool::<C>::new(imap_client.clone())),
        Arc::new(McpFetchEmailsTool::<C>::new(imap_client.clone())),
        Arc::new(McpMoveEmailTool::<C>::new(imap_client.clone())),
        Arc::new(McpStoreFlagsTool::<C>::new(imap_client.clone())),
        Arc::new(McpAppendEmailTool::<C>::new(imap_client.clone())),
        Arc::new(McpExpungeFolderTool::<C>::new(imap_client.clone())),
    ];
    for tool in tools {
        tool_registry.insert(tool.name().to_string(), tool);
    }
    Arc::new(tool_registry)
}

// Implement From<ImapError> for McpPortError
impl From<ImapError> for McpPortError {
    fn from(err: ImapError) -> Self {
        log::warn!("Converting IMAP Error to MCP Port Error: {:?}", err);
        match err {
            ImapError::Connection(m) => McpPortError::ImapConnectionError(m),
            ImapError::Tls(m) => McpPortError::ImapConnectionError(m), // Map TLS also to ConnectionError for MCP
            ImapError::Auth(m) => McpPortError::ImapAuthenticationError(m),
            ImapError::Parse(m) => McpPortError::ToolError(format!("IMAP Parse Error: {}", m)), // Maybe map to InvalidParams? Or ToolError?
            ImapError::BadResponse(m) => McpPortError::ToolError(format!("IMAP Bad Response: {}", m)),
            ImapError::Mailbox(m) => McpPortError::ImapFolderNotFound(m), // Assuming mailbox errors usually mean not found
            ImapError::Fetch(m) => McpPortError::ImapEmailNotFound(m), // Assuming fetch errors often relate to not found UIDs
            ImapError::Append(m) => McpPortError::ImapOperationFailed(m),
            ImapError::Operation(m) => McpPortError::ImapOperationFailed(m),
            ImapError::Command(m) => McpPortError::InvalidParams(m), // Command errors often due to bad params
            ImapError::Config(m) => McpPortError::InternalError { message: format!("IMAP Config Error: {}", m) },
            ImapError::Io(m) => McpPortError::ImapConnectionError(format!("IMAP IO Error: {}", m)),
            ImapError::Internal(m) => McpPortError::InternalError { message: format!("IMAP Internal Error: {}", m) },
            ImapError::EnvelopeNotFound => McpPortError::ImapEmailNotFound("Envelope data missing in fetch response".to_string()),
            
            // Add mappings for the new variants
            ImapError::FolderNotFound(m) => McpPortError::ImapFolderNotFound(m),
            ImapError::FolderExists(m) => McpPortError::ImapFolderExists(m),
            ImapError::RequiresFolderSelection(m) => McpPortError::ImapRequiresFolderSelection(m),
            ImapError::ConnectionError(m) => McpPortError::ImapConnectionError(m),
            ImapError::AuthenticationError(m) => McpPortError::ImapAuthenticationError(m),
            ImapError::EmailNotFound(uids) => McpPortError::ImapEmailNotFound(format!("UID(s) {:?} not found", uids)),
            ImapError::OperationFailed(m) => McpPortError::ImapOperationFailed(m),
            ImapError::InvalidCriteria(c) => McpPortError::ImapInvalidCriteria(format!("Invalid search criteria: {:?}", c)),
            ImapError::FolderNotSelected => McpPortError::ImapRequiresFolderSelection("Operation requires folder selection".to_string()),
            ImapError::ParseError(m) => McpPortError::ToolError(format!("IMAP Parse Error: {}", m)),
            // SessionError needs careful handling. Extract underlying info if possible.
            ImapError::SessionError(e) => {
                // Try to provide a more specific error based on the underlying async_imap::error::Error
                match e {
                    async_imap::error::Error::ConnectionLost => McpPortError::ImapConnectionError("Connection Lost".to_string()),
                    async_imap::error::Error::Parse(p_err) => McpPortError::ToolError(format!("IMAP Parse Error: {}", p_err)),
                    async_imap::error::Error::No(s) | async_imap::error::Error::Bad(s) => McpPortError::ImapOperationFailed(s),
                    async_imap::error::Error::Io(io_err) => McpPortError::ImapConnectionError(format!("IO Error: {}", io_err)),
                    // Handle other async_imap errors as specifically as possible, or fallback
                    _ => McpPortError::ImapOperationFailed(format!("Underlying IMAP Session Error: {}", e))
                }
            }
        }
    }
} 