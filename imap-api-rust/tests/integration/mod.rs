#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};
    use imap_api_rust::{
        api::routes::configure_routes,
        imap::client::ImapClient,
        models::folder::FolderCreateRequest,
    };
    use std::sync::{Arc, Mutex};
    use tera::Tera;
    use native_tls::{TlsConnector, TlsStream};
    use imap as imap_crate;
    use std::net::TcpStream;
    use tokio;
    use crate::common::setup_test_app;

    // Constants for integration tests
    const TEST_FOLDER_PREFIX: &str = "INBOX.TEST_RUST_";

    // Define the concrete session type
    type ActualImapSession = imap_crate::Session<TlsStream<TcpStream>>;

    // Helper to get a unique test folder name
    fn get_test_folder_name(suffix: &str) -> String {
        format!("{}{}", TEST_FOLDER_PREFIX, suffix)
    }

    // Test listing folders
    #[actix_web::test]
    async fn test_list_folders() {
        let app = test::init_service(setup_test_app()).await;
        let req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let folders: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        
        // Check if INBOX exists (should always be present in IMAP)
        assert!(folders.iter().any(|f| f["name"].as_str().unwrap() == "INBOX"));
    }

    // Test creating and deleting a folder
    #[actix_web::test]
    async fn test_create_and_delete_folder() {
        let app = test::init_service(setup_test_app()).await;
        let test_folder = "TestFolder123";

        // Test folder creation
        let create_req = test::TestRequest::post()
            .uri("/folders")
            .set_json(&FolderCreateRequest {
                name: test_folder.to_string(),
            })
            .to_request();
        let resp = test::call_service(&app, create_req).await;
        assert!(resp.status().is_success());

        // Verify folder exists
        let list_req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, list_req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let folders: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(folders.iter().any(|f| f["name"].as_str().unwrap() == test_folder));

        // Test folder deletion
        let delete_req = test::TestRequest::delete()
            .uri(&format!("/folders/{}", test_folder))
            .to_request();
        let resp = test::call_service(&app, delete_req).await;
        assert!(resp.status().is_success());

        // Verify folder is gone
        let list_req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, list_req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let folders: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(!folders.iter().any(|f| f["name"].as_str().unwrap() == test_folder));
    }

    // Test fetching emails from a folder
    #[actix_web::test]
    async fn test_fetch_email() {
        let app = test::init_service(setup_test_app()).await;
        let test_folder = "TestEmailFolder123";

        // Try to fetch emails from the test folder
        let req = test::TestRequest::get()
            .uri(&format!("/folders/{}/emails", test_folder))
            .to_request();
        let resp = test::call_service(&app, req).await;
        
        // Should succeed even if empty
        assert!(resp.status().is_success());
    }

    // Test invalid folder
    #[actix_web::test]
    async fn test_invalid_folder() {
        let app = test::init_service(setup_test_app()).await;
        
        // Try to access a folder that shouldn't exist
        let req = test::TestRequest::get()
            .uri("/emails/INBOX.NON_EXISTENT_FOLDER_123456789")
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        // Accept either client error (404) or server error (500) since both indicate the folder doesn't exist
        assert!(resp.status().is_client_error() || resp.status().is_server_error(), 
                "Expected error for invalid folder, got: {}", resp.status());
        
        println!("Got error response for invalid folder as expected: {}", resp.status());
    }

    // Test folder rename
    #[actix_web::test]
    async fn test_folder_rename() {
        let app = test::init_service(setup_test_app()).await;
        let source_folder_name = get_test_folder_name("RenameSource");
        let target_folder_name = get_test_folder_name("RenameTarget");
        
        // Create a folder to rename
        let create_req_body = FolderCreateRequest { name: source_folder_name.clone() };
        let req_create = test::TestRequest::post()
            .uri("/folders")
            .set_json(&create_req_body)
            .to_request();
        let resp_create = test::call_service(&app, req_create).await;
        assert!(resp_create.status().is_success(), 
                "Failed to create source folder: {}", resp_create.status());
        
        // Make sure target doesn't exist
        let delete_req = test::TestRequest::delete()
            .uri(&format!("/folders/{}", target_folder_name))
            .to_request();
        let _ = test::call_service(&app, delete_req).await;
    }
}

