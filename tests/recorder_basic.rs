use common::recordings::*;

#[tokio::test]
async fn test_recording_types_serialization() {
  let config = RecordingConfig {
    id: "rec-1".to_string(),
    source_stream_id: Some("stream-1".to_string()),
    source_uri: None,
    retention_hours: Some(48),
    format: Some(RecordingFormat::Mp4),
  };

  let json = serde_json::to_string(&config).unwrap();
  let deserialized: RecordingConfig = serde_json::from_str(&json).unwrap();

  assert_eq!(config, deserialized);
  assert_eq!(config.format.unwrap(), RecordingFormat::Mp4);
}

#[tokio::test]
async fn test_recording_state_is_active() {
  assert!(RecordingState::Pending.is_active());
  assert!(RecordingState::Starting.is_active());
  assert!(RecordingState::Recording.is_active());
  assert!(RecordingState::Paused.is_active());
  assert!(!RecordingState::Stopping.is_active());
  assert!(!RecordingState::Stopped.is_active());
  assert!(!RecordingState::Error.is_active());
}

#[tokio::test]
async fn test_recording_start_request() {
  let config = RecordingConfig {
    id: "test-rec".to_string(),
    source_stream_id: None,
    source_uri: Some("rtsp://camera.local/stream".to_string()),
    retention_hours: Some(24),
    format: None,
  };

  let request = RecordingStartRequest {
    config: config.clone(),
    lease_ttl_secs: Some(120),
  };

  assert_eq!(request.config.id, "test-rec");
  assert_eq!(request.lease_ttl_secs, Some(120));
}
