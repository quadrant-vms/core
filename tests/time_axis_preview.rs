//! Integration tests for time-axis preview functionality

use common::playback::{
    PlaybackSourceType, TimeAxisPreviewRequest, TimeAxisPreviewResponse,
};
use playback_service::preview::{generate_time_axis_preview, PreviewConfig};
use std::path::PathBuf;

#[test]
fn test_preview_config_defaults() {
    let config = PreviewConfig::default();
    assert_eq!(config.default_count, 10);
    assert_eq!(config.default_width, 320);
    assert_eq!(config.default_height, 180);
    assert_eq!(config.default_quality, 5);
    assert_eq!(config.max_count, 100);
}

#[test]
fn test_time_axis_preview_nonexistent_recording() {
    let request = TimeAxisPreviewRequest {
        source_id: "nonexistent-recording".to_string(),
        source_type: PlaybackSourceType::Recording,
        count: 10,
        width: Some(320),
        height: Some(180),
        quality: Some(5),
    };

    let storage_root = PathBuf::from("/tmp/nonexistent");
    let config = PreviewConfig::default();

    let result = generate_time_axis_preview(request, &storage_root, &config);
    assert!(result.is_err());
}

#[test]
fn test_time_axis_preview_zero_count() {
    let request = TimeAxisPreviewRequest {
        source_id: "test-recording".to_string(),
        source_type: PlaybackSourceType::Recording,
        count: 0,
        width: Some(320),
        height: Some(180),
        quality: Some(5),
    };

    let storage_root = PathBuf::from("./data/recordings");
    let config = PreviewConfig::default();

    let result = generate_time_axis_preview(request, &storage_root, &config);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must be greater than 0"));
}

#[test]
fn test_time_axis_preview_max_count_limiting() {
    let config = PreviewConfig {
        max_count: 20,
        ..Default::default()
    };

    // Request more than max_count
    let request = TimeAxisPreviewRequest {
        source_id: "test-recording".to_string(),
        source_type: PlaybackSourceType::Recording,
        count: 100, // More than max_count of 20
        width: Some(320),
        height: Some(180),
        quality: Some(5),
    };

    let storage_root = PathBuf::from("/tmp/nonexistent");

    // This will fail because recording doesn't exist, but we're testing that
    // the count limiting logic doesn't panic
    let result = generate_time_axis_preview(request, &storage_root, &config);
    assert!(result.is_err());
}

#[test]
fn test_time_axis_preview_stream_not_supported() {
    let request = TimeAxisPreviewRequest {
        source_id: "test-stream".to_string(),
        source_type: PlaybackSourceType::Stream,
        count: 10,
        width: Some(320),
        height: Some(180),
        quality: Some(5),
    };

    let storage_root = PathBuf::from("./data/recordings");
    let config = PreviewConfig::default();

    let result = generate_time_axis_preview(request, &storage_root, &config);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("only supported for recordings"));
}

#[test]
fn test_time_axis_preview_request_serialization() {
    let request = TimeAxisPreviewRequest {
        source_id: "test-recording".to_string(),
        source_type: PlaybackSourceType::Recording,
        count: 10,
        width: Some(320),
        height: Some(180),
        quality: Some(5),
    };

    // Verify it can be serialized to JSON
    let json = serde_json::to_string(&request).expect("failed to serialize");
    assert!(json.contains("test-recording"));
    assert!(json.contains("recording"));

    // Verify it can be deserialized from JSON
    let deserialized: TimeAxisPreviewRequest =
        serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(deserialized.source_id, "test-recording");
    assert_eq!(deserialized.source_type, PlaybackSourceType::Recording);
    assert_eq!(deserialized.count, 10);
}

#[test]
fn test_time_axis_preview_response_serialization() {
    use common::playback::TimeAxisThumbnail;

    let response = TimeAxisPreviewResponse {
        source_id: "test-recording".to_string(),
        source_type: PlaybackSourceType::Recording,
        duration_secs: 120.0,
        thumbnails: vec![
            TimeAxisThumbnail {
                timestamp_secs: 12.0,
                position_percent: 0.1,
                width: 320,
                height: 180,
                image_data: "base64encodeddata".to_string(),
            },
            TimeAxisThumbnail {
                timestamp_secs: 60.0,
                position_percent: 0.5,
                width: 320,
                height: 180,
                image_data: "base64encodeddata2".to_string(),
            },
        ],
    };

    // Verify it can be serialized to JSON
    let json = serde_json::to_string(&response).expect("failed to serialize");
    assert!(json.contains("test-recording"));
    assert!(json.contains("120"));

    // Verify it can be deserialized from JSON
    let deserialized: TimeAxisPreviewResponse =
        serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(deserialized.source_id, "test-recording");
    assert_eq!(deserialized.duration_secs, 120.0);
    assert_eq!(deserialized.thumbnails.len(), 2);
    assert_eq!(deserialized.thumbnails[0].timestamp_secs, 12.0);
    assert_eq!(deserialized.thumbnails[0].position_percent, 0.1);
    assert_eq!(deserialized.thumbnails[1].timestamp_secs, 60.0);
    assert_eq!(deserialized.thumbnails[1].position_percent, 0.5);
}
