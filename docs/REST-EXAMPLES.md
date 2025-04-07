# IMAP API Usage Examples

This document provides code examples for using the IMAP API from various programming languages. Each example demonstrates common operations like listing folders, reading emails, and managing email content.

## Table of Contents

1. [JavaScript/TypeScript](#javascripttypescript)
2. [Python](#python)
3. [Rust](#rust)
4. [Go](#go)
5. [Java](#java)
6. [C#](#c)

## JavaScript/TypeScript

### Setup

```typescript
const API_BASE_URL = 'http://localhost:8080';
const IMAP_CREDENTIALS = btoa('username:password');

const headers = {
  'Authorization': `Basic ${IMAP_CREDENTIALS}`,
  'Content-Type': 'application/json'
};
```

### List Folders

```typescript
async function listFolders() {
  try {
    const response = await fetch(`${API_BASE_URL}/folders`, { headers });
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const folders = await response.json();
    console.log('Folders:', folders);
    return folders;
  } catch (error) {
    console.error('Error listing folders:', error);
    throw error;
  }
}
```

### List Emails in Folder

```typescript
async function listEmails(folder: string) {
  try {
    const response = await fetch(
      `${API_BASE_URL}/emails/${encodeURIComponent(folder)}`,
      { headers }
    );
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const emails = await response.json();
    console.log(`Emails in ${folder}:`, emails);
    return emails;
  } catch (error) {
    console.error('Error listing emails:', error);
    throw error;
  }
}
```

### Create Email

```typescript
async function createEmail(folder: string, emailData: {
  subject: string;
  body: { text_plain: string; text_html?: string };
  to: string[];
  cc?: string[];
  bcc?: string[];
}) {
  try {
    const response = await fetch(
      `${API_BASE_URL}/emails/${encodeURIComponent(folder)}`,
      {
        method: 'POST',
        headers,
        body: JSON.stringify(emailData)
      }
    );
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const result = await response.json();
    console.log('Email created:', result);
    return result;
  } catch (error) {
    console.error('Error creating email:', error);
    throw error;
  }
}
```

## Python

### Setup

```python
import requests
import base64

API_BASE_URL = 'http://localhost:8080'
IMAP_CREDENTIALS = base64.b64encode(b'username:password').decode('utf-8')

headers = {
    'Authorization': f'Basic {IMAP_CREDENTIALS}',
    'Content-Type': 'application/json'
}
```

### List Folders

```python
def list_folders():
    try:
        response = requests.get(f'{API_BASE_URL}/folders', headers=headers)
        response.raise_for_status()
        folders = response.json()
        print('Folders:', folders)
        return folders
    except requests.exceptions.RequestException as e:
        print('Error listing folders:', e)
        raise
```

### List Emails in Folder

```python
def list_emails(folder):
    try:
        response = requests.get(
            f'{API_BASE_URL}/emails/{folder}',
            headers=headers
        )
        response.raise_for_status()
        emails = response.json()
        print(f'Emails in {folder}:', emails)
        return emails
    except requests.exceptions.RequestException as e:
        print('Error listing emails:', e)
        raise
```

### Create Email

```python
def create_email(folder, email_data):
    try:
        response = requests.post(
            f'{API_BASE_URL}/emails/{folder}',
            headers=headers,
            json=email_data
        )
        response.raise_for_status()
        result = response.json()
        print('Email created:', result)
        return result
    except requests.exceptions.RequestException as e:
        print('Error creating email:', e)
        raise
```

## Rust

### Setup

```rust
use reqwest::{Client, header};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

const API_BASE_URL: &str = "http://localhost:8080";

fn get_headers(username: &str, password: &str) -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    let credentials = format!("{}:{}", username, password);
    let encoded = BASE64.encode(credentials);
    headers.insert(
        header::AUTHORIZATION,
        format!("Basic {}", encoded).parse().unwrap()
    );
    headers.insert(
        header::CONTENT_TYPE,
        "application/json".parse().unwrap()
    );
    headers
}
```

### List Folders

```rust
async fn list_folders(client: &Client, headers: &header::HeaderMap) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let response = client
        .get(format!("{}/folders", API_BASE_URL))
        .headers(headers.clone())
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error! status: {}", response.status()).into());
    }
    
    let folders = response.json::<Vec<String>>().await?;
    println!("Folders: {:?}", folders);
    Ok(folders)
}
```

### List Emails in Folder

```rust
async fn list_emails(client: &Client, headers: &header::HeaderMap, folder: &str) -> Result<Vec<EmailSummary>, Box<dyn std::error::Error>> {
    let response = client
        .get(format!("{}/emails/{}", API_BASE_URL, folder))
        .headers(headers.clone())
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error! status: {}", response.status()).into());
    }
    
    let emails = response.json::<Vec<EmailSummary>>().await?;
    println!("Emails in {}: {:?}", folder, emails);
    Ok(emails)
}
```

### Create Email

```rust
async fn create_email(
    client: &Client,
    headers: &header::HeaderMap,
    folder: &str,
    email_data: &EmailCreateRequest
) -> Result<EmailCreateResponse, Box<dyn std::error::Error>> {
    let response = client
        .post(format!("{}/emails/{}", API_BASE_URL, folder))
        .headers(headers.clone())
        .json(email_data)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error! status: {}", response.status()).into());
    }
    
    let result = response.json::<EmailCreateResponse>().await?;
    println!("Email created: {:?}", result);
    Ok(result)
}
```

## Go

### Setup

```go
package main

import (
    "encoding/base64"
    "fmt"
    "net/http"
    "strings"
)

const APIBaseURL = "http://localhost:8080"

func getHeaders(username, password string) map[string]string {
    credentials := username + ":" + password
    encoded := base64.StdEncoding.EncodeToString([]byte(credentials))
    return map[string]string{
        "Authorization": "Basic " + encoded,
        "Content-Type":  "application/json",
    }
}
```

### List Folders

```go
func listFolders(client *http.Client, headers map[string]string) ([]string, error) {
    req, err := http.NewRequest("GET", APIBaseURL+"/folders", nil)
    if err != nil {
        return nil, fmt.Errorf("error creating request: %v", err)
    }

    for key, value := range headers {
        req.Header.Set(key, value)
    }

    resp, err := client.Do(req)
    if err != nil {
        return nil, fmt.Errorf("error making request: %v", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return nil, fmt.Errorf("HTTP error! status: %d", resp.StatusCode)
    }

    var folders []string
    if err := json.NewDecoder(resp.Body).Decode(&folders); err != nil {
        return nil, fmt.Errorf("error decoding response: %v", err)
    }

    fmt.Printf("Folders: %v\n", folders)
    return folders, nil
}
```

### List Emails in Folder

```go
func listEmails(client *http.Client, headers map[string]string, folder string) ([]EmailSummary, error) {
    req, err := http.NewRequest("GET", APIBaseURL+"/emails/"+folder, nil)
    if err != nil {
        return nil, fmt.Errorf("error creating request: %v", err)
    }

    for key, value := range headers {
        req.Header.Set(key, value)
    }

    resp, err := client.Do(req)
    if err != nil {
        return nil, fmt.Errorf("error making request: %v", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return nil, fmt.Errorf("HTTP error! status: %d", resp.StatusCode)
    }

    var emails []EmailSummary
    if err := json.NewDecoder(resp.Body).Decode(&emails); err != nil {
        return nil, fmt.Errorf("error decoding response: %v", err)
    }

    fmt.Printf("Emails in %s: %v\n", folder, emails)
    return emails, nil
}
```

## Java

### Setup

```java
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Base64;
import java.util.Map;

public class ImapApiClient {
    private static final String API_BASE_URL = "http://localhost:8080";
    private final HttpClient client;
    private final Map<String, String> headers;

    public ImapApiClient(String username, String password) {
        this.client = HttpClient.newHttpClient();
        String credentials = username + ":" + password;
        String encoded = Base64.getEncoder().encodeToString(credentials.getBytes());
        this.headers = Map.of(
            "Authorization", "Basic " + encoded,
            "Content-Type", "application/json"
        );
    }
}
```

### List Folders

```java
public List<String> listFolders() throws Exception {
    HttpRequest request = HttpRequest.newBuilder()
        .uri(URI.create(API_BASE_URL + "/folders"))
        .headers(headers.entrySet().stream()
            .flatMap(e -> Stream.of(e.getKey(), e.getValue()))
            .toArray(String[]::new))
        .GET()
        .build();

    HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
    
    if (response.statusCode() != 200) {
        throw new RuntimeException("HTTP error! status: " + response.statusCode());
    }

    List<String> folders = new ObjectMapper().readValue(response.body(), new TypeReference<List<String>>() {});
    System.out.println("Folders: " + folders);
    return folders;
}
```

### List Emails in Folder

```java
public List<EmailSummary> listEmails(String folder) throws Exception {
    HttpRequest request = HttpRequest.newBuilder()
        .uri(URI.create(API_BASE_URL + "/emails/" + folder))
        .headers(headers.entrySet().stream()
            .flatMap(e -> Stream.of(e.getKey(), e.getValue()))
            .toArray(String[]::new))
        .GET()
        .build();

    HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
    
    if (response.statusCode() != 200) {
        throw new RuntimeException("HTTP error! status: " + response.statusCode());
    }

    List<EmailSummary> emails = new ObjectMapper().readValue(response.body(), new TypeReference<List<EmailSummary>>() {});
    System.out.println("Emails in " + folder + ": " + emails);
    return emails;
}
```

## C#

### Setup

```csharp
using System;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Threading.Tasks;

public class ImapApiClient
{
    private readonly HttpClient _client;
    private readonly string _baseUrl;

    public ImapApiClient(string baseUrl, string username, string password)
    {
        _baseUrl = baseUrl;
        _client = new HttpClient();
        
        var credentials = Convert.ToBase64String(Encoding.ASCII.GetBytes($"{username}:{password}"));
        _client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Basic", credentials);
        _client.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
    }
}
```

### List Folders

```csharp
public async Task<List<string>> ListFoldersAsync()
{
    var response = await _client.GetAsync($"{_baseUrl}/folders");
    response.EnsureSuccessStatusCode();
    
    var folders = await response.Content.ReadFromJsonAsync<List<string>>();
    Console.WriteLine($"Folders: {string.Join(", ", folders)}");
    return folders;
}
```

### List Emails in Folder

```csharp
public async Task<List<EmailSummary>> ListEmailsAsync(string folder)
{
    var response = await _client.GetAsync($"{_baseUrl}/emails/{folder}");
    response.EnsureSuccessStatusCode();
    
    var emails = await response.Content.ReadFromJsonAsync<List<EmailSummary>>();
    Console.WriteLine($"Emails in {folder}: {string.Join(", ", emails)}");
    return emails;
}
```

### Create Email

```csharp
public async Task<EmailCreateResponse> CreateEmailAsync(string folder, EmailCreateRequest emailData)
{
    var response = await _client.PostAsJsonAsync($"{_baseUrl}/emails/{folder}", emailData);
    response.EnsureSuccessStatusCode();
    
    var result = await response.Content.ReadFromJsonAsync<EmailCreateResponse>();
    Console.WriteLine($"Email created: {result}");
    return result;
}
```

## Common Data Models

### Email Summary

```typescript
interface EmailSummary {
    uid: string;
    subject: string;
    from: string;
    date: string;
    flags: string[];
    message_id?: string;
}
```

### Email Create Request

```typescript
interface EmailCreateRequest {
    subject: string;
    body: {
        text_plain: string;
        text_html?: string;
    };
    to: string[];
    cc?: string[];
    bcc?: string[];
    attachments?: {
        filename: string;
        content: string; // base64 encoded
        content_type: string;
    }[];
}
```

### Email Create Response

```typescript
interface EmailCreateResponse {
    message: string;
    uid: string;
    message_id: string;
}
``` 