//! Integration tests for thumbnail generation

use common::recordings::ThumbnailInfo;
use std::path::PathBuf;

/// This test verifies that the thumbnail API endpoints are properly wired up
/// and handle errors correctly when recordings don't exist.
#[tokio::test]
async fn test_thumbnail_api_not_found() {
    // Start recorder-node server in test mode
    let base_url = start_test_recorder_node().await;

    // Try to get thumbnail for non-existent recording
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "{}/thumbnail?recording_id=nonexistent&timestamp_secs=5.0",
            base_url
        ))
        .send()
        .await
        .expect("failed to send request");

    // Should return 404 Not Found
    assert_eq!(response.status(), 404);
}

/// This test verifies that the thumbnail grid API handles invalid parameters correctly
#[tokio::test]
async fn test_thumbnail_grid_validation() {
    // Start recorder-node server in test mode
    let base_url = start_test_recorder_node().await;

    let client = reqwest::Client::new();

    // Test with non-existent recording
    let response = client
        .get(format!(
            "{}/thumbnail/grid?recording_id=nonexistent&count=5",
            base_url
        ))
        .send()
        .await
        .expect("failed to send request");

    // Should return 404 Not Found
    assert_eq!(response.status(), 404);
}

/// Test the thumbnail generation logic with a real video file
/// This test is skipped by default because it requires ffmpeg and a test video
#[tokio::test]
#[ignore]
async fn test_thumbnail_generation_with_video() {
    use common::thumbnail::{generate_thumbnail, probe_video_duration};
    use std::fs;
    use std::process::Command;

    // Create a test video using ffmpeg
    let test_dir = PathBuf::from("/tmp/quadrant-vms-test");
    fs::create_dir_all(&test_dir).expect("failed to create test directory");

    let test_video = test_dir.join("test-video.mp4");

    // Generate a 10-second test video with testsrc
    let output = Command::new("ffmpeg")
        .args(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=10:size=640x480:rate=30",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-y",
            test_video.to_str().unwrap(),
        ])
        .output()
        .expect("failed to generate test video");

    if !output.status.success() {
        panic!("ffmpeg failed to generate test video");
    }

    // Probe the duration
    let duration = probe_video_duration(&test_video).expect("failed to probe duration");
    assert!(duration >= 9.0 && duration <= 11.0, "duration should be ~10 seconds");

    // Generate a thumbnail at the middle of the video
    let thumbnail_data = generate_thumbnail(&test_video, 5.0, 320, 180, 5)
        .expect("failed to generate thumbnail");

    // Verify it's a valid JPEG
    assert!(!thumbnail_data.is_empty());
    assert_eq!(
        &thumbnail_data[0..3],
        &[0xFF, 0xD8, 0xFF],
        "should be valid JPEG"
    );

    // Cleanup
    fs::remove_dir_all(&test_dir).ok();
}

/// Helper function to start a test recorder-node server
/// Returns the base URL
async fn start_test_recorder_node() -> String {
    use axum::{routing::get, Router};
    use recorder_node::api::{get_thumbnail, get_thumbnail_grid, healthz};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get local addr");

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/thumbnail", get(get_thumbnail))
        .route("/thumbnail/grid", get(get_thumbnail_grid));

    // Spawn server in background
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("server failed to start");
    });

    // Wait a bit for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    format!("http://{}", addr)
}

/// Unit test for thumbnail types serialization
#[test]
fn test_thumbnail_info_serialization() {
    let thumbnail = ThumbnailInfo {
        recording_id: "test-rec-123".to_string(),
        timestamp_secs: 5.5,
        width: 320,
        height: 180,
        image_data: "base64encodeddata==".to_string(),
    };

    let json = serde_json::to_string(&thumbnail).expect("failed to serialize");
    assert!(json.contains("test-rec-123"));
    assert!(json.contains("5.5"));
    assert!(json.contains("320"));
    assert!(json.contains("180"));

    let deserialized: ThumbnailInfo =
        serde_json::from_str(&json).expect("failed to deserialize");
    assert_eq!(deserialized.recording_id, "test-rec-123");
    assert_eq!(deserialized.timestamp_secs, 5.5);
}
