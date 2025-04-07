# IMAP API Documentation

This document provides detailed information on all available endpoints in the IMAP API, including request and response formats.

## Table of Contents

1. [Base Endpoints](#base-endpoints)
2. [Folder Management](#folder-management)
3. [Email Management](#email-management)
4. [Authentication](#authentication)
5. [Error Handling](#error-handling)

## Base Endpoints

### Get Homepage

Returns the API documentation as HTML.

- **URL**: `/`
- **Method**: `GET`
- **Auth required**: No
- **Parameters**: None

#### Success Response

- **Code**: `200 OK`
- **Content**: HTML documentation

### Get API Documentation

Returns the API documentation in JSON format.

- **URL**: `/api-docs`
- **Method**: `GET`
- **Auth required**: No
- **Parameters**: None

#### Success Response

- **Code**: `200 OK`
- **Content**: JSON documentation

## Folder Management

### List All Folders

Returns a list of all IMAP folders.

- **URL**: `/folders`
- **Method**: `GET`
- **Auth required**: Yes
- **Parameters**: None

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
[
  {
    "name": "INBOX",
    "has_children": true,
    "flags": ["\\HasNoChildren"]
  },
  {
    "name": "Sent",
    "has_children": false,
    "flags": ["\\HasNoChildren"]
  },
  {
    "name": "Drafts",
    "has_children": false,
    "flags": ["\\HasNoChildren"]
  }
]
```

### Create Folder

Creates a new IMAP folder.

- **URL**: `/folders`
- **Method**: `POST`
- **Auth required**: Yes
- **Parameters**:

```json
{
  "name": "NewFolder"
}
```

#### Success Response

- **Code**: `201 Created`
- **Content**: 

```json
{
  "message": "Folder created successfully",
  "name": "NewFolder"
}
```

### Delete Folder

Deletes an empty IMAP folder.

- **URL**: `/folders/{folder_name}`
- **Method**: `DELETE`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder to delete

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "message": "Folder deleted successfully"
}
```

#### Error Response

- **Code**: `400 Bad Request`
- **Content**: 

```json
{
  "error": "Folder not empty: 5 messages"
}
```

### Rename Folder

Renames an existing IMAP folder.

- **URL**: `/folders/{folder_name}/rename`
- **Method**: `PUT`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder to rename
- **Body Parameters**:

```json
{
  "new_name": "RenamedFolder"
}
```

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "message": "Folder renamed successfully",
  "old_name": "OldFolderName",
  "new_name": "RenamedFolder"
}
```

### Get Folder Statistics

Returns statistics for a specific folder.

- **URL**: `/folders/{folder_name}/stats`
- **Method**: `GET`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "name": "INBOX",
  "total_messages": 42,
  "unread_messages": 7,
  "size_bytes": 2345678,
  "first_message_date": "2023-01-01T10:00:00Z",
  "last_message_date": "2023-06-15T15:30:00Z"
}
```

## Email Management

### List Emails in Folder

Returns a list of emails in a specific folder.

- **URL**: `/emails/{folder_name}`
- **Method**: `GET`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder
- **Query Parameters**:
  - `limit` (optional): Maximum number of emails to return (default: 50)
  - `offset` (optional): Offset for pagination (default: 0)
  - `sort` (optional): Sort order (default: "date:desc")

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "emails": [
    {
      "uid": "1234",
      "subject": "Hello World",
      "from": "sender@example.com",
      "date": "2023-06-15T12:34:56Z",
      "flags": ["\\Seen"],
      "message_id": "<abc123@example.com>"
    },
    {
      "uid": "1235",
      "subject": "Meeting Reminder",
      "from": "office@example.com",
      "date": "2023-06-14T09:12:34Z",
      "flags": ["\\Seen", "\\Flagged"],
      "message_id": "<def456@example.com>"
    }
  ],
  "total": 42,
  "offset": 0,
  "limit": 50
}
```

### Get Email

Returns a specific email by UID.

- **URL**: `/emails/{folder_name}/{uid}`
- **Method**: `GET`
- **Auth required**: Yes
- **URL Parameters**: 
  - `folder_name` - Name of the folder containing the email
  - `uid` - UID of the email

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "uid": "1234",
  "subject": "Hello World",
  "from": "sender@example.com",
  "to": ["recipient@example.com"],
  "cc": ["other@example.com"],
  "date": "2023-06-15T12:34:56Z",
  "flags": ["\\Seen"],
  "message_id": "<abc123@example.com>",
  "body": {
    "text_plain": "This is a plain text email body",
    "text_html": "<html><body><p>This is an HTML email body</p></body></html>"
  },
  "attachments": [
    {
      "filename": "document.pdf",
      "content_type": "application/pdf",
      "size_bytes": 12345
    }
  ]
}
```

### List Unread Emails

Returns a list of unread emails in a folder.

- **URL**: `/emails/{folder_name}/unread`
- **Method**: `GET`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder
- **Query Parameters**:
  - `limit` (optional): Maximum number of emails to return (default: 50)
  - `offset` (optional): Offset for pagination (default: 0)

#### Success Response

- **Code**: `200 OK`
- **Content**: Same format as "List Emails in Folder"

### Move Email

Moves an email from one folder to another.

- **URL**: `/emails/move`
- **Method**: `POST`
- **Auth required**: Yes
- **Body Parameters**:

```json
{
  "uid": "1234",
  "source_folder": "INBOX",
  "dest_folder": "Archive"
}
```

#### Success Response

- **Code**: `200 OK`
- **Content**: 

```json
{
  "message": "Email moved successfully"
}
```

### Create Email

Creates a new email and adds it to a folder.

- **URL**: `/emails/{folder_name}`
- **Method**: `POST`
- **Auth required**: Yes
- **URL Parameters**: `folder_name` - Name of the folder where the email will be stored
- **Body Parameters**:

```json
{
  "subject": "New Email Subject",
  "body": {
    "text_plain": "Plain text content",
    "text_html": "<html><body><p>HTML content</p></body></html>"
  },
  "to": ["recipient@example.com"],
  "cc": ["cc@example.com"],
  "bcc": ["bcc@example.com"],
  "attachments": [
    {
      "filename": "attachment.pdf",
      "content": "base64_encoded_content",
      "content_type": "application/pdf"
    }
  ]
}
```

#### Success Response

- **Code**: `201 Created`
- **Content**: 

```json
{
  "message": "Email created successfully",
  "uid": "1236",
  "message_id": "<xyz789@example.com>"
}
```

## Authentication

This API uses HTTP Basic Authentication with the IMAP credentials.

Include the following header in all requests:

```
Authorization: Basic base64(username:password)
```

Where `base64(username:password)` is the Base64 encoding of `username:password`.

## Error Handling

All errors are returned in a consistent JSON format:

```json
{
  "error": "Error message describing what went wrong"
}
```

Common HTTP status codes:

- `400 Bad Request` - Invalid request parameters
- `401 Unauthorized` - Authentication failure
- `404 Not Found` - Resource not found (folder, email, etc.)
- `409 Conflict` - Operation cannot be performed due to the current state (e.g., folder not empty)
- `500 Internal Server Error` - Unexpected server error

For detailed error handling, see [Error Documentation](ERRORS.md). 