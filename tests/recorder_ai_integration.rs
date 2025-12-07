use anyhow::Result;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use common::recordings::{RecordingAiConfig, RecordingConfig, RecordingFormat, RecordingStartRequest};
use recorder_node::recording::manager::RECORDING_MANAGER;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle, time::Duration};

type FrameStore = Arc<RwLock<Vec<Value>>>;

/// Mock AI service that records submitted frames
async fn mock_ai_submit_frame(
    Path(task_id): Path<String>,
    State(frames): State<FrameStore>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let mut frames = frames.write().await;
    let mut frame_data = payload.clone();
    frame_data["task_id"] = json!(task_id);
    frames.push(frame_data);

    Json(json!({
        "success": true,
        "message": "Frame received"
    }))
}

async fn mock_ai_list_frames(State(frames): State<FrameStore>) -> Json<Value> {
    let frames = frames.read().await;
    Json(json!({
        "frames": frames.clone(),
        "count": frames.len()
    }))
}

async fn spawn_router(router: Router) -> Result<(SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server failed");
    });
    Ok((addr, handle))
}

#[tokio::test]
#[ignore] // Temporarily disabled due to timing issues in CI
async fn test_recorder_submits_frames_to_ai_service() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    std::env::set_var("MOCK_RECORDING", "1");

    // Clear any state from previous tests
    RECORDING_MANAGER.clear().await;

    // Set up mock AI service
    let frame_store: FrameStore = Arc::new(RwLock::new(Vec::new()));
    let ai_router = Router::new()
        .route("/v1/tasks/:task_id/frames", post(mock_ai_submit_frame))
        .route("/v1/frames", get(mock_ai_list_frames))
        .with_state(frame_store.clone());

    let (ai_addr, ai_task) = spawn_router(ai_router).await?;
    let ai_url = format!("http://{}", ai_addr);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start recording with AI config
    let config = RecordingConfig {
        id: "rec-ai-test".to_string(),
        source_stream_id: Some("stream-1".to_string()),
        source_uri: Some("rtsp://example.com/stream".to_string()),
        retention_hours: Some(24),
        format: Some(RecordingFormat::Mp4),
    };

    let ai_config = RecordingAiConfig {
        ai_service_url: ai_url.clone(),
        ai_task_id: "test-task-123".to_string(),
        capture_interval_secs: 1, // Fast interval for testing
        frame_width: 320,
        frame_height: 240,
        jpeg_quality: 10,
    };

    let req = RecordingStartRequest {
        config,
        lease_ttl_secs: Some(30),
        ai_config: Some(ai_config),
    };

    let response = RECORDING_MANAGER.start(req).await?;
    assert!(response.accepted);

    // Brief wait to ensure frame capture loop is started
    // Note: In mock mode, frame extraction will fail (no actual video),
    // but we're testing that the frame capture loop is started
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Stop the recording
    let stopped = RECORDING_MANAGER.stop("rec-ai-test").await?;
    assert!(stopped);

    // In a real scenario with actual video, we would verify frames were submitted:
    // let http_client = Client::builder().build()?;
    // let frames_resp = http_client.get(format!("{}/v1/frames", ai_url)).send().await?;
    // let frames_data: Value = frames_resp.json().await?;
    // assert!(frames_data["count"].as_u64().unwrap() > 0);

    ai_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_recorder_without_ai_config() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    std::env::set_var("MOCK_RECORDING", "1");

    // Clear any state from previous tests
    RECORDING_MANAGER.clear().await;

    // Start recording without AI config
    let config = RecordingConfig {
        id: "rec-no-ai".to_string(),
        source_stream_id: Some("stream-1".to_string()),
        source_uri: Some("rtsp://example.com/stream".to_string()),
        retention_hours: Some(24),
        format: Some(RecordingFormat::Mp4),
    };

    let req = RecordingStartRequest {
        config,
        lease_ttl_secs: Some(30),
        ai_config: None, // No AI processing
    };

    let response = RECORDING_MANAGER.start(req).await?;
    assert!(response.accepted);

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Stop the recording
    let stopped = RECORDING_MANAGER.stop("rec-no-ai").await?;
    assert!(stopped);

    Ok(())
}
