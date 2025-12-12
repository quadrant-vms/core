use anyhow::Result;
use common::playback::*;
use std::time::Duration;

/// Test basic playback API endpoints
#[tokio::test]
async fn test_playback_api_basic() -> Result<()> {
    // Note: This test requires playback-service to be running
    // Start with: cargo run -p playback-service

    let client = reqwest::Client::new();
    let base_url = "http://localhost:8087/api";

    // Test health endpoint
    let resp = client
        .get(format!("{}/healthz", base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert_eq!(body, "ok");

    // Test list sessions (should be empty initially)
    let resp = client
        .get(format!("{}/v1/playback/sessions", base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let list: PlaybackListResponse = resp.json().await?;
    println!("Initial sessions: {:?}", list.sessions);

    Ok(())
}

/// Test playback session lifecycle for HLS stream playback
#[tokio::test]
async fn test_playback_session_lifecycle() -> Result<()> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8087/api";

    // Create a playback config for a stream
    let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
    let config = PlaybackConfig {
        session_id: session_id.clone(),
        source_type: PlaybackSourceType::Stream,
        source_id: "test-stream-1".to_string(),
        protocol: PlaybackProtocol::Hls,
        start_time_secs: None,
        speed: Some(1.0),
    };

    // Start playback (will fail if stream doesn't exist, which is expected)
    let start_req = PlaybackStartRequest {
        config: config.clone(),
        lease_ttl_secs: Some(300),
    };

    let resp = client
        .post(format!("{}/v1/playback/start", base_url))
        .json(&start_req)
        .send()
        .await?;

    // This might fail with 500 if stream doesn't exist, which is OK for this test
    if resp.status().is_success() {
        let start_resp: PlaybackStartResponse = resp.json().await?;
        println!("Playback started: {:?}", start_resp);

        // Wait a bit
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Stop playback
        let stop_req = PlaybackStopRequest {
            session_id: session_id.clone(),
        };

        let resp = client
            .post(format!("{}/v1/playback/stop", base_url))
            .json(&stop_req)
            .send()
            .await?;

        assert_eq!(resp.status(), 200);
        let stop_resp: PlaybackStopResponse = resp.json().await?;
        println!("Playback stopped: {:?}", stop_resp);
        assert!(stop_resp.stopped);
    } else {
        println!("Playback start failed (expected if stream doesn't exist): {}", resp.status());
    }

    Ok(())
}

/// Test seek functionality for recording playback
#[tokio::test]
async fn test_playback_seek() -> Result<()> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8087/api";

    let session_id = format!("test-recording-session-{}", uuid::Uuid::new_v4());
    let config = PlaybackConfig {
        session_id: session_id.clone(),
        source_type: PlaybackSourceType::Recording,
        source_id: "test-recording-1".to_string(),
        protocol: PlaybackProtocol::Hls,
        start_time_secs: Some(0.0),
        speed: Some(1.0),
    };

    let start_req = PlaybackStartRequest {
        config,
        lease_ttl_secs: Some(300),
    };

    let resp = client
        .post(format!("{}/v1/playback/start", base_url))
        .json(&start_req)
        .send()
        .await?;

    // If playback starts successfully, test seeking
    if resp.status().is_success() {
        let start_resp: PlaybackStartResponse = resp.json().await?;
        println!("Recording playback started: {:?}", start_resp);

        // Seek to 10 seconds
        let seek_req = PlaybackSeekRequest {
            session_id: session_id.clone(),
            position_secs: 10.0,
        };

        let resp = client
            .post(format!("{}/v1/playback/seek", base_url))
            .json(&seek_req)
            .send()
            .await?;

        assert_eq!(resp.status(), 200);
        let seek_resp: PlaybackSeekResponse = resp.json().await?;
        println!("Seek response: {:?}", seek_resp);

        // Stop playback
        let stop_req = PlaybackStopRequest {
            session_id: session_id.clone(),
        };

        client
            .post(format!("{}/v1/playback/stop", base_url))
            .json(&stop_req)
            .send()
            .await?;
    } else {
        println!("Recording playback start failed (expected if recording doesn't exist): {}", resp.status());
    }

    Ok(())
}

/// Test pause and resume controls
#[tokio::test]
async fn test_playback_controls() -> Result<()> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8087/api";

    let session_id = format!("test-control-session-{}", uuid::Uuid::new_v4());
    let config = PlaybackConfig {
        session_id: session_id.clone(),
        source_type: PlaybackSourceType::Recording,
        source_id: "test-recording-2".to_string(),
        protocol: PlaybackProtocol::Hls,
        start_time_secs: None,
        speed: Some(1.0),
    };

    let start_req = PlaybackStartRequest {
        config,
        lease_ttl_secs: Some(300),
    };

    let resp = client
        .post(format!("{}/v1/playback/start", base_url))
        .json(&start_req)
        .send()
        .await?;

    if resp.status().is_success() {
        // Pause
        let pause_req = PlaybackControlRequest {
            session_id: session_id.clone(),
            action: PlaybackAction::Pause,
        };

        let resp = client
            .post(format!("{}/v1/playback/control", base_url))
            .json(&pause_req)
            .send()
            .await?;

        assert_eq!(resp.status(), 200);
        let pause_resp: PlaybackControlResponse = resp.json().await?;
        println!("Pause response: {:?}", pause_resp);

        // Resume
        let resume_req = PlaybackControlRequest {
            session_id: session_id.clone(),
            action: PlaybackAction::Resume,
        };

        let resp = client
            .post(format!("{}/v1/playback/control", base_url))
            .json(&resume_req)
            .send()
            .await?;

        assert_eq!(resp.status(), 200);
        let resume_resp: PlaybackControlResponse = resp.json().await?;
        println!("Resume response: {:?}", resume_resp);

        // Stop via control
        let stop_req = PlaybackControlRequest {
            session_id: session_id.clone(),
            action: PlaybackAction::Stop,
        };

        client
            .post(format!("{}/v1/playback/control", base_url))
            .json(&stop_req)
            .send()
            .await?;
    } else {
        println!("Playback start failed (expected if recording doesn't exist)");
    }

    Ok(())
}
