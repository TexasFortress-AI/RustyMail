use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct FolderNotEmptyErrorResponse {
    pub error: String,
    pub message_count: u32,
}
