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
    /// Enable Low-Latency HLS mode (only for HLS protocol)
    #[serde(default)]
    pub low_latency: bool,
    /// DVR configuration for time-shift playback (only for streams)
    #[serde(default)]
    pub dvr: Option<DvrConfig>,
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
    /// DVR window information (only for DVR-enabled sessions)
    #[serde(default)]
    pub dvr_window: Option<DvrWindowInfo>,
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

/// DVR configuration for time-shift playback
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DvrConfig {
    /// Enable DVR mode for this session
    pub enabled: bool,
    /// Maximum rewind limit in seconds (None = unlimited based on buffer)
    pub rewind_limit_secs: Option<f64>,
    /// Size of the DVR buffer window in seconds
    pub buffer_window_secs: f64,
}

impl Default for DvrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rewind_limit_secs: Some(3600.0), // 1 hour default
            buffer_window_secs: 300.0,        // 5 minutes default buffer
        }
    }
}

/// DVR window information - available time range for time-shift playback
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DvrWindowInfo {
    /// Stream identifier
    pub stream_id: String,
    /// Earliest available timestamp (Unix timestamp in seconds)
    pub earliest_available: u64,
    /// Latest available timestamp (Unix timestamp in seconds) - the live edge
    pub latest_available: u64,
    /// Total buffer duration in seconds
    pub buffer_seconds: f64,
    /// Current playback position timestamp (Unix timestamp in seconds)
    pub current_position: Option<u64>,
    /// Offset from live edge in seconds (negative = in the past)
    pub live_offset_secs: Option<f64>,
}

/// Request to get DVR window information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvrWindowRequest {
    pub session_id: String,
}

/// Request to seek to a specific time in DVR buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvrSeekRequest {
    pub session_id: String,
    /// Absolute Unix timestamp in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_secs: Option<u64>,
    /// OR relative offset from live edge in seconds (negative = past, 0 = live)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_offset_secs: Option<f64>,
}

/// Response for DVR seek operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvrSeekResponse {
    pub success: bool,
    /// Actual timestamp seeked to
    pub timestamp_secs: Option<u64>,
    /// Current offset from live edge
    pub live_offset_secs: Option<f64>,
    pub message: Option<String>,
}

/// Request to jump to live edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvrJumpToLiveRequest {
    pub session_id: String,
}

/// HLS segment metadata for DVR timeline tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DvrSegment {
    /// Segment filename
    pub filename: String,
    /// Media sequence number
    pub sequence: u64,
    /// Segment duration in seconds
    pub duration: f64,
    /// Start timestamp (Unix timestamp in seconds)
    pub start_timestamp: u64,
    /// End timestamp (Unix timestamp in seconds)
    pub end_timestamp: u64,
    /// Full path to segment file
    pub file_path: String,
}

// === Time-Axis Preview ===

/// Request for time-axis preview thumbnails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAxisPreviewRequest {
    /// Recording ID or stream ID
    pub source_id: String,
    /// Source type: "recording" or "stream"
    pub source_type: PlaybackSourceType,
    /// Number of thumbnails to generate (evenly spaced)
    pub count: u32,
    /// Thumbnail width in pixels
    pub width: Option<u32>,
    /// Thumbnail height in pixels
    pub height: Option<u32>,
    /// JPEG quality (1-10, lower = smaller file size)
    pub quality: Option<u32>,
}

/// Individual thumbnail in the time-axis preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAxisThumbnail {
    /// Timestamp in seconds from start
    pub timestamp_secs: f64,
    /// Position as percentage of total duration (0.0 - 1.0)
    pub position_percent: f64,
    /// Thumbnail width in pixels
    pub width: u32,
    /// Thumbnail height in pixels
    pub height: u32,
    /// Base64-encoded JPEG image data
    pub image_data: String,
}

/// Response containing time-axis preview thumbnails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAxisPreviewResponse {
    /// Source identifier
    pub source_id: String,
    /// Source type
    pub source_type: PlaybackSourceType,
    /// Total duration in seconds
    pub duration_secs: f64,
    /// List of thumbnails evenly spaced along the timeline
    pub thumbnails: Vec<TimeAxisThumbnail>,
}
