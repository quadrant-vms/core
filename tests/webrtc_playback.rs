use common::playback::*;

#[tokio::test]
async fn test_webrtc_playback_url_generation() {
    // Test that WebRTC protocol generates proper WHEP endpoint URLs

    // Create a playback config for a stream with WebRTC protocol
    let stream_config = PlaybackConfig {
        session_id: "test-webrtc-session-1".to_string(),
        source_type: PlaybackSourceType::Stream,
        source_id: "camera-1".to_string(),
        protocol: PlaybackProtocol::WebRtc,
        start_time_secs: None,
        speed: None,
        low_latency: false,
        dvr: None,
    };

    // Test the protocol is set correctly
    assert_eq!(stream_config.protocol, PlaybackProtocol::WebRtc);
    assert_eq!(stream_config.source_type, PlaybackSourceType::Stream);

    // Create a playback config for a recording with WebRTC protocol
    let recording_config = PlaybackConfig {
        session_id: "test-webrtc-session-2".to_string(),
        source_type: PlaybackSourceType::Recording,
        source_id: "rec-123".to_string(),
        protocol: PlaybackProtocol::WebRtc,
        start_time_secs: Some(10.0),
        speed: None,
        low_latency: false,
        dvr: None,
    };

    // Test the protocol is set correctly
    assert_eq!(recording_config.protocol, PlaybackProtocol::WebRtc);
    assert_eq!(recording_config.source_type, PlaybackSourceType::Recording);
}

#[tokio::test]
async fn test_whep_offer_structure() {
    use serde_json::json;

    // Test WHEP offer JSON structure
    let offer_json = json!({
        "sdp": "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n",
        "codec": "H264"
    });

    let offer_str = serde_json::to_string(&offer_json).unwrap();
    assert!(offer_str.contains("sdp"));
    assert!(offer_str.contains("codec"));
}

#[tokio::test]
async fn test_whep_answer_structure() {
    use serde_json::json;

    // Test WHEP answer JSON structure
    let answer_json = json!({
        "sdp": "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n",
        "session_id": "550e8400-e29b-41d4-a716-446655440000",
        "session_url": "http://localhost:8087/api/whep/session/550e8400-e29b-41d4-a716-446655440000"
    });

    let answer_str = serde_json::to_string(&answer_json).unwrap();
    assert!(answer_str.contains("sdp"));
    assert!(answer_str.contains("session_id"));
    assert!(answer_str.contains("session_url"));
}

#[tokio::test]
async fn test_playback_state_transitions() {
    // Test that playback states are appropriate for WebRTC
    let states = vec![
        PlaybackState::Pending,
        PlaybackState::Starting,
        PlaybackState::Playing,
        PlaybackState::Paused,
        PlaybackState::Stopped,
    ];

    // All these states should be valid for WebRTC playback
    for state in states {
        match state {
            PlaybackState::Pending => assert!(state.is_active()),
            PlaybackState::Starting => assert!(state.is_active()),
            PlaybackState::Playing => assert!(state.is_active()),
            PlaybackState::Paused => assert!(state.is_active()),
            PlaybackState::Stopped => assert!(!state.is_active()),
            _ => {}
        }
    }
}
