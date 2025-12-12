use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordingConfig {
  pub id: String,
  pub source_stream_id: Option<String>,
  pub source_uri: Option<String>,
  pub retention_hours: Option<u32>,
  pub format: Option<RecordingFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordingFormat {
  Mp4,
  Hls,
  Mkv,
}

impl Default for RecordingFormat {
  fn default() -> Self {
    RecordingFormat::Mp4
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
  Pending,
  Starting,
  Recording,
  Paused,
  Stopping,
  Stopped,
  Error,
}

impl RecordingState {
  pub fn is_active(&self) -> bool {
    matches!(
      self,
      RecordingState::Pending
        | RecordingState::Starting
        | RecordingState::Recording
        | RecordingState::Paused
    )
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordingMetadata {
  pub duration_secs: Option<u64>,
  pub file_size_bytes: Option<u64>,
  pub video_codec: Option<String>,
  pub audio_codec: Option<String>,
  pub resolution: Option<(u32, u32)>,
  pub bitrate_kbps: Option<u32>,
  pub fps: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordingInfo {
  pub config: RecordingConfig,
  pub state: RecordingState,
  pub lease_id: Option<String>,
  pub storage_path: Option<String>,
  pub last_error: Option<String>,
  pub started_at: Option<u64>,
  pub stopped_at: Option<u64>,
  #[serde(default)]
  pub node_id: Option<String>,
  #[serde(default)]
  pub metadata: Option<RecordingMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingAiConfig {
  /// AI service base URL (e.g., "http://localhost:8084")
  pub ai_service_url: String,
  /// AI task ID to submit frames to
  pub ai_task_id: String,
  /// Frame capture interval in seconds
  #[serde(default = "default_capture_interval")]
  pub capture_interval_secs: u64,
  /// Frame width (0 = auto)
  #[serde(default = "default_frame_width")]
  pub frame_width: u32,
  /// Frame height (0 = auto)
  #[serde(default)]
  pub frame_height: u32,
  /// JPEG quality (2-31, lower is better)
  #[serde(default = "default_jpeg_quality")]
  pub jpeg_quality: u32,
}

fn default_capture_interval() -> u64 {
  2
}

fn default_frame_width() -> u32 {
  640
}

fn default_jpeg_quality() -> u32 {
  5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStartRequest {
  pub config: RecordingConfig,
  #[serde(default)]
  pub lease_ttl_secs: Option<u64>,
  /// Optional AI processing configuration
  #[serde(default)]
  pub ai_config: Option<RecordingAiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStartResponse {
  pub accepted: bool,
  pub lease_id: Option<String>,
  pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStopRequest {
  pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStopResponse {
  pub stopped: bool,
  pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingListResponse {
  pub recordings: Vec<RecordingInfo>,
}

// Thumbnail-related types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailRequest {
  pub recording_id: String,
  pub timestamp_secs: Option<f64>,
  pub width: Option<u32>,
  pub height: Option<u32>,
  pub quality: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailGridRequest {
  pub recording_id: String,
  pub count: u32,
  pub width: Option<u32>,
  pub height: Option<u32>,
  pub quality: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailInfo {
  pub recording_id: String,
  pub timestamp_secs: f64,
  pub width: u32,
  pub height: u32,
  /// Base64-encoded JPEG image data
  pub image_data: String,
}
