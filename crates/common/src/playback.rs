use serde::{Deserialize, Serialize};

/// Playback session configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackConfig {
    /// Unique session ID
    pub session_id: String,
    /// Source type: "stream" or "recording"
    pub source_type: PlaybackSourceType,
    /// Source identifier (stream_id or recording_id)
    pub source_id: String,
    /// Playback protocol: "hls", "rtsp", "webrtc"
    pub protocol: PlaybackProtocol,
    /// Optional start time for recordings (seconds from start)
    pub start_time_secs: Option<f64>,
    /// Optional playback speed (1.0 = normal)
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackSourceType {
    Stream,
    Recording,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackProtocol {
    Hls,
    Rtsp,
    WebRtc,
}

/// Playback session state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Pending,
    Starting,
    Playing,
    Paused,
    Seeking,
    Stopping,
    Stopped,
    Error,
}

impl PlaybackState {
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            PlaybackState::Pending
                | PlaybackState::Starting
                | PlaybackState::Playing
                | PlaybackState::Paused
                | PlaybackState::Seeking
        )
    }
}

/// Playback session information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackInfo {
    pub config: PlaybackConfig,
    pub state: PlaybackState,
    pub lease_id: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    /// HLS playlist URL or RTSP stream URL
    pub playback_url: Option<String>,
    /// Current playback position (seconds)
    pub current_position_secs: Option<f64>,
    /// Total duration (for recordings)
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub stopped_at: Option<u64>,
}

/// Request to start a playback session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStartRequest {
    pub config: PlaybackConfig,
    #[serde(default)]
    pub lease_ttl_secs: Option<u64>,
}

/// Response for playback start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStartResponse {
    pub accepted: bool,
    pub session_id: String,
    pub lease_id: Option<String>,
    pub playback_url: Option<String>,
    pub message: Option<String>,
}

/// Request to stop a playback session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStopRequest {
    pub session_id: String,
}

/// Response for playback stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStopResponse {
    pub stopped: bool,
    pub message: Option<String>,
}

/// Request to seek in a playback session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSeekRequest {
    pub session_id: String,
    /// Target position in seconds
    pub position_secs: f64,
}

/// Response for seek operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSeekResponse {
    pub success: bool,
    pub current_position_secs: Option<f64>,
    pub message: Option<String>,
}

/// Request to pause/resume playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackControlRequest {
    pub session_id: String,
    pub action: PlaybackAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackAction {
    Pause,
    Resume,
    Stop,
}

/// Response for control operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackControlResponse {
    pub success: bool,
    pub message: Option<String>,
}

/// List playback sessions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackListResponse {
    pub sessions: Vec<PlaybackInfo>,
}
