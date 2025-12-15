// Integration tests for operator-ui service
// These tests verify the operator-ui REST API functionality

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    // Helper to check if operator-ui is running
    async fn is_operator_ui_running() -> bool {
        reqwest::get("http://localhost:8090/health")
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    #[tokio::test]
    #[ignore] // Run only when operator-ui service is running
    async fn test_health_endpoint() {
        if !is_operator_ui_running().await {
            println!("Operator UI not running, skipping test");
            return;
        }

        let response = reqwest::get("http://localhost:8090/health")
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert_eq!(body["service"], "operator-ui");
    }

    #[tokio::test]
    #[ignore] // Run only when operator-ui service is running
    async fn test_dashboard_stats() {
        if !is_operator_ui_running().await {
            println!("Operator UI not running, skipping test");
            return;
        }

        let response = reqwest::get("http://localhost:8090/api/dashboard/stats")
            .await
            .expect("Failed to send request");

        // Should return 200 even if backend services are down (returns zeros)
        assert!(
            response.status().is_success(),
            "Expected success status, got {}",
            response.status()
        );

        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert!(body["devices"].is_object());
        assert!(body["streams"].is_object());
        assert!(body["recordings"].is_object());
    }

    #[tokio::test]
    #[ignore] // Run only when operator-ui service is running
    async fn test_incidents_list() {
        if !is_operator_ui_running().await {
            println!("Operator UI not running, skipping test");
            return;
        }

        let response = reqwest::get("http://localhost:8090/api/incidents")
            .await
            .expect("Failed to send request");

        assert!(response.status().is_success());

        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert!(body.is_array());
    }

    #[tokio::test]
    #[ignore] // Run only when operator-ui service is running
    async fn test_create_incident() {
        if !is_operator_ui_running().await {
            println!("Operator UI not running, skipping test");
            return;
        }

        let client = reqwest::Client::new();
        let incident = serde_json::json!({
            "title": "Test Incident",
            "description": "This is a test incident",
            "severity": "low",
            "source": "test"
        });

        let response = client
            .post("http://localhost:8090/api/incidents")
            .json(&incident)
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert!(body["incident"].is_object());
        assert_eq!(body["incident"]["title"], "Test Incident");
    }
}
