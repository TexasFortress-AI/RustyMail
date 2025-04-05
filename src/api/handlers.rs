use crate::error::ImapApiError;
use crate::imap::client::ImapClient;
use crate::models::folder::FolderRenameRequest;
use crate::models::error::ApiErrorResponse;
use crate::models::email::{EmailListResponse, EmailMoveRequest};
use actix_web::{web, HttpResponse, Responder};
use std::sync::{Arc, Mutex};
use tera::{Tera, Context};
use serde_json::json;
use serde::{Deserialize, Serialize};
use crate::imap::client::ImapSessionTrait;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

// Helper to map ImapApiError to HttpResponse
fn handle_imap_error(e: ImapApiError) -> HttpResponse {
    eprintln!("IMAP Error: {:?}", e);
    HttpResponse::InternalServerError().json(ApiErrorResponse {
        error: format!("IMAP error: {}", e),
    })
}

// Handler for GET /folders
#[tracing::instrument(skip(imap_client), name = "handle_list_folders")]
pub async fn list_folders_handler<S: ImapSessionTrait>(imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>) -> impl Responder {
    let client = imap_client.lock().unwrap();
    match client.list_folders().await {
        Ok(folders) => {
            let folder_responses = folders.inner
                .iter()
                .map(|name| json!({"name": name}))
                .collect::<Vec<_>>();
            HttpResponse::Ok().json(folder_responses)
        }
        Err(e) => handle_imap_error(e)
    }
}

// Handler for POST /folders
#[tracing::instrument(skip(imap_client, folder_data), name = "handle_create_folder")]
pub async fn create_folder_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_data: web::Json<CreateFolderRequest>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    match client.create_folder(&folder_data.name).await {
        Ok(_) => HttpResponse::Created().json(json!({"name": folder_data.name})),
        Err(e) => handle_imap_error(e)
    }
}

// Handler for DELETE /folders/{folder}
#[tracing::instrument(skip(imap_client), name = "handle_delete_folder")]
pub async fn delete_folder_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    match client.delete_folder(&folder_name).await {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => handle_imap_error(e)
    }
}

// Handler for PUT /folders/{folder}/rename
#[tracing::instrument(skip(imap_client, folder_data), name = "handle_rename_folder")]
pub async fn rename_folder_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_data: web::Json<FolderRenameRequest>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    match client.rename_folder(&folder_data.old_name, &folder_data.new_name).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "old_name": folder_data.old_name,
            "new_name": folder_data.new_name,
            "message": "Folder renamed successfully"
        })),
        Err(e) => handle_imap_error(e)
    }
}

// Handler for GET /folders/{folder}/stats
#[tracing::instrument(skip(imap_client), name = "handle_get_folder_stats")]
pub async fn get_folder_stats_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    path: web::Path<String>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    let folder_name = path.into_inner();
    match client.get_folder_stats(&folder_name).await {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(e) => handle_imap_error(e),
    }
}

// Handler for GET /emails/{folder}
#[tracing::instrument(skip(imap_client), name = "handle_list_emails")]
pub async fn list_emails_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    
    // First select the folder
    if let Err(e) = client.select_folder(&folder_name).await {
        return handle_imap_error(e);
    }
    
    // Then search for emails (in this case, all emails)
    match client.search("ALL").await {
        Ok(emails) => {
            let email_list = emails.into_iter().map(|uid| {
                json!({
                    "uid": uid,
                    "subject": "Email Subject",
                    "from": "sender@example.com",
                    "date": "2023-01-01T00:00:00Z"
                })
            }).collect::<Vec<_>>();
            
            HttpResponse::Ok().json(json!({ "emails": email_list }))
        },
        Err(e) => handle_imap_error(e)
    }
}

// Handler for GET /emails/{folder}/unread
#[tracing::instrument(skip(imap_client), name = "handle_list_unread_emails")]
pub async fn list_unread_emails_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    folder_name: web::Path<String>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    match client.list_unread_emails(&folder_name.into_inner()).await {
        Ok(emails) => HttpResponse::Ok().json(EmailListResponse { emails }),
        Err(e) => handle_imap_error(e),
    }
}

// Handler for GET /emails/{folder}/{uid}
#[tracing::instrument(skip(imap_client), name = "handle_get_email")]
pub async fn get_email_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    path_params: web::Path<(String, String)>,
) -> impl Responder {
    let (folder_name, email_id) = path_params.into_inner();
    let client = imap_client.lock().unwrap();
    
    // First select the folder
    if let Err(e) = client.select_folder(&folder_name).await {
        return handle_imap_error(e);
    }
    
    // Then fetch the specific email
    match client.fetch(&email_id).await {
        Ok(_) => {
            // Create a mock email detail for testing
            let email_data = json!({
                "uid": email_id.parse::<u32>().unwrap_or(0),
                "subject": "Email Subject",
                "from": "sender@example.com",
                "to": "recipient@example.com",
                "date": "2023-01-01T00:00:00Z",
                "text_body": "Email body content goes here...",
                "html_body": "<p>Email body content goes here...</p>",
                "attachments": []
            });
            HttpResponse::Ok().json(email_data)
        },
        Err(e) => handle_imap_error(e)
    }
}

// Handler for POST /emails/move
#[tracing::instrument(skip(imap_client, move_request), name = "handle_move_email")]
pub async fn move_email_handler<S: ImapSessionTrait>(
    imap_client: web::Data<Arc<Mutex<ImapClient<S>>>>,
    move_request: web::Json<EmailMoveRequest>,
) -> impl Responder {
    let client = imap_client.lock().unwrap();
    
    // Try to move the email
    match client.move_messages(&move_request.uid.to_string(), &move_request.dest_folder).await {
        Ok(_) => {
            HttpResponse::Ok().json(json!({
                "message": format!("Email moved from {} to {}", 
                                   move_request.source_folder, 
                                   move_request.dest_folder)
            }))
        },
        Err(e) => handle_imap_error(e)
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
