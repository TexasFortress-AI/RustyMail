#[cfg(test)]
mod tests {
    use actix_web::test;
    use imap_api_rust::models::folder::FolderCreateRequest;
    use crate::mocks::common::setup_mock_test_app;

    #[actix_web::test]
    async fn test_mock_list_folders() {
        let app = test::init_service(setup_mock_test_app()).await;
        let req = test::TestRequest::get().uri("/folders").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_mock_create_folder() {
        let app = test::init_service(setup_mock_test_app()).await;
        let req = test::TestRequest::post()
            .uri("/folders")
            .set_json(&FolderCreateRequest {
                name: "TestFolder".to_string(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
} 