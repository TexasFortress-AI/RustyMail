use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailSummary {
    pub uid: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub message_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailBody {
    pub text_plain: Option<String>,
    pub text_html: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMoveRequest {
    pub source_folder: String,
    pub dest_folder: String,
    pub uid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailMoveResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailListResponse {
    pub emails: Vec<EmailListItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailListItem {
    pub uid: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailDetail {
    pub subject: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub date: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content: String, // base64 encoded
    pub content_type: Option<String>,
}
