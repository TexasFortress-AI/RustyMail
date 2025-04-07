use core::error::ImapApiError;
use core::imap::client::{ImapClient, ImapSessionTrait};
use core::models::folder::{FolderRenameRequest, FolderCreateRequest};
use core::models::error::ApiErrorResponse;
use core::models::email::{EmailListResponse, EmailMoveRequest};
use actix_web::{web, HttpResponse, Responder};
use std::sync::Arc;
use parking_lot::Mutex;
use tera::{Tera, Context};
use serde_json::json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

fn handle_imap_error(err: ImapApiError) -> HttpResponse {
    HttpResponse::InternalServerError().body(err.to_string())
}

pub async fn list_folders<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>
) -> Result<impl Responder, ImapApiError> {
    let client = client.lock();
    match client.list_folders().await {
        Ok(folders) => Ok(HttpResponse::Ok().json(folders.inner)),
        Err(e) => Err(e.into()),
    }
}

pub async fn create_folder<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder: web::Json<FolderCreateRequest>,
) -> Result<impl Responder, ImapApiError> {
    let client = client.lock();
    match client.create_folder(&folder.name).await {
        Ok(_) => Ok(HttpResponse::Created().finish()),
        Err(e) => Err(e.into()),
    }
}

pub async fn delete_folder<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> Result<impl Responder, ImapApiError> {
    let client = client.lock();
    match client.delete_folder(&folder_name).await {
        Ok(_) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Err(e.into()),
    }
}

pub async fn list_emails<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> Result<impl Responder, ImapApiError> {
    let client = client.lock();
    match client.select_folder(&folder_name).await {
        Ok(_) => match client.search_emails("ALL").await {
            Ok(emails) => Ok(HttpResponse::Ok().json(emails)),
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

pub async fn list_unread_emails<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> Result<impl Responder, ImapApiError> {
    let client = client.lock();
    match client.select_folder(&folder_name).await {
        Ok(_) => match client.search_emails("UNSEEN").await {
            Ok(emails) => Ok(HttpResponse::Ok().json(emails)),
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

pub async fn get_email<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    path: web::Path<(String, String)>,
) -> Result<impl Responder, ImapApiError> {
    let (folder_name, email_id) = path.into_inner();
    let client = client.lock();
    match client.select_folder(&folder_name).await {
        Ok(_) => match client.fetch_email_by_uid(&email_id).await {
            Ok(email) => Ok(HttpResponse::Ok().json(email.inner)),
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

pub async fn move_email<S: ImapSessionTrait + 'static>(
    client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    path: web::Path<(String, String, String)>,
) -> Result<impl Responder, ImapApiError> {
    let (source_folder, email_id, target_folder) = path.into_inner();
    let client = client.lock();
    match client.select_folder(&source_folder).await {
        Ok(_) => match client.move_email(&email_id, &target_folder).await {
            Ok(_) => Ok(HttpResponse::NoContent().finish()),
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

// --- Root Handler --- 

#[tracing::instrument(skip(tera), name = "handle_root")]
pub async fn root_handler(tera: web::Data<Tera>) -> impl Responder {
    let mut context = Context::new();
    // Pass any needed variables to the template, e.g., server port
    // For simplicity, we hardcode it here, but it should come from config
    context.insert("port", "5000"); // Example port

    match tera.render("index.html", &context) {
        Ok(rendered) => HttpResponse::Ok().content_type("text/html").body(rendered),
        Err(e) => {
            tracing::error!("Template rendering error: {}", e);
            HttpResponse::InternalServerError().body("Failed to render documentation page")
        }
    }
}

// --- API Docs Handler --- 

#[tracing::instrument(name = "handle_api_docs")]
pub async fn api_docs_handler() -> impl Responder {
    // Construct the JSON response based on the plan
    let docs = json!({
        "message": "Welcome to the IMAP API (Rust Implementation)",
        "endpoints": [
            // Folder Endpoints
            {
                "endpoint": "/folders",
                "method": "GET",
                "description": "Lists all folders in the IMAP mailbox.",
                "example": {
                    "curl": "curl http://localhost:5000/folders"
                },
                "response": {
                    "success_200": {"folders": ["INBOX", "Sent", "Trash"]},
                    "error_500": {"error": "Failed to list folders"}
                }
            },
            {
                "endpoint": "/folders",
                "method": "POST",
                "description": "Creates a new folder.",
                "example": {
                    "curl": "curl -X POST -H \"Content-Type: application/json\" -d '{\"name\": \"MyNewFolder\"}' http://localhost:5000/folders"
                },
                "request_body": {
                    "name": "string (required)"
                },
                "response": {
                    "success_201": {"message": "Folder created successfully", "name": "MyNewFolder"},
                    "error_400": {"error": "Failed to create folder: reason"}
                }
            },
            {
                "endpoint": "/folders/{folder}",
                "method": "DELETE",
                "description": "Deletes an empty folder.",
                "example": {
                    "curl": "curl -X DELETE http://localhost:5000/folders/MyEmptyFolder"
                },
                "response": {
                    "success_200": {"message": "Folder deleted successfully"},
                    "error_400_not_empty": {"error": "Folder not empty", "message_count": "N"},
                    "error_400_failed": {"error": "Failed to delete folder: reason"},
                    "error_404": {"error": "Folder '{folder}' not found"}
                }
            },
            {
                "endpoint": "/folders/{folder}/rename",
                "method": "PUT",
                "description": "Renames a folder.",
                "example": {
                    "curl": "curl -X PUT -H \"Content-Type: application/json\" -d '{\"new_name\": \"RenamedFolder\"}' http://localhost:5000/folders/OldFolder/rename"
                },
                "request_body": {
                    "new_name": "string (required)"
                },
                "response": {
                    "success_200": {"message": "Folder renamed successfully", "old_name": "OldFolder", "new_name": "RenamedFolder"},
                    "error_400": {"error": "Failed to rename folder: reason"},
                    "error_404": {"error": "Folder 'OldFolder' not found"}
                }
            },
            {
                "endpoint": "/folders/{folder}/stats",
                "method": "GET",
                "description": "Retrieves statistics for a specific folder.",
                "example": {
                    "curl": "curl http://localhost:5000/folders/INBOX/stats"
                },
                "response": {
                    "success_200": {
                        "name": "folder",
                        "total_messages": "N",
                        "unread_messages": "M",
                        "size_bytes": "S | (Not implemented)", // Indicate if size is not implemented
                        "first_message_date": "ISO-date|null",
                        "last_message_date": "ISO-date|null"
                     },
                    "error_404": {"error": "Folder '{folder}' not found"},
                    "error_400_500": {"error": "Failed to get folder stats: reason"}
                }
            },
            // Email Endpoints
            {
                "endpoint": "/emails/{folder}",
                "method": "GET",
                "description": "Lists all emails in the specified folder (metadata only).",
                "example": {
                    "curl": "curl http://localhost:5000/emails/INBOX"
                },
                "response": {
                    "success_200": {"emails": [{"uid": "123", "subject": "Subject", "from": "sender@example.com", "date": "Date", "flags": ["\\Seen"]}]},
                    "error_404": {"error": "Folder '{folder}' not found"}
                }
            },
             {
                "endpoint": "/emails/{folder}/unread",
                "method": "GET",
                "description": "Lists unread emails in the specified folder (metadata only).",
                "example": {
                    "curl": "curl http://localhost:5000/emails/INBOX/unread"
                },
                "response": {
                    "success_200": {"emails": [{"uid": "123", "subject": "Subject", "from": "sender@example.com", "date": "Date", "flags": []}]},
                    "error_404": {"error": "Folder '{folder}' not found"}
                }
            },
            {
                "endpoint": "/emails/{folder}/{uid}",
                "method": "GET",
                "description": "Fetches the full details, including body, of a specific email by UID.",
                "example": {
                    "curl": "curl http://localhost:5000/emails/INBOX/123"
                },
                "response": {
                    "success_200": {"uid": "123", "subject": "Subject", "from": "sender@example.com", "date": "Date", "text_body": "Text|null", "html_body": "HTML|null"},
                    "error_404": {"error": "Email '{folder}/{uid}' not found"}
                }
            },
            {
                "endpoint": "/emails/move",
                "method": "POST",
                "description": "Moves an email from one folder to another.",
                "example": {
                    "curl": "curl -X POST -H \"Content-Type: application/json\" -d '{\"uid\": \"123\", \"source_folder\": \"INBOX\", \"dest_folder\": \"Archive\"}' http://localhost:5000/emails/move"
                },
                 "request_body": {
                    "uid": "string (required)",
                    "source_folder": "string (required)",
                    "dest_folder": "string (required)"
                },
                "response": {
                    "success_200": {"message": "Moved email {uid} from {source_folder} to {dest_folder}"},
                    "error_400": {"error": "Missing uid, source_folder, or dest_folder"},
                    "error_404_folder": {"error": "Folder '{folder}' not found"},
                    "error_404_email": {"error": "Email '{folder}/{uid} (source)' not found"}
                }
            }
        ]
    });
    HttpResponse::Ok().json(docs)
}

// Define a struct for folder requests
#[derive(serde::Deserialize)]
pub struct FolderRequest {
    pub name: String,
}
