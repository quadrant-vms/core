use crate::{config::GatewayConfig, coordinator::CoordinatorClient, worker::{RecorderClient, WorkerClient}};
use common::{
  leases::LeaseRenewRequest,
  recordings::RecordingInfo,
  streams::{StreamInfo, StreamState},
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
  inner: Arc<AppStateInner>,
}

struct AppStateInner {
  config: GatewayConfig,
  coordinator: Arc<dyn CoordinatorClient>,
  worker: Arc<dyn WorkerClient>,
  recorder: Arc<dyn RecorderClient>,
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
                    let mut streams = state.streams().write().await;
                    if let Some(entry) = streams.get_mut(&stream_id) {
                        entry.state = StreamState::Error;
                        entry.last_error = Some("Worker health check failed".to_string());
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
                        let mut streams = state.streams().write().await;
                        if let Some(entry) = streams.get_mut(&stream_id) {
                            if entry.state == StreamState::Error {
                                entry.state = StreamState::Running;
                            }
                            entry.last_error = None;
                        }
                    }
                    Err(err) => {
                        consecutive_failures += 1;
                        if consecutive_failures >= MAX_RETRIES {
                            let mut streams = state.streams().write().await;
                            if let Some(entry) = streams.get_mut(&stream_id) {
                                entry.state = StreamState::Error;
                                entry.last_error = Some(format!(
                                    "Lease renewal failed after {} retries: {}",
                                    MAX_RETRIES, err
                                ));
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
