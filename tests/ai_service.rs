/// Integration tests for AI service
use ai_service::{
    api, plugin::mock_detector::MockDetectorPlugin, plugin::registry::PluginRegistry,
    AiServiceState,
};
use common::ai_tasks::{
    AiFrameConfig, AiOutputConfig, AiTaskConfig, AiTaskStartRequest, PluginListResponse,
    VideoFrame,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper function to create a test AI service with mock plugins
async fn setup_test_service() -> (axum::Router, AiServiceState) {
    let registry = PluginRegistry::new();

    // Register mock detector plugin
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    registry.register(mock_detector).await.unwrap();

    let state = AiServiceState::new("test-node".to_string(), registry);
    let app = api::router(state.clone());

    (app, state)
}

#[tokio::test]
async fn test_list_plugins() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/v1/plugins")
        .await;

    assert_eq!(response.status_code(), 200);

    let plugins: PluginListResponse = response.json();
    assert_eq!(plugins.plugins.len(), 1);
    assert_eq!(plugins.plugins[0].id, "mock_object_detector");
}

#[tokio::test]
async fn test_get_plugin() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/v1/plugins/mock_object_detector")
        .await;

    assert_eq!(response.status_code(), 200);

    let plugin_info: common::ai_tasks::PluginInfo = response.json();
    assert_eq!(plugin_info.id, "mock_object_detector");
    assert_eq!(plugin_info.name, "Mock Object Detector");
}

#[tokio::test]
async fn test_get_nonexistent_plugin() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/v1/plugins/nonexistent")
        .await;

    assert_eq!(response.status_code(), 404);
}

#[tokio::test]
async fn test_start_task() {
    let (app, _state) = setup_test_service().await;

    let task_config = AiTaskConfig {
        id: "test-task-1".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        source_stream_id: Some("stream-123".to_string()),
        source_recording_id: None,
        model_config: serde_json::json!({
            "confidence_threshold": 0.7
        }),
        frame_config: AiFrameConfig {
            frame_interval: 1,
            max_fps: None,
            skip_seconds: 0,
        },
        output: AiOutputConfig {
            output_type: "file".to_string(),
            config: serde_json::json!({
                "path": "/tmp/test.json"
            }),
        },
    };

    let request = AiTaskStartRequest {
        config: task_config,
        lease_ttl_secs: Some(60),
    };

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .post("/v1/tasks")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), 200);

    let start_response: common::ai_tasks::AiTaskStartResponse = response.json();
    assert!(start_response.accepted);
    assert!(start_response.lease_id.is_some());
}

#[tokio::test]
async fn test_start_task_with_invalid_plugin() {
    let (app, _state) = setup_test_service().await;

    let task_config = AiTaskConfig {
        id: "test-task-2".to_string(),
        plugin_type: "nonexistent_plugin".to_string(),
        input_stream_id: Some("stream-123".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 1,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/test.json".to_string(),
        },
    };

    let request = AiTaskStartRequest {
        config: task_config,
        lease_ttl_secs: Some(60),
    };

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .post("/v1/tasks")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), 400);

    let start_response: common::ai_tasks::AiTaskStartResponse = response.json();
    assert!(!start_response.accepted);
}

#[tokio::test]
async fn test_list_tasks() {
    let (app, state) = setup_test_service().await;

    // Start a task first
    let task_config = AiTaskConfig {
        id: "test-task-3".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        input_stream_id: Some("stream-123".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 1,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/test.json".to_string(),
        },
    };

    state.start_task(task_config, Some(60)).await.unwrap();

    // List tasks
    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/v1/tasks")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    let tasks = body["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn test_get_task() {
    let (app, state) = setup_test_service().await;

    // Start a task first
    let task_config = AiTaskConfig {
        id: "test-task-4".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        input_stream_id: Some("stream-123".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 1,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/test.json".to_string(),
        },
    };

    state.start_task(task_config, Some(60)).await.unwrap();

    // Get task
    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/v1/tasks/test-task-4")
        .await;

    assert_eq!(response.status_code(), 200);

    let task_info: common::ai_tasks::AiTaskInfo = response.json();
    assert_eq!(task_info.config.id, "test-task-4");
    assert_eq!(task_info.state, common::ai_tasks::AiTaskState::Processing);
}

#[tokio::test]
async fn test_stop_task() {
    let (app, state) = setup_test_service().await;

    // Start a task first
    let task_config = AiTaskConfig {
        id: "test-task-5".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        input_stream_id: Some("stream-123".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 1,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/test.json".to_string(),
        },
    };

    state.start_task(task_config, Some(60)).await.unwrap();

    // Stop task
    let response = axum_test::TestServer::new(app)
        .unwrap()
        .delete("/v1/tasks/test-task-5")
        .await;

    assert_eq!(response.status_code(), 200);

    let stop_response: common::ai_tasks::AiTaskStopResponse = response.json();
    assert!(stop_response.success);

    // Verify task is stopped
    let task_info = state.get_task("test-task-5").await.unwrap();
    assert_eq!(task_info.state, common::ai_tasks::AiTaskState::Stopped);
}

#[tokio::test]
async fn test_healthz() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/healthz")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_readyz() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/readyz")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let (app, _state) = setup_test_service().await;

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/metrics")
        .await;

    // Just verify the endpoint is accessible
    assert_eq!(response.status_code(), 200);
}

#[tokio::test]
async fn test_submit_frame() {
    let (app, state) = setup_test_service().await;

    // Start a task first
    let task_config = AiTaskConfig {
        id: "test-task-frame".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        input_stream_id: Some("stream-123".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 1,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/test.json".to_string(),
        },
    };

    state.start_task(task_config, Some(60)).await.unwrap();

    // Create a test frame (small JPEG header as base64)
    let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg_data);

    let frame = VideoFrame {
        source_id: "stream-123".to_string(),
        timestamp: 1234567890,
        sequence: 1,
        width: 640,
        height: 480,
        format: "jpeg".to_string(),
        data: base64_data,
    };

    // Submit frame
    let response = axum_test::TestServer::new(app)
        .unwrap()
        .post("/v1/tasks/test-task-frame/frames")
        .json(&frame)
        .await;

    assert_eq!(response.status_code(), 200);

    let result: common::ai_tasks::AiResult = response.json();
    assert_eq!(result.task_id, "test-task-frame");
    assert_eq!(result.plugin_type, "mock_object_detector");

    // Mock detector should return 2 detections
    assert_eq!(result.detections.len(), 2);

    // Verify task stats were updated
    let task_info = state.get_task("test-task-frame").await.unwrap();
    assert_eq!(task_info.frames_processed, 1);
    assert_eq!(task_info.detections_made, 2);
}

#[tokio::test]
async fn test_submit_frame_to_nonexistent_task() {
    let (app, _state) = setup_test_service().await;

    let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg_data);

    let frame = VideoFrame {
        source_id: "stream-123".to_string(),
        timestamp: 1234567890,
        sequence: 1,
        width: 640,
        height: 480,
        format: "jpeg".to_string(),
        data: base64_data,
    };

    // Submit frame to non-existent task
    let response = axum_test::TestServer::new(app)
        .unwrap()
        .post("/v1/tasks/nonexistent-task/frames")
        .json(&frame)
        .await;

    assert_eq!(response.status_code(), 400);
}
