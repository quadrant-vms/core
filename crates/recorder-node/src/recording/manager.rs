use anyhow::{anyhow, Result};
use common::recordings::*;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::pipeline::RecordingPipeline;

lazy_static! {
  pub static ref RECORDING_MANAGER: RecordingManager = RecordingManager::new();
}

pub struct RecordingManager {
  recordings: Arc<RwLock<HashMap<String, RecordingInfo>>>,
  pipelines: Arc<RwLock<HashMap<String, RecordingPipeline>>>,
}

impl RecordingManager {
  pub fn new() -> Self {
    Self {
      recordings: Arc::new(RwLock::new(HashMap::new())),
      pipelines: Arc::new(RwLock::new(HashMap::new())),
    }
  }

  pub async fn start(&self, req: RecordingStartRequest) -> Result<RecordingStartResponse> {
    let id = req.config.id.clone();

    let mut recordings = self.recordings.write().await;
    if recordings.contains_key(&id) {
      return Ok(RecordingStartResponse {
        accepted: false,
        lease_id: None,
        message: Some(format!("recording {} already exists", id)),
      });
    }

    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();

    let info = RecordingInfo {
      config: req.config.clone(),
      state: RecordingState::Starting,
      lease_id: None,
      storage_path: None,
      last_error: None,
      started_at: Some(now),
      stopped_at: None,
    };

    recordings.insert(id.clone(), info);
    drop(recordings);

    let pipeline = RecordingPipeline::new(req.config.clone());
    let mut pipelines = self.pipelines.write().await;
    pipelines.insert(id.clone(), pipeline);
    drop(pipelines);

    let recordings_clone = Arc::clone(&self.recordings);
    let pipelines_clone = Arc::clone(&self.pipelines);

    tokio::spawn(async move {
      let mut recordings = recordings_clone.write().await;
      if let Some(info) = recordings.get_mut(&id) {
        info.state = RecordingState::Recording;
      }
      drop(recordings);

      info!(id = %id, "recording pipeline started");

      let mut pipelines = pipelines_clone.write().await;
      if let Some(pipeline) = pipelines.get_mut(&id) {
        if let Err(e) = pipeline.run().await {
          warn!(id = %id, error = %e, "recording pipeline failed");
          let mut recordings = recordings_clone.write().await;
          if let Some(info) = recordings.get_mut(&id) {
            info.state = RecordingState::Error;
            info.last_error = Some(e.to_string());
          }
        }
      }
    });

    Ok(RecordingStartResponse {
      accepted: true,
      lease_id: None,
      message: Some("recording started".to_string()),
    })
  }

  pub async fn stop(&self, id: &str) -> Result<bool> {
    let mut recordings = self.recordings.write().await;
    let info = recordings
      .get_mut(id)
      .ok_or_else(|| anyhow!("recording not found"))?;

    if !info.state.is_active() {
      return Ok(false);
    }

    info.state = RecordingState::Stopping;
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
    info.stopped_at = Some(now);

    drop(recordings);

    let mut pipelines = self.pipelines.write().await;
    if let Some(mut pipeline) = pipelines.remove(id) {
      pipeline.stop().await?;
    }

    let mut recordings = self.recordings.write().await;
    if let Some(info) = recordings.get_mut(id) {
      info.state = RecordingState::Stopped;
    }

    info!(id = %id, "recording stopped");
    Ok(true)
  }

  pub async fn list(&self) -> Vec<RecordingInfo> {
    let recordings = self.recordings.read().await;
    recordings.values().cloned().collect()
  }

  pub async fn get(&self, id: &str) -> Option<RecordingInfo> {
    let recordings = self.recordings.read().await;
    recordings.get(id).cloned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_recording_lifecycle() {
    let manager = RecordingManager::new();

    let config = RecordingConfig {
      id: "test-rec-1".to_string(),
      source_stream_id: Some("stream-1".to_string()),
      source_uri: Some("rtsp://example.com/stream".to_string()),
      retention_hours: Some(24),
      format: Some(RecordingFormat::Mp4),
    };

    let req = RecordingStartRequest {
      config,
      lease_ttl_secs: Some(60),
    };

    let response = manager.start(req).await.unwrap();
    assert!(response.accepted);

    let list = manager.list().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].config.id, "test-rec-1");

    let stopped = manager.stop("test-rec-1").await.unwrap();
    assert!(stopped);

    let info = manager.get("test-rec-1").await.unwrap();
    assert_eq!(info.state, RecordingState::Stopped);
  }
}
