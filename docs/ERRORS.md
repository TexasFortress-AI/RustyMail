# IMAP API Error Documentation

This document provides a comprehensive list of error codes and error messages that may be returned by the IMAP API, along with explanations and troubleshooting tips.

## Error Response Format

All error responses follow a consistent JSON format:

```json
{
  "error": "Error message describing what went wrong"
}
```

For some specific error types, additional fields may be included:

```json
{
  "error": "Folder not empty",
  "message_count": 5
}
```

## HTTP Status Codes

| Status Code | Description | Common Causes |
|-------------|-------------|--------------|
| 400 | Bad Request | Invalid input parameters, malformed JSON, missing required fields |
| 401 | Unauthorized | Invalid credentials, expired session |
| 403 | Forbidden | Insufficient permissions to perform the operation |
| 404 | Not Found | Resource (folder, email) does not exist |
| 409 | Conflict | Resource already exists, folder not empty |
| 422 | Unprocessable Entity | Request is well-formed but cannot be processed due to semantic errors |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Unexpected server error, backend service failure |
| 502 | Bad Gateway | IMAP server error or unavailable |
| 503 | Service Unavailable | API service temporarily overloaded or down for maintenance |
| 504 | Gateway Timeout | IMAP server timeout |

## Error Categories

### Authentication Errors

| Error Code | Message | Description | Troubleshooting |
|------------|---------|-------------|----------------|
| AUTH_001 | "Authentication failed" | Invalid username or password | Verify your IMAP credentials |
| AUTH_002 | "Invalid token" | The authentication token is invalid or expired | Re-authenticate to obtain a new token |
| AUTH_003 | "Authentication required" | No authentication provided for a protected endpoint | Include proper authentication headers |

### Folder Management Errors

| Error Code | Message | Description | Troubleshooting |
|------------|---------|-------------|----------------|
| FOLDER_001 | "Folder not found: {folder_name}" | The specified folder does not exist | Verify the folder name and check for typos |
| FOLDER_002 | "Folder already exists: {folder_name}" | Attempting to create a folder that already exists | Use a different folder name or check if the folder already exists before creating |
| FOLDER_003 | "Folder not empty: {message_count} messages" | Attempting to delete a folder that contains messages | Move or delete all messages from the folder first |
| FOLDER_004 | "Invalid folder name: {folder_name}" | Folder name contains invalid characters | Avoid using special characters like `/`, `*`, `%`, `"`, etc. |
| FOLDER_005 | "Cannot rename special folder: {folder_name}" | Attempting to rename a system folder like INBOX | System folders cannot be renamed, try creating a new folder |
| FOLDER_006 | "Folder creation failed: {reason}" | Server rejected folder creation | Check server permissions and folder name constraints |

### Email Management Errors

| Error Code | Message | Description | Troubleshooting |
|------------|---------|-------------|----------------|
| EMAIL_001 | "Email not found: UID {uid}" | The specified email does not exist | Verify the email UID and folder name |
| EMAIL_002 | "Invalid email format" | Malformed email during creation | Check the email format, particularly for attachments |
| EMAIL_003 | "Attachment too large: {size}" | Attachment exceeds size limits | Reduce attachment size or split into multiple emails |
| EMAIL_004 | "Move operation failed: {reason}" | Failed to move email between folders | Verify source and destination folders exist and have appropriate permissions |
| EMAIL_005 | "Invalid email fields: {fields}" | Required or malformed email fields | Check that required fields (to, subject) are present and formatted correctly |
| EMAIL_006 | "Email fetch failed: {reason}" | Failed to retrieve email content | Verify the UID and try again, or check if the email has been deleted |

### IMAP Server Errors

| Error Code | Message | Description | Troubleshooting |
|------------|---------|-------------|----------------|
| IMAP_001 | "Connection to IMAP server failed" | Cannot establish connection to IMAP server | Check server address and port, ensure the server is running |
| IMAP_002 | "IMAP command failed: {command}" | Server rejected an IMAP command | Review the specific command and any constraints |
| IMAP_003 | "IMAP server timeout" | Server did not respond in time | Try again later, the server might be overloaded |
| IMAP_004 | "IMAP TLS/SSL error" | Secure connection failed | Check SSL/TLS configuration and certificates |
| IMAP_005 | "IMAP server disconnected" | Connection was lost during operation | Retry the operation, connection might have been closed by the server |
| IMAP_006 | "IMAP quota exceeded" | Mailbox quota has been reached | Delete unnecessary emails to free up space |

### General API Errors

| Error Code | Message | Description | Troubleshooting |
|------------|---------|-------------|----------------|
| API_001 | "Missing required parameter: {param}" | A required parameter is missing in the request | Add the missing parameter to your request |
| API_002 | "Invalid parameter: {param}" | A parameter has an invalid value | Check the parameter format and constraints |
| API_003 | "Rate limit exceeded" | Too many requests in a short time | Reduce request frequency or implement backoff strategy |
| API_004 | "Internal server error" | Unexpected error occurred | Report the issue with details of your request |
| API_005 | "Service unavailable" | API service is temporary down | Try again later |
| API_006 | "Invalid JSON format" | The request body is not valid JSON | Check your JSON syntax |

## Error Handling Best Practices

When handling errors in your client application:

1. **Check HTTP status codes first** - Handle different status codes appropriately
2. **Parse and log error messages** - Extract the error message for troubleshooting
3. **Implement retry logic** - For transient errors (timeout, temporary server issues)
4. **Backoff strategy** - If rate limited (429), implement exponential backoff
5. **User-friendly messages** - Translate technical errors to user-friendly messages

## Example Error Handling

### JavaScript Example

```javascript
async function fetchFolders() {
  try {
    const response = await fetch('/folders', {
      headers: { 'Authorization': 'Basic ' + btoa(username + ':' + password) }
    });
    
    if (!response.ok) {
      const errorData = await response.json();
      
      // Handle specific errors
      if (response.status === 401) {
        // Authentication error
        console.error('Authentication failed. Please check your credentials.');
        // Prompt user to login again
      } else if (response.status === 429) {
        // Rate limiting
        console.error('Rate limit exceeded. Trying again in 30 seconds.');
        // Implement retry with backoff
        setTimeout(fetchFolders, 30000);
      } else {
        // General error
        console.error(`Error: ${errorData.error}`);
      }
      
      return null;
    }
    
    const data = await response.json();
    return data;
  } catch (error) {
    console.error('Network error:', error);
    return null;
  }
}
```

### Rust Example

```rust
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

async fn fetch_folders(client: &Client, api_base: &str, auth: &str) -> Result<Vec<Folder>, String> {
    let response = client
        .get(&format!("{}/folders", api_base))
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    
    match response.status() {
        StatusCode::OK => {
            let folders = response.json::<Vec<Folder>>().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            Ok(folders)
        },
        StatusCode::UNAUTHORIZED => {
            Err("Authentication failed. Please check your credentials.".to_string())
        },
        StatusCode::TOO_MANY_REQUESTS => {
            Err("Rate limit exceeded. Please try again later.".to_string())
        },
        _ => {
            let error = response.json::<ErrorResponse>().await
                .map_err(|_| "Unknown error".to_string())?;
            Err(error.error)
        }
    }
}
```

## Reporting Errors

When reporting errors to the API developers, please include:

1. The HTTP status code
2. The full error message and code
3. Details of your request (URL, headers, body - with sensitive information redacted)
4. Time and date of the error
5. Any patterns or reproducible steps

This information helps in diagnosing and fixing the issue quickly. 