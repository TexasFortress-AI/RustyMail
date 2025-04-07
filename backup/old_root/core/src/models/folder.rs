use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub name: String,
    pub has_children: bool,
    pub flags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderListResponse {
    pub folders: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderCreateRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderCreateResponse {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderRenameRequest {
    pub old_name: String,
    pub new_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderRenameResponse {
    pub old_name: String,
    pub new_name: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderDeleteResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderStats {
    pub name: String,
    pub total_messages: u32,
    pub unread_messages: u32,
    pub size_bytes: u64,
    pub first_message_date: Option<String>,
    pub last_message_date: Option<String>,
}
