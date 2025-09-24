//! OpenAPI documentation using a declarative approach
//!
//! This module provides OpenAPI/Swagger documentation for the REST API
//! without requiring extensive modifications to existing code.

use actix_web::{web, HttpResponse};
use serde_json::json;

/// Generate the OpenAPI specification in JSON format
pub fn generate_openapi_spec() -> serde_json::Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "RustyMail API",
            "description": "Modern IMAP client REST API with comprehensive email management capabilities",
            "version": "1.0.0",
            "contact": {
                "name": "RustyMail Team",
                "email": "support@rustymail.example.com"
            },
            "license": {
                "name": "MIT",
                "url": "https://opensource.org/licenses/MIT"
            }
        },
        "servers": [
            {
                "url": "http://localhost:8080",
                "description": "Local development server"
            },
            {
                "url": "https://api.rustymail.example.com",
                "description": "Production server"
            }
        ],
        "security": [
            {
                "ApiKeyAuth": []
            }
        ],
        "tags": [
            {
                "name": "folders",
                "description": "Folder management operations"
            },
            {
                "name": "emails",
                "description": "Email operations"
            },
            {
                "name": "search",
                "description": "Email search functionality"
            },
            {
                "name": "api-keys",
                "description": "API key management"
            }
        ],
        "paths": {
            "/api/v1/folders": {
                "get": {
                    "tags": ["folders"],
                    "summary": "List all folders",
                    "description": "Retrieve a list of all IMAP folders",
                    "operationId": "listFolders",
                    "security": [{"ApiKeyAuth": []}],
                    "responses": {
                        "200": {
                            "description": "List of folders",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/FolderListResponse"
                                    }
                                }
                            }
                        },
                        "401": {
                            "description": "Unauthorized",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": ["folders"],
                    "summary": "Create a new folder",
                    "operationId": "createFolder",
                    "security": [{"ApiKeyAuth": []}],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateFolderRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Folder created successfully"
                        },
                        "400": {
                            "description": "Bad request",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/folders/{folder_name}": {
                "get": {
                    "tags": ["folders"],
                    "summary": "Get folder details",
                    "operationId": "getFolder",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            },
                            "description": "Name of the folder"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Folder details",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/FolderResponse"
                                    }
                                }
                            }
                        },
                        "404": {
                            "description": "Folder not found",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                },
                "put": {
                    "tags": ["folders"],
                    "summary": "Rename a folder",
                    "operationId": "renameFolder",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/UpdateFolderRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Folder renamed successfully"
                        }
                    }
                },
                "delete": {
                    "tags": ["folders"],
                    "summary": "Delete a folder",
                    "operationId": "deleteFolder",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "responses": {
                        "204": {
                            "description": "Folder deleted successfully"
                        }
                    }
                }
            },
            "/api/v1/folders/{folder_name}/emails": {
                "get": {
                    "tags": ["emails"],
                    "summary": "List emails in folder",
                    "operationId": "listEmails",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "schema": {
                                "type": "integer",
                                "default": 50,
                                "minimum": 1,
                                "maximum": 100
                            }
                        },
                        {
                            "name": "offset",
                            "in": "query",
                            "schema": {
                                "type": "integer",
                                "default": 0,
                                "minimum": 0
                            }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of emails",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/EmailListResponse"
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": ["emails"],
                    "summary": "Create/upload new email",
                    "operationId": "createEmail",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateEmailRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Email created successfully"
                        }
                    }
                }
            },
            "/api/v1/folders/{folder_name}/emails/{uid}": {
                "get": {
                    "tags": ["emails"],
                    "summary": "Get email by UID",
                    "operationId": "getEmail",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        },
                        {
                            "name": "uid",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "integer",
                                "format": "int32"
                            }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Email details",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/EmailResponse"
                                    }
                                }
                            }
                        }
                    }
                },
                "patch": {
                    "tags": ["emails"],
                    "summary": "Update email flags",
                    "operationId": "updateEmailFlags",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        },
                        {
                            "name": "uid",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "integer"
                            }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/UpdateEmailRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Flags updated successfully"
                        }
                    }
                },
                "delete": {
                    "tags": ["emails"],
                    "summary": "Delete email",
                    "operationId": "deleteEmail",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        },
                        {
                            "name": "uid",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "integer"
                            }
                        }
                    ],
                    "responses": {
                        "204": {
                            "description": "Email deleted successfully"
                        }
                    }
                }
            },
            "/api/v1/folders/{folder_name}/emails/{uid}/move": {
                "post": {
                    "tags": ["emails"],
                    "summary": "Move email to another folder",
                    "operationId": "moveEmail",
                    "security": [{"ApiKeyAuth": []}],
                    "parameters": [
                        {
                            "name": "folder_name",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        },
                        {
                            "name": "uid",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "integer"
                            }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/MoveEmailRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Email moved successfully"
                        }
                    }
                }
            },
            "/api/v1/search": {
                "post": {
                    "tags": ["search"],
                    "summary": "Search emails",
                    "description": "Search emails using IMAP search criteria",
                    "operationId": "searchEmails",
                    "security": [{"ApiKeyAuth": []}],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/SearchRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Search results",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/EmailListResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/api-keys": {
                "get": {
                    "tags": ["api-keys"],
                    "summary": "List API keys",
                    "operationId": "listApiKeys",
                    "security": [{"ApiKeyAuth": ["admin"]}],
                    "responses": {
                        "200": {
                            "description": "List of API keys",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ApiKeyListResponse"
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": ["api-keys"],
                    "summary": "Create new API key",
                    "operationId": "createApiKey",
                    "security": [{"ApiKeyAuth": ["admin"]}],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateApiKeyRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "API key created",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "key": {
                                                "type": "string",
                                                "description": "The generated API key"
                                            },
                                            "message": {
                                                "type": "string"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/api-keys/info": {
                "get": {
                    "tags": ["api-keys"],
                    "summary": "Get current API key info",
                    "operationId": "getApiKeyInfo",
                    "security": [{"ApiKeyAuth": []}],
                    "responses": {
                        "200": {
                            "description": "API key information",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ApiKeyInfo"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/api-keys/{key}": {
                "delete": {
                    "tags": ["api-keys"],
                    "summary": "Revoke API key",
                    "operationId": "revokeApiKey",
                    "security": [{"ApiKeyAuth": ["admin"]}],
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "responses": {
                        "204": {
                            "description": "API key revoked"
                        }
                    }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "ApiKeyAuth": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "X-API-Key"
                }
            },
            "schemas": {
                "ErrorResponse": {
                    "type": "object",
                    "required": ["code", "message", "timestamp"],
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Error code for programmatic handling"
                        },
                        "message": {
                            "type": "string",
                            "description": "Human-readable error message"
                        },
                        "details": {
                            "$ref": "#/components/schemas/ErrorDetails"
                        },
                        "request_id": {
                            "type": "string",
                            "description": "Request ID for tracing"
                        },
                        "timestamp": {
                            "type": "string",
                            "format": "date-time"
                        }
                    }
                },
                "ErrorDetails": {
                    "type": "object",
                    "properties": {
                        "validation_errors": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/ValidationError"
                            }
                        },
                        "suggestions": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "help_links": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "ValidationError": {
                    "type": "object",
                    "properties": {
                        "field": {
                            "type": "string"
                        },
                        "message": {
                            "type": "string"
                        },
                        "constraint": {
                            "type": "string"
                        }
                    }
                },
                "FolderListResponse": {
                    "type": "object",
                    "properties": {
                        "folders": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/Folder"
                            }
                        },
                        "total": {
                            "type": "integer"
                        }
                    }
                },
                "Folder": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "delimiter": {
                            "type": "string"
                        },
                        "attributes": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "FolderResponse": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "delimiter": {
                            "type": "string"
                        },
                        "attributes": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "CreateFolderRequest": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 255
                        },
                        "parent": {
                            "type": "string"
                        }
                    }
                },
                "UpdateFolderRequest": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 255
                        }
                    }
                },
                "EmailListResponse": {
                    "type": "object",
                    "properties": {
                        "emails": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/Email"
                            }
                        },
                        "total": {
                            "type": "integer"
                        },
                        "pagination": {
                            "$ref": "#/components/schemas/PaginationInfo"
                        }
                    }
                },
                "Email": {
                    "type": "object",
                    "properties": {
                        "uid": {
                            "type": "integer",
                            "format": "int32"
                        },
                        "subject": {
                            "type": "string"
                        },
                        "from": {
                            "type": "string"
                        },
                        "to": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "date": {
                            "type": "string",
                            "format": "date-time"
                        },
                        "flags": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "size": {
                            "type": "integer"
                        }
                    }
                },
                "EmailResponse": {
                    "type": "object",
                    "properties": {
                        "uid": {
                            "type": "integer"
                        },
                        "headers": {
                            "$ref": "#/components/schemas/EmailHeaders"
                        },
                        "body": {
                            "type": "string"
                        },
                        "attachments": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/EmailAttachment"
                            }
                        },
                        "flags": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "EmailHeaders": {
                    "type": "object",
                    "properties": {
                        "subject": {
                            "type": "string"
                        },
                        "from": {
                            "type": "string"
                        },
                        "to": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "cc": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "date": {
                            "type": "string",
                            "format": "date-time"
                        },
                        "message_id": {
                            "type": "string"
                        }
                    }
                },
                "EmailAttachment": {
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string"
                        },
                        "mime_type": {
                            "type": "string"
                        },
                        "size": {
                            "type": "integer"
                        }
                    }
                },
                "CreateEmailRequest": {
                    "type": "object",
                    "required": ["content"],
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "Base64 encoded email content"
                        },
                        "flags": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "UpdateEmailRequest": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["add", "remove", "set"]
                        },
                        "flags": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "MoveEmailRequest": {
                    "type": "object",
                    "required": ["to_folder"],
                    "properties": {
                        "to_folder": {
                            "type": "string"
                        }
                    }
                },
                "SearchRequest": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "IMAP search query"
                        },
                        "folder": {
                            "type": "string"
                        },
                        "limit": {
                            "type": "integer",
                            "default": 50
                        },
                        "offset": {
                            "type": "integer",
                            "default": 0
                        }
                    }
                },
                "CreateApiKeyRequest": {
                    "type": "object",
                    "required": ["name", "email", "imap_credentials"],
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "email": {
                            "type": "string",
                            "format": "email"
                        },
                        "imap_credentials": {
                            "$ref": "#/components/schemas/ImapCredentials"
                        },
                        "scopes": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": ["ReadEmail", "WriteEmail", "ManageFolders", "Dashboard", "Admin"]
                            }
                        }
                    }
                },
                "ImapCredentials": {
                    "type": "object",
                    "required": ["username", "password", "server", "port"],
                    "properties": {
                        "username": {
                            "type": "string"
                        },
                        "password": {
                            "type": "string",
                            "format": "password"
                        },
                        "server": {
                            "type": "string"
                        },
                        "port": {
                            "type": "integer",
                            "default": 993
                        }
                    }
                },
                "ApiKeyInfo": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "email": {
                            "type": "string"
                        },
                        "created_at": {
                            "type": "string",
                            "format": "date-time"
                        },
                        "last_used": {
                            "type": "string",
                            "format": "date-time"
                        },
                        "scopes": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "is_active": {
                            "type": "boolean"
                        }
                    }
                },
                "ApiKeyListResponse": {
                    "type": "object",
                    "properties": {
                        "keys": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/ApiKeyInfo"
                            }
                        },
                        "total": {
                            "type": "integer"
                        }
                    }
                },
                "PaginationInfo": {
                    "type": "object",
                    "properties": {
                        "total": {
                            "type": "integer"
                        },
                        "limit": {
                            "type": "integer"
                        },
                        "offset": {
                            "type": "integer"
                        },
                        "has_more": {
                            "type": "boolean"
                        }
                    }
                }
            }
        }
    })
}

/// Handler to serve the OpenAPI JSON specification
pub async fn serve_openapi_spec() -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(generate_openapi_spec()))
}

/// Configure OpenAPI documentation endpoints
pub fn configure_openapi(cfg: &mut web::ServiceConfig) {
    cfg.route("/api-docs/openapi.json", web::get().to(serve_openapi_spec));

    // Serve Swagger UI HTML
    cfg.route("/swagger-ui", web::get().to(serve_swagger_ui));
}

/// Serve Swagger UI HTML page
async fn serve_swagger_ui() -> Result<HttpResponse, actix_web::Error> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>RustyMail API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
    <style>
        html { box-sizing: border-box; overflow: -moz-scrollbars-vertical; overflow-y: scroll; }
        *, *:before, *:after { box-sizing: inherit; }
        body { margin:0; background: #fafafa; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: "/api-docs/openapi.json",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout"
            });
        };
    </script>
</body>
</html>"#;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_structure() {
        let spec = generate_openapi_spec();

        // Verify it's an object
        assert!(spec.is_object());

        // Verify required fields
        assert_eq!(spec["openapi"], "3.0.3");
        assert_eq!(spec["info"]["title"], "RustyMail API");
        assert_eq!(spec["info"]["version"], "1.0.0");

        // Verify paths exist
        assert!(spec["paths"].is_object());
        assert!(spec["paths"]["/api/v1/folders"].is_object());

        // Verify components
        assert!(spec["components"].is_object());
        assert!(spec["components"]["schemas"].is_object());
        assert!(spec["components"]["securitySchemes"].is_object());
    }

    #[test]
    fn test_error_schema() {
        let spec = generate_openapi_spec();
        let error_schema = &spec["components"]["schemas"]["ErrorResponse"];

        assert!(error_schema.is_object());
        assert_eq!(error_schema["type"], "object");
        assert!(error_schema["required"].is_array());
        assert!(error_schema["properties"]["code"].is_object());
    }
}