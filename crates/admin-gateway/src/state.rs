use crate::{config::GatewayConfig, coordinator::CoordinatorClient, worker::{RecorderClient, WorkerClient}};
use common::{
  leases::LeaseRenewRequest,
  recordings::RecordingInfo,
  state_store::StateStore,
  streams::{StreamInfo, StreamState},
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::warn;

#[derive(Clone)]
pub struct AppState {
  inner: Arc<AppStateInner>,
}

struct AppStateInner {
  config: GatewayConfig,
  coordinator: Arc<dyn CoordinatorClient>,
  worker: Arc<dyn WorkerClient>,
  recorder: Arc<dyn RecorderClient>,
  state_store: Option<Arc<dyn StateStore>>,
  streams: RwLock<HashMap<String, StreamInfo>>,
  recordings: RwLock<HashMap<String, RecordingInfo>>,
  renewals: RwLock<HashMap<String, CancellationToken>>,
}

impl AppState {
  pub fn new(
    config: GatewayConfig,
    coordinator: Arc<dyn CoordinatorClient>,
    worker: Arc<dyn WorkerClient>,
    recorder: Arc<dyn RecorderClient>,
  ) -> Self {
    let inner = AppStateInner {
      config,
      coordinator,
      worker,
      recorder,
      state_store: None,
      streams: RwLock::new(HashMap::new()),
      recordings: RwLock::new(HashMap::new()),
      renewals: RwLock::new(HashMap::new()),
    };
    Self {
      inner: Arc::new(inner),
    }
  }

  pub fn with_state_store(
    config: GatewayConfig,
    coordinator: Arc<dyn CoordinatorClient>,
    worker: Arc<dyn WorkerClient>,
    recorder: Arc<dyn RecorderClient>,
    state_store: Arc<dyn StateStore>,
  ) -> Self {
    let inner = AppStateInner {
      config,
      coordinator,
      worker,
      recorder,
      state_store: Some(state_store),
      streams: RwLock::new(HashMap::new()),
      recordings: RwLock::new(HashMap::new()),
      renewals: RwLock::new(HashMap::new()),
    };
    Self {
      inner: Arc::new(inner),
    }
  }

  pub fn node_id(&self) -> &str {
    &self.inner.config.node_id
  }

  pub fn coordinator(&self) -> Arc<dyn CoordinatorClient> {
    self.inner.coordinator.clone()
  }

  pub fn worker(&self) -> Arc<dyn WorkerClient> {
    self.inner.worker.clone()
  }

  pub fn recorder(&self) -> Arc<dyn RecorderClient> {
    self.inner.recorder.clone()
  }

  pub fn streams(&self) -> &RwLock<HashMap<String, StreamInfo>> {
    &self.inner.streams
  }

  pub fn recordings(&self) -> &RwLock<HashMap<String, RecordingInfo>> {
    &self.inner.recordings
  }

  pub fn state_store(&self) -> Option<Arc<dyn StateStore>> {
    self.inner.state_store.clone()
  }

  /// Persist stream state to StateStore if configured
  pub async fn persist_stream(&self, info: &StreamInfo) {
    if let Some(store) = &self.inner.state_store {
      if let Err(e) = store.save_stream(info).await {
        warn!(stream_id = %info.config.id, error = %e, "failed to persist stream state");
      }
    }
  }

  /// Persist recording state to StateStore if configured
  pub async fn persist_recording(&self, info: &RecordingInfo) {
    if let Some(store) = &self.inner.state_store {
      if let Err(e) = store.save_recording(info).await {
        warn!(recording_id = %info.config.id, error = %e, "failed to persist recording state");
      }
    }
  }

  /// Delete stream from StateStore if configured
  pub async fn delete_stream_state(&self, stream_id: &str) {
    if let Some(store) = &self.inner.state_store {
      if let Err(e) = store.delete_stream(stream_id).await {
        warn!(stream_id = %stream_id, error = %e, "failed to delete stream state");
      }
    }
  }

  /// Delete recording from StateStore if configured
  pub async fn delete_recording_state(&self, recording_id: &str) {
    if let Some(store) = &self.inner.state_store {
      if let Err(e) = store.delete_recording(recording_id).await {
        warn!(recording_id = %recording_id, error = %e, "failed to delete recording state");
      }
    }
  }

  /// Bootstrap: restore state from StateStore on startup
  pub async fn bootstrap(&self) -> anyhow::Result<()> {
    if let Some(store) = &self.inner.state_store {
      // Restore streams for this node
      let streams = store.list_streams(Some(self.node_id())).await?;
      let mut streams_map = self.streams().write().await;
      for stream in streams {
        streams_map.insert(stream.config.id.clone(), stream);
      }
      drop(streams_map);

      // Restore recordings for this node
      let recordings = store.list_recordings(Some(self.node_id())).await?;
      let mut recordings_map = self.recordings().write().await;
      for recording in recordings {
        recordings_map.insert(recording.config.id.clone(), recording);
      }
      drop(recordings_map);

      tracing::info!(
        node_id = %self.node_id(),
        "state restored from StateStore"
      );
    }
    Ok(())
  }

  pub async fn start_lease_renewal(&self, stream_id: String, lease_id: String, ttl_secs: u64) {
    let token = CancellationToken::new();
    {
      let mut renewals = self.inner.renewals.write().await;
      if let Some(existing) = renewals.insert(stream_id.clone(), token.clone()) {
        existing.cancel();
      }
    }

    let state = self.clone();
    let interval_secs = ttl_secs / 2;
    let renew_interval = Duration::from_secs(std::cmp::max(interval_secs, 5));

    tokio::spawn(async move {
      let coordinator = state.coordinator();
      let worker = state.worker();
      let mut consecutive_failures = 0u32;
      const MAX_RETRIES: u32 = 3;

      loop {
        tokio::select! {
            _ = token.cancelled() => {
                break;
            }
            _ = tokio::time::sleep(renew_interval) => {
                // Check worker health first
                let worker_healthy = worker.health_check().await.unwrap_or(false);
                if !worker_healthy {
                    let info = {
                        let mut streams = state.streams().write().await;
                        if let Some(entry) = streams.get_mut(&stream_id) {
                            entry.state = StreamState::Error;
                            entry.last_error = Some("Worker health check failed".to_string());
                            Some(entry.clone())
                        } else {
                            None
                        }
                    };
                    if let Some(info) = info {
                        state.persist_stream(&info).await;
                    }
                    break;
                }

                // Attempt lease renewal with retry logic
                let req = LeaseRenewRequest {
                    lease_id: lease_id.clone(),
                    ttl_secs,
                };

                match coordinator.renew(&req).await {
                    Ok(_) => {
                        consecutive_failures = 0;
                        let info = {
                            let mut streams = state.streams().write().await;
                            if let Some(entry) = streams.get_mut(&stream_id) {
                                if entry.state == StreamState::Error {
                                    entry.state = StreamState::Running;
                                }
                                entry.last_error = None;
                                Some(entry.clone())
                            } else {
                                None
                            }
                        };
                        if let Some(info) = info {
                            state.persist_stream(&info).await;
                        }
                    }
                    Err(err) => {
                        consecutive_failures += 1;
                        if consecutive_failures >= MAX_RETRIES {
                            let info = {
                                let mut streams = state.streams().write().await;
                                if let Some(entry) = streams.get_mut(&stream_id) {
                                    entry.state = StreamState::Error;
                                    entry.last_error = Some(format!(
                                        "Lease renewal failed after {} retries: {}",
                                        MAX_RETRIES, err
                                    ));
                                    Some(entry.clone())
                                } else {
                                    None
                                }
                            };
                            if let Some(info) = info {
                                state.persist_stream(&info).await;
                            }
                            break;
                        } else {
                            // Log warning but continue trying
                            tracing::warn!(
                                stream_id = %stream_id,
                                attempt = consecutive_failures,
                                error = %err,
                                "Lease renewal failed, will retry"
                            );
                            // Exponential backoff before next attempt
                            let backoff = Duration::from_millis(100 * 2u64.pow(consecutive_failures - 1));
                            tokio::time::sleep(backoff).await;
                        }
                    }
                }
            }
        }
      }
      state.clear_renewal(&stream_id).await;
    });
  }

  pub async fn cancel_lease_renewal(&self, stream_id: &str) {
    if let Some(token) = self.inner.renewals.write().await.remove(stream_id) {
      token.cancel();
    }
  }

  async fn clear_renewal(&self, stream_id: &str) {
    self.inner.renewals.write().await.remove(stream_id);
  }
}
