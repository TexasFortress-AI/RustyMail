use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailListResponse {
    pub emails: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMoveRequest {
    pub source_folder: String,
    pub target_folder: String,
    pub email_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderRenameRequest {
    pub old_name: String,
    pub new_name: String,
} 