use serde::{Serialize, Deserialize};

// Stats Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub active_dashboard_sse_clients: usize,
    pub requests_per_minute: f64,
    pub average_response_time_ms: f64,
    pub system_health: SystemHealth,
    pub last_updated: String, // ISO timestamp
}

// Represents a single data point for request rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRateData {
    pub timestamp: String,
    pub value: u32,
}

// Represents system health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: SystemStatus,
    pub memory_usage: f32, // percentage
    pub cpu_usage: f32,    // percentage
}

// Enum for system status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemStatus {
    Healthy,
    Degraded,
    Critical,
}

// Client Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub r#type: ClientType,
    pub connected_at: String, // ISO timestamp
    pub status: ClientStatus,
    pub last_activity: String, // ISO timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ClientType {
    Sse,
    Api,
    Console,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum ClientStatus {
    Active,
    Idle,
    Disconnecting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedClients {
    pub clients: Vec<ClientInfo>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub total: usize,
    pub page: usize,
    pub limit: usize,
    pub total_pages: usize,
}

// Config Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub active_adapter: ImapAdapter,
    pub available_adapters: Vec<ImapAdapter>,
    pub version: String,
    pub uptime: u64, // seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapAdapter {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_active: bool,
}

// Chatbot Types
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatbotQuery {
    pub query: String,
    pub conversation_id: Option<String>,
    pub provider_override: Option<String>,
    pub model_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatbotResponse {
    pub text: String,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_data: Option<EmailData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followup_suggestions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<EmailMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<EmailFolder>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub snippet: String,
    pub is_read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailFolder {
    pub name: String,
    pub count: u32,
    pub unread_count: u32,
}

// SSE Event Types
#[derive(Debug, Clone, Serialize)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
}
