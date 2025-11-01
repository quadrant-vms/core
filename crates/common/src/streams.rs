use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamConfig {
  pub id: String,
  pub camera_id: Option<String>,
  pub uri: String,
  pub codec: Option<String>,
  pub container: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
  Pending,
  Starting,
  Running,
  Stopping,
  Stopped,
  Error,
}

impl StreamState {
  pub fn is_active(&self) -> bool {
    matches!(
      self,
      StreamState::Pending | StreamState::Starting | StreamState::Running
    )
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamInfo {
  pub config: StreamConfig,
  pub state: StreamState,
  pub lease_id: Option<String>,
  pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStartRequest {
  pub config: StreamConfig,
  #[serde(default)]
  pub lease_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStartResponse {
  pub accepted: bool,
  pub lease_id: Option<String>,
  pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStopRequest {
  pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStopResponse {
  pub stopped: bool,
  pub message: Option<String>,
}
