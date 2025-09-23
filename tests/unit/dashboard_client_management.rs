// Unit tests for dashboard client management functionality
#[cfg(test)]
mod tests {
    use rustymail::dashboard::services::clients::ClientManager;
    use rustymail::dashboard::api::models::{ClientType, ClientStatus};
    use std::time::Duration;

    #[tokio::test]
    async fn test_client_registration() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Register an SSE client
        let client_id = manager.register_client(
            ClientType::Sse,
            Some("127.0.0.1".to_string()),
            Some("Mozilla/5.0".to_string()),
        ).await;

        assert!(!client_id.is_empty());

        // Verify client exists
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.pagination.total, 1);
        assert_eq!(clients.clients[0].id, client_id);
        assert_eq!(clients.clients[0].r#type, ClientType::Sse);
    }

    #[tokio::test]
    async fn test_client_activity_update() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Register a client
        let client_id = manager.register_client(
            ClientType::Api,
            Some("192.168.1.1".to_string()),
            Some("curl/7.68.0".to_string()),
        ).await;

        // Update activity
        manager.update_client_activity(&client_id).await;

        // Get client info - check if it's in the list
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.pagination.total, 1);

        let info = &clients.clients[0];
        assert_eq!(info.id, client_id);
        assert_eq!(info.status, ClientStatus::Active);
    }

    #[tokio::test]
    async fn test_client_removal() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Register and then remove a client
        let client_id = manager.register_client(
            ClientType::Console,
            None,
            None,
        ).await;

        manager.remove_client(&client_id).await;

        // Verify client is gone
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.pagination.total, 0);
    }

    #[tokio::test]
    async fn test_multiple_client_types() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Register different client types
        let _sse_id = manager.register_client(ClientType::Sse, None, None).await;
        let _api_id = manager.register_client(ClientType::Api, None, None).await;
        let _console_id = manager.register_client(ClientType::Console, None, None).await;

        // List all clients
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.pagination.total, 3);

        // We can't filter by type with the current API since filter is just a string
        // This would need enhancement to the ClientManager to support type filtering
    }

    #[tokio::test]
    async fn test_client_status_transitions() {
        let manager = ClientManager::new(Duration::from_secs(60));

        let client_id = manager.register_client(
            ClientType::Sse,
            Some("10.0.0.1".to_string()),
            Some("Chrome/96.0".to_string()),
        ).await;

        // Initial status should be Active
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.clients[0].status, ClientStatus::Active);

        // Update status to Idle
        manager.update_client_status(&client_id, ClientStatus::Idle).await;
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.clients[0].status, ClientStatus::Idle);

        // Update to Disconnecting
        manager.update_client_status(&client_id, ClientStatus::Disconnecting).await;
        let clients = manager.get_clients(1, 10, None).await;
        assert_eq!(clients.clients[0].status, ClientStatus::Disconnecting);
    }

    #[tokio::test]
    async fn test_pagination() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Register 15 clients
        for i in 0..15 {
            manager.register_client(
                ClientType::Api,
                Some(format!("10.0.0.{}", i)),
                None,
            ).await;
        }

        // Test pagination
        let page1 = manager.get_clients(1, 10, None).await;
        assert_eq!(page1.clients.len(), 10);
        assert_eq!(page1.pagination.page, 1);
        assert_eq!(page1.pagination.total, 15);
        assert_eq!(page1.pagination.total_pages, 2);

        let page2 = manager.get_clients(2, 10, None).await;
        assert_eq!(page2.clients.len(), 5);
        assert_eq!(page2.pagination.page, 2);
    }

    #[tokio::test]
    async fn test_client_count() {
        let manager = ClientManager::new(Duration::from_secs(60));

        // Initially should have 0 clients
        assert_eq!(manager.get_client_count().await, 0);

        // Register some clients
        manager.register_client(ClientType::Sse, None, None).await;
        manager.register_client(ClientType::Api, None, None).await;

        // Should now have 2 clients
        assert_eq!(manager.get_client_count().await, 2);
    }
}