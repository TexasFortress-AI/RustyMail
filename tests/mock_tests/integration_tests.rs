#[cfg(test)]
mod integration_tests_module {
    use actix_web::{test, App, http::StatusCode};
    use crate::api::routes::configure_routes;
    use super::common::setup_test_app_data;
    use super::mock::MockImapSession;
    use serde_json::json;

    #[actix_rt::test]
    async fn test_imap_connection() {
        let app_data = setup_test_app_data();
        let app = test::init_service(
            App::new()
                .app_data(app_data.clone())
                .configure(configure_routes::<MockImapSession>)
        ).await;

        let req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array(), "Response body should be a JSON array");
        let folders = body.as_array().unwrap();
        assert!(folders.iter().any(|v| v["name"] == "INBOX"), "INBOX folder not found in response");
    }

    #[actix_rt::test]
    async fn test_email_parsing() {
        let app_data = setup_test_app_data();
        let app = test::init_service(
            App::new()
                .app_data(app_data.clone())
                .configure(configure_routes::<MockImapSession>)
        ).await;

        let req = test::TestRequest::get().uri("/emails/INBOX").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["emails"].is_array(), "Response should contain an 'emails' array");
        assert_eq!(body["emails"].as_array().unwrap().len(), 3, "Expected 3 emails based on mock");
    }

    #[actix_rt::test]
    async fn test_folder_management() {
        let app_data = setup_test_app_data();
        let app = test::init_service(
            App::new()
                .app_data(app_data.clone())
                .configure(configure_routes::<MockImapSession>)
        ).await;

        let create_data = json!({ "name": "TestFolder" });
        let req_create = test::TestRequest::post()
            .uri("/folders")
            .set_json(&create_data)
            .to_request();
        let resp_create = test::call_service(&app, req_create).await;
        assert_eq!(resp_create.status(), StatusCode::CREATED, "Folder creation failed");
        let body_create: serde_json::Value = test::read_body_json(resp_create).await;
        assert_eq!(body_create["name"], "TestFolder", "Created folder name mismatch");

        let req_list = test::TestRequest::get().uri("/folders").to_request();
        let resp_list = test::call_service(&app, req_list).await;
        assert_eq!(resp_list.status(), StatusCode::OK, "Listing folders failed");
        let body_list: serde_json::Value = test::read_body_json(resp_list).await;
        assert!(body_list.is_array(), "List response should be an array");
        assert!(body_list.as_array().unwrap().iter().any(|v| v["name"] == "TestFolder"), "Newly created folder not found in list");

        let req_delete = test::TestRequest::delete()
            .uri("/folders/TestFolder")
            .to_request();
        let resp_delete = test::call_service(&app, req_delete).await;
        assert_eq!(resp_delete.status(), StatusCode::NO_CONTENT, "Folder deletion failed");
    }
} 