//! Integration tests for stream-node
//!
//! These tests verify that the stream-node integrates correctly with
//! other services like the coordinator.

use anyhow::Result;
use common::streams::{StreamConfig, StreamStartRequest, StreamState};

#[tokio::test]
async fn test_stream_config_serialization() -> Result<()> {
    let config = StreamConfig {
        id: "stream-1".to_string(),
        camera_id: Some("cam-001".to_string()),
        uri: "rtsp://example.com/stream".to_string(),
        codec: Some("h264".to_string()),
        container: Some("ts".to_string()),
    };

    let serialized = serde_json::to_string(&config)?;
    let deserialized: StreamConfig = serde_json::from_str(&serialized)?;

    assert_eq!(config.id, deserialized.id);
    assert_eq!(config.uri, deserialized.uri);
    assert_eq!(config.codec, deserialized.codec);
    assert_eq!(config.container, deserialized.container);

    Ok(())
}

#[tokio::test]
async fn test_stream_start_request_serialization() -> Result<()> {
    let request = StreamStartRequest {
        config: StreamConfig {
            id: "stream-1".to_string(),
            camera_id: Some("cam-001".to_string()),
            uri: "rtsp://example.com/stream".to_string(),
            codec: Some("h264".to_string()),
            container: Some("fmp4".to_string()),
        },
        lease_ttl_secs: Some(60),
    };

    let serialized = serde_json::to_string(&request)?;
    let deserialized: StreamStartRequest = serde_json::from_str(&serialized)?;

    assert_eq!(request.config.id, deserialized.config.id);
    assert_eq!(request.lease_ttl_secs, deserialized.lease_ttl_secs);

    Ok(())
}

#[tokio::test]
async fn test_stream_state_is_active() {
    // Test is_active method for different states
    assert!(StreamState::Pending.is_active());
    assert!(StreamState::Starting.is_active());
    assert!(StreamState::Running.is_active());
    assert!(!StreamState::Stopping.is_active());
    assert!(!StreamState::Stopped.is_active());
    assert!(!StreamState::Error.is_active());
}

#[tokio::test]
async fn test_stream_config_with_camera() {
    let config = StreamConfig {
        id: "stream-camera-test".to_string(),
        camera_id: Some("camera-123".to_string()),
        uri: "rtsp://camera.local/stream".to_string(),
        codec: Some("h265".to_string()),
        container: Some("ts".to_string()),
    };

    assert!(config.camera_id.is_some());
    assert_eq!(config.camera_id.unwrap(), "camera-123");
    assert_eq!(config.codec, Some("h265".to_string()));
}

#[tokio::test]
async fn test_stream_config_minimal() {
    let config = StreamConfig {
        id: "stream-minimal".to_string(),
        camera_id: None,
        uri: "rtsp://camera.local/stream".to_string(),
        codec: None,
        container: None,
    };

    assert!(config.camera_id.is_none());
    assert!(config.codec.is_none());
    assert!(config.container.is_none());
    assert_eq!(config.uri, "rtsp://camera.local/stream");
}
