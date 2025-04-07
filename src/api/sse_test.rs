#[cfg(test)]
mod tests {
    use actix_web::{test, web, App, http::StatusCode, body::MessageBody};
    use futures_util::{StreamExt, TryStreamExt};
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::{Mutex, mpsc};
    use crate::api::sse::{SseAdapter, SseState, configure_sse_service, SseCommandPayload};
    use crate::api::rest::RestConfig;
    use tokio::time::{timeout, Duration};

    // Helper function to extract SSE events from response body
    async fn extract_sse_events(body: impl MessageBody + Unpin) -> Vec<String> {
        let mut events = Vec::new();
        let mut stream = Box::pin(body.into_stream());
        // Use a timeout to avoid waiting forever if no more events come
        while let Ok(Some(Ok(chunk))) = timeout(Duration::from_millis(100), stream.next()).await {
            if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                // Simple split, assumes messages end with \n\n
                events.extend(text.split("\n\n").filter(|s| !s.is_empty()).map(|s| s.to_string()));
            }
        }
        events
    }

    // --- Test Setup ---
    async fn setup_test_sse_app() -> (impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse>, Arc<Mutex<SseState>>) {
        let dummy_rest_config = RestConfig { host: "localhost".to_string(), port: 0 }; 
        let sse_adapter = SseAdapter::new(dummy_rest_config);
        let sse_state_arc = sse_adapter.shared_state.clone(); 

        let app = test::init_service(
            App::new()
                .app_data(web::Data::from(sse_state_arc.clone())) // Pass the Arc<Mutex<SseState>>
                .configure(|cfg| configure_sse_service(cfg, sse_state_arc.clone())) // Pass it again here
        ).await;
        (app, sse_adapter.shared_state) // Return state for checking later
    }

    // --- Tests ---

    #[actix_web::test]
    async fn test_sse_connect_and_welcome() {
        let (app, state) = setup_test_sse_app().await;
        let req = test::TestRequest::get().uri("/api/v1/sse/connect").to_request();
        let resp = test::call_service(&app, req).await;
        
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get("content-type").unwrap(), "text/event-stream");

        let events = extract_sse_events(resp.into_body()).await;
        
        // Check for welcome event
        assert!(!events.is_empty(), "No events received");
        assert!(events[0].contains("event: welcome\ndata: { \"clientId\": 1 }"), "Did not receive correct welcome event");
        
        // Check state reflects the connection
        assert_eq!(state.lock().await.clients.len(), 1);
        assert_eq!(state.lock().await.visitor_count, 1);

        // Note: Testing heartbeat/disconnect requires keeping stream alive, more complex setup
    }

    #[actix_web::test]
    async fn test_sse_post_command_accepted() {
        let (app, _) = setup_test_sse_app().await;
        
        // Post a command
        let command_payload = json!({
            "command": "test/echo",
            "params": { "value": 123 }
        });
        let cmd_req = test::TestRequest::post()
            .uri("/api/v1/sse/command")
            .set_json(&command_payload)
            .to_request();
            
        let cmd_resp = test::call_service(&app, cmd_req).await;
        
        assert_eq!(cmd_resp.status(), StatusCode::ACCEPTED);
        let body: Value = test::read_body_json(cmd_resp).await;
        assert_eq!(body["status"], "Command received");
    }

     #[actix_web::test]
    async fn test_sse_broadcast_on_command() {
        let (app, state) = setup_test_sse_app().await;

        // Simulate a client connecting and store its receiver
        // This is tricky because the server holds the sender. We need to intercept.
        // Alternative: Check state changes or side effects if broadcast triggers them.
        
        // For this test, let's check the state's broadcast function more directly (less ideal integration test)
        let (tx, mut rx) = mpsc::channel::<String>(5);
        let client_id;
        {
            let mut state_guard = state.lock().await;
            client_id = state_guard.add_client(tx); // Manually add a client to the state
        }
        assert_eq!(state.lock().await.clients.len(), 1);

        // Post a command (which should trigger broadcast in the handler)
        let command_payload = json!({
            "command": "test/broadcast",
            "params": { "data": "hello world" }
        });
        let cmd_req = test::TestRequest::post()
            .uri("/api/v1/sse/command")
            .set_json(&command_payload)
            .to_request();
            
        let cmd_resp = test::call_service(&app, cmd_req).await;
        assert_eq!(cmd_resp.status(), StatusCode::ACCEPTED);

        // Check if the manually added client received the broadcast message via its receiver
         match timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Some(msg)) => {
                let expected_data = format!("data: {}", command_payload.to_string());
                assert!(msg.starts_with(&expected_data), "Received message does not match broadcast");
            }
            Ok(None) => panic!("Client receiver channel closed unexpectedly"),
            Err(_) => panic!("Timed out waiting for broadcast message"),
        }

        // Clean up state
        state.lock().await.remove_client(client_id);
    }
} 