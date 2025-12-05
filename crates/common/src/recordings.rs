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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
  pub duration_secs: Option<u64>,
  pub file_size_bytes: Option<u64>,
  pub video_codec: Option<String>,
  pub audio_codec: Option<String>,
  pub resolution: Option<(u32, u32)>,
  pub bitrate_kbps: Option<u32>,
  pub fps: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordingInfo {
  pub config: RecordingConfig,
  pub state: RecordingState,
  pub lease_id: Option<String>,
  pub storage_path: Option<String>,
  pub last_error: Option<String>,
  pub started_at: Option<u64>,
  pub stopped_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStartRequest {
  pub config: RecordingConfig,
  #[serde(default)]
  pub lease_ttl_secs: Option<u64>,
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
