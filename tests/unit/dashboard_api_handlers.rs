#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, http::StatusCode};
    use rustymail::dashboard::{
        api::{
            handlers::*,
            models::{ChatbotQuery, ChatbotResponse, ServerConfig},
        },
        services::{
            DashboardState,
            metrics::MetricsService,
            clients::ClientManager,
            config::ConfigService,
            ai::AiService,
            email::EmailService,
        },
    };
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use serde_json::{json, Value};

    async fn create_test_state() -> web::Data<DashboardState> {
        let metrics_service = Arc::new(MetricsService::new());
        let client_manager = Arc::new(ClientManager::new());
        let config_service = Arc::new(ConfigService::new());
        let email_service = Arc::new(EmailService::new_mock());
        let ai_service = Arc::new(AiService::new());

        web::Data::new(DashboardState {
            metrics_service,
            client_manager,
            config_service,
            ai_service,
            email_service,
            event_manager: Arc::new(RwLock::new(Default::default())),
        })
    }

    #[actix_web::test]
    async fn test_get_dashboard_stats() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/stats", web::get().to(get_dashboard_stats))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/stats")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("total_messages").is_some());
        assert!(body.get("active_connections").is_some());
        assert!(body.get("total_folders").is_some());
    }

    #[actix_web::test]
    async fn test_get_connected_clients() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/clients", web::get().to(get_connected_clients))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/clients?page=1&limit=10")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("clients").is_some());
        assert!(body.get("total").is_some());
        assert!(body.get("page").is_some());
    }

    #[actix_web::test]
    async fn test_get_connected_clients_invalid_page() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/clients", web::get().to(get_connected_clients))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/clients?page=0&limit=10")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_get_configuration() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/config", web::get().to(get_configuration))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/config")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: ServerConfig = test::read_body_json(resp).await;
        assert!(body.imap_adapter.is_some());
        assert!(body.available_adapters.is_some());
    }

    #[actix_web::test]
    async fn test_query_chatbot() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/chatbot/query", web::post().to(query_chatbot))
        ).await;

        let query = ChatbotQuery {
            query: "What folders do I have?".to_string(),
            context: None,
            model_override: None,
            provider_override: None,
        };

        let req = test::TestRequest::post()
            .uri("/api/dashboard/chatbot/query")
            .set_json(&query)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: ChatbotResponse = test::read_body_json(resp).await;
        assert!(!body.response.is_empty());
    }

    #[actix_web::test]
    async fn test_list_mcp_tools() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/tools", web::get().to(list_mcp_tools))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/mcp/tools")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        let tools = body.get("tools").unwrap().as_array().unwrap();
        assert!(!tools.is_empty());

        // Verify some expected tools are present
        let tool_names: Vec<String> = tools.iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .map(|s| s.to_string())
            .collect();

        assert!(tool_names.contains(&"list_folders".to_string()));
        assert!(tool_names.contains(&"search_emails".to_string()));
        assert!(tool_names.contains(&"atomic_move_message".to_string()));
    }

    #[actix_web::test]
    async fn test_execute_mcp_tool_list_folders() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/execute", web::post().to(execute_mcp_tool))
        ).await;

        let request = json!({
            "tool": "list_folders",
            "parameters": {}
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body.get("success").unwrap(), true);
        assert!(body.get("data").is_some());
    }

    #[actix_web::test]
    async fn test_execute_mcp_tool_invalid() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/execute", web::post().to(execute_mcp_tool))
        ).await;

        let request = json!({
            "tool": "invalid_tool_name",
            "parameters": {}
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_execute_mcp_tool_missing_params() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/execute", web::post().to(execute_mcp_tool))
        ).await;

        let request = json!({
            // Missing "tool" field
            "parameters": {}
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_get_client_subscriptions() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/client/:id/subscriptions", web::get().to(get_client_subscriptions))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/client/test-client-123/subscriptions")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("subscriptions").is_some());
    }

    #[actix_web::test]
    async fn test_update_client_subscriptions() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/client/:id/subscriptions", web::put().to(update_client_subscriptions))
        ).await;

        let subscriptions = json!({
            "subscriptions": ["stats", "clients", "config"]
        });

        let req = test::TestRequest::put()
            .uri("/api/dashboard/client/test-client-123/subscriptions")
            .set_json(&subscriptions)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_get_available_event_types() {
        let app = test::init_service(
            App::new()
                .route("/api/dashboard/events/types", web::get().to(get_available_event_types))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/events/types")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        let types = body.get("event_types").unwrap().as_array().unwrap();
        assert!(!types.is_empty());
    }

    #[actix_web::test]
    async fn test_subscribe_to_event() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/events/subscribe", web::post().to(subscribe_to_event))
        ).await;

        let subscription = json!({
            "client_id": "test-client-123",
            "event_type": "stats"
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/events/subscribe")
            .set_json(&subscription)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_unsubscribe_from_event() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/events/unsubscribe", web::post().to(unsubscribe_from_event))
        ).await;

        let unsubscribe = json!({
            "client_id": "test-client-123",
            "event_type": "stats"
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/events/unsubscribe")
            .set_json(&unsubscribe)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_get_ai_providers() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/ai/providers", web::get().to(get_ai_providers))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/ai/providers")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("providers").is_some());
        assert!(body.get("current_provider").is_some());
    }

    #[actix_web::test]
    async fn test_set_ai_provider() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/ai/provider", web::post().to(set_ai_provider))
        ).await;

        let provider = json!({
            "provider": "openai",
            "api_key": "test-key"
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/ai/provider")
            .set_json(&provider)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_get_ai_models() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/ai/models", web::get().to(get_ai_models))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/ai/models")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("models").is_some());
    }

    #[actix_web::test]
    async fn test_set_ai_model() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/ai/model", web::post().to(set_ai_model))
        ).await;

        let model = json!({
            "model": "gpt-3.5-turbo"
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/ai/model")
            .set_json(&model)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_chatbot_query_with_context() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/chatbot/query", web::post().to(query_chatbot))
        ).await;

        let query = ChatbotQuery {
            query: "Search for emails from John".to_string(),
            context: Some(json!({"previous_query": "List folders"})),
            model_override: Some("gpt-4".to_string()),
            provider_override: Some("openai".to_string()),
        };

        let req = test::TestRequest::post()
            .uri("/api/dashboard/chatbot/query")
            .set_json(&query)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_execute_mcp_tool_search_emails() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/execute", web::post().to(execute_mcp_tool))
        ).await;

        let request = json!({
            "tool": "search_emails",
            "parameters": {
                "folder": "INBOX",
                "query": "FROM john@example.com",
                "max_results": 10
            }
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("success").is_some());
    }

    #[actix_web::test]
    async fn test_execute_mcp_tool_batch_operations() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/mcp/execute", web::post().to(execute_mcp_tool))
        ).await;

        let request = json!({
            "tool": "atomic_batch_move",
            "parameters": {
                "source_folder": "INBOX",
                "target_folder": "Archive",
                "uids": "1,2,3,4,5"
            }
        });

        let req = test::TestRequest::post()
            .uri("/api/dashboard/mcp/execute")
            .set_json(&request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_get_connected_clients_with_filter() {
        let state = create_test_state().await;
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .route("/api/dashboard/clients", web::get().to(get_connected_clients))
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/dashboard/clients?page=1&limit=5&filter=active")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body.get("page").unwrap(), 1);
        assert!(body.get("limit").is_some());
    }
}