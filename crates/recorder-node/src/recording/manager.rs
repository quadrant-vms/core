use anyhow::{anyhow, Result};
use common::{
  leases::{LeaseAcquireRequest, LeaseKind, LeaseReleaseRequest, LeaseRenewRequest},
  recordings::*,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::frame_capturer::{self, FrameCaptureConfig};
use super::pipeline::RecordingPipeline;
use crate::coordinator::CoordinatorClient;

lazy_static! {
  pub static ref RECORDING_MANAGER: RecordingManager = RecordingManager::new();
}

pub struct RecordingManager {
  recordings: Arc<RwLock<HashMap<String, RecordingInfo>>>,
  pipelines: Arc<RwLock<HashMap<String, RecordingPipeline>>>,
  renewals: Arc<RwLock<HashMap<String, CancellationToken>>>,
  frame_capturers: Arc<RwLock<HashMap<String, CancellationToken>>>,
  coordinator: Arc<RwLock<Option<Arc<dyn CoordinatorClient>>>>,
  node_id: Arc<RwLock<Option<String>>>,
}

impl RecordingManager {
  pub fn new() -> Self {
    Self {
      recordings: Arc::new(RwLock::new(HashMap::new())),
      pipelines: Arc::new(RwLock::new(HashMap::new())),
      renewals: Arc::new(RwLock::new(HashMap::new())),
      frame_capturers: Arc::new(RwLock::new(HashMap::new())),
      coordinator: Arc::new(RwLock::new(None)),
      node_id: Arc::new(RwLock::new(None)),
    }
  }

  /// Clear all recordings and state (for testing only)
  pub async fn clear(&self) {
    self.recordings.write().await.clear();
    self.pipelines.write().await.clear();
    let renewals = self.renewals.write().await.drain().collect::<Vec<_>>();
    for (_, token) in renewals {
      token.cancel();
    }
    let capturers = self.frame_capturers.write().await.drain().collect::<Vec<_>>();
    for (_, token) in capturers {
      token.cancel();
    }
    *self.coordinator.write().await = None;
    *self.node_id.write().await = None;
  }

  pub async fn set_coordinator(&self, coordinator: Arc<dyn CoordinatorClient>, node_id: String) {
    *self.coordinator.write().await = Some(coordinator);
    *self.node_id.write().await = Some(node_id);
  }

  pub async fn start(&self, req: RecordingStartRequest) -> Result<RecordingStartResponse> {
    let id = req.config.id.clone();

    let recordings = self.recordings.read().await;
    if recordings.contains_key(&id) {
      return Ok(RecordingStartResponse {
        accepted: false,
        lease_id: None,
        message: Some(format!("recording {} already exists", id)),
      });
    }
    drop(recordings);

    // Attempt to acquire lease if coordinator is configured
    let lease_id = if let Some(coordinator) = self.coordinator.read().await.clone() {
      let node_id = self
        .node_id
        .read()
        .await
        .clone()
        .unwrap_or_else(|| "recorder-node".to_string());
      let ttl_secs = req.lease_ttl_secs.unwrap_or(60).max(5);

      let lease_req = LeaseAcquireRequest {
        resource_id: id.clone(),
        holder_id: node_id,
        kind: LeaseKind::Recorder,
        ttl_secs,
      };

      info!(id = %id, ttl = ttl_secs, "acquiring recorder lease");
      let lease_resp = coordinator.acquire(&lease_req).await?;

      if !lease_resp.granted {
        return Ok(RecordingStartResponse {
          accepted: false,
          lease_id: None,
          message: Some(format!(
            "lease not granted for recording {}",
            id
          )),
        });
      }

      let record = lease_resp
        .record
        .ok_or_else(|| anyhow!("lease granted but no record returned"))?;

      // Start renewal loop
      self.start_lease_renewal(id.clone(), record.lease_id.clone(), ttl_secs).await;

      Some(record.lease_id)
    } else {
      info!(id = %id, "no coordinator configured, starting without lease");
      None
    };

    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();

    let node_id_value = self.node_id.read().await.clone();

    let info = RecordingInfo {
      config: req.config.clone(),
      state: RecordingState::Starting,
      node_id: node_id_value,
      lease_id: lease_id.clone(),
      storage_path: None,
      last_error: None,
      started_at: Some(now),
      stopped_at: None,
      metadata: None,
    };

    let mut recordings = self.recordings.write().await;
    recordings.insert(id.clone(), info);
    drop(recordings);

    let pipeline = RecordingPipeline::new(req.config.clone());
    let mut pipelines = self.pipelines.write().await;
    pipelines.insert(id.clone(), pipeline);
    drop(pipelines);

    // Start frame capture if AI config is provided
    if let Some(ai_cfg) = &req.ai_config {
      let source_uri = req
        .config
        .source_uri
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
      let cancel_token = CancellationToken::new();

      let frame_cfg = FrameCaptureConfig {
        ai_service_url: ai_cfg.ai_service_url.clone(),
        ai_task_id: ai_cfg.ai_task_id.clone(),
        capture_interval_secs: ai_cfg.capture_interval_secs,
        frame_width: ai_cfg.frame_width,
        frame_height: ai_cfg.frame_height,
        jpeg_quality: ai_cfg.jpeg_quality,
      };

      info!(
        id = %id,
        ai_task_id = %frame_cfg.ai_task_id,
        "starting frame capture for recording"
      );

      frame_capturer::start_frame_capture(
        id.clone(),
        source_uri,
        frame_cfg,
        cancel_token.clone(),
      );

      let mut capturers = self.frame_capturers.write().await;
      capturers.insert(id.clone(), cancel_token);
    }

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
        // Store output path
        let output_path = pipeline.output_path().to_string_lossy().to_string();
        let mut recordings = recordings_clone.write().await;
        if let Some(info) = recordings.get_mut(&id) {
          info.storage_path = Some(output_path);
        }
        drop(recordings);

        // Run pipeline
        if let Err(e) = pipeline.run().await {
          warn!(id = %id, error = %e, "recording pipeline failed");
          let mut recordings = recordings_clone.write().await;
          if let Some(info) = recordings.get_mut(&id) {
            info.state = RecordingState::Error;
            info.last_error = Some(e.to_string());
          }
        } else {
          // Extract metadata after successful recording
          info!(id = %id, "recording completed, extracting metadata");
          match pipeline.extract_metadata().await {
            Ok(_metadata) => {
              info!(id = %id, "metadata extraction successful");
              // Metadata is logged but not stored in RecordingInfo yet
              // TODO: Add metadata field to RecordingInfo
            }
            Err(e) => {
              warn!(id = %id, error = %e, "metadata extraction failed");
            }
          }
        }
      }
    });

    Ok(RecordingStartResponse {
      accepted: true,
      lease_id,
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

    let lease_id = info.lease_id.clone();
    drop(recordings);

    // Cancel renewal loop
    self.cancel_lease_renewal(id).await;

    // Cancel frame capture if running
    if let Some(token) = self.frame_capturers.write().await.remove(id) {
      info!(id = %id, "stopping frame capture");
      token.cancel();
    }

    // Stop the pipeline
    let mut pipelines = self.pipelines.write().await;
    if let Some(mut pipeline) = pipelines.remove(id) {
      pipeline.stop().await?;
    }
    drop(pipelines);

    // Release the lease if we have one
    if let Some(lease_id) = lease_id {
      if let Some(coordinator) = self.coordinator.read().await.clone() {
        let release_req = LeaseReleaseRequest {
          lease_id: lease_id.clone(),
        };
        info!(id = %id, lease_id = %lease_id, "releasing recorder lease");
        if let Err(e) = coordinator.release(&release_req).await {
          warn!(id = %id, error = %e, "failed to release lease");
        }
      }
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

  async fn start_lease_renewal(&self, recording_id: String, lease_id: String, ttl_secs: u64) {
    let token = CancellationToken::new();
    {
      let mut renewals = self.renewals.write().await;
      if let Some(existing) = renewals.insert(recording_id.clone(), token.clone()) {
        existing.cancel();
      }
    }

    let recordings = Arc::clone(&self.recordings);
    let coordinator = self.coordinator.clone();
    let interval_secs = ttl_secs / 2;
    let renew_interval = Duration::from_secs(std::cmp::max(interval_secs, 5));

    tokio::spawn(async move {
      loop {
        tokio::select! {
          _ = token.cancelled() => {
            break;
          }
          _ = tokio::time::sleep(renew_interval) => {
            let req = LeaseRenewRequest {
              lease_id: lease_id.clone(),
              ttl_secs,
            };

            let coordinator_guard = coordinator.read().await;
            if let Some(coordinator) = coordinator_guard.as_ref() {
              match coordinator.renew(&req).await {
                Ok(_) => {
                  let mut recordings = recordings.write().await;
                  if let Some(entry) = recordings.get_mut(&recording_id) {
                    if entry.state == RecordingState::Error {
                      entry.state = RecordingState::Recording;
                    }
                    entry.last_error = None;
                  }
                }
                Err(err) => {
                  warn!(id = %recording_id, error = %err, "lease renewal failed");
                  let mut recordings = recordings.write().await;
                  if let Some(entry) = recordings.get_mut(&recording_id) {
                    entry.state = RecordingState::Error;
                    entry.last_error = Some(err.to_string());
                  }
                  break;
                }
              }
            } else {
              break;
            }
          }
        }
      }
    });
  }

  async fn cancel_lease_renewal(&self, recording_id: &str) {
    if let Some(token) = self.renewals.write().await.remove(recording_id) {
      token.cancel();
    }
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
      ai_config: None,
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
