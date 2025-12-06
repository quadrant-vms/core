use crate::{error::ApiError, state::AppState};
use axum::{
  Json, Router,
  extract::{Path, State},
  routing::{delete, get},
};
use common::{
  leases::{LeaseAcquireRequest, LeaseKind, LeaseReleaseRequest},
  recordings::{RecordingInfo, RecordingStartRequest, RecordingStartResponse, RecordingState, RecordingStopRequest, RecordingStopResponse},
  streams::{StreamInfo, StreamStartRequest, StreamStartResponse, StreamState, StreamStopResponse},
};
use tracing::info;

pub fn router(state: AppState) -> Router {
  Router::new()
    .route("/healthz", get(healthz))
    .route("/v1/streams", get(list_streams).post(start_stream))
    .route("/v1/streams/:id", delete(stop_stream))
    .route("/v1/recordings", get(list_recordings).post(start_recording))
    .route("/v1/recordings/:id", delete(stop_recording))
    .with_state(state)
}

async fn healthz() -> &'static str {
  "ok"
}

async fn list_streams(State(state): State<AppState>) -> Result<Json<Vec<StreamInfo>>, ApiError> {
  let streams = state.streams().read().await;
  let list = streams.values().cloned().collect();
  Ok(Json(list))
}

async fn start_stream(
  State(state): State<AppState>,
  Json(payload): Json<StreamStartRequest>,
) -> Result<Json<StreamStartResponse>, ApiError> {
  let config = payload.config;
  if config.id.trim().is_empty() {
    return Err(ApiError::bad_request("stream id required"));
  }
  if config.uri.trim().is_empty() {
    return Err(ApiError::bad_request("stream uri required"));
  }

  {
    let streams = state.streams().read().await;
    if let Some(existing) = streams.get(&config.id) {
      if existing.state.is_active() {
        return Ok(Json(StreamStartResponse {
          accepted: false,
          lease_id: existing.lease_id.clone(),
          message: Some("stream already active".into()),
        }));
      }
    }
  }

  let ttl = payload.lease_ttl_secs.unwrap_or(30).max(5);
  let lease_req = LeaseAcquireRequest {
    resource_id: config.id.clone(),
    holder_id: state.node_id().to_string(),
    kind: LeaseKind::Stream,
    ttl_secs: ttl,
  };

  let coordinator = state.coordinator();
  let lease_resp = coordinator.acquire(&lease_req).await?;

  if !lease_resp.granted {
    return Ok(Json(StreamStartResponse {
      accepted: false,
      lease_id: lease_resp.record.map(|r| r.lease_id),
      message: Some("resource already leased".into()),
    }));
  }

  let record = lease_resp
    .record
    .ok_or_else(|| ApiError::internal("coordinator granted lease without record"))?;

  let stream_info = StreamInfo {
    config: config.clone(),
    state: StreamState::Starting,
    lease_id: Some(record.lease_id.clone()),
    last_error: None,
  };

  {
    let mut streams = state.streams().write().await;
    streams.insert(config.id.clone(), stream_info);
  }

  let worker = state.worker();
  if let Err(err) = worker.start_stream(&config).await {
    {
      let mut streams = state.streams().write().await;
      if let Some(entry) = streams.get_mut(&config.id) {
        entry.state = StreamState::Error;
        entry.last_error = Some(err.to_string());
      }
    }
    let coordinator = state.coordinator();
    let _ = coordinator
      .release(&LeaseReleaseRequest {
        lease_id: record.lease_id.clone(),
      })
      .await;
    return Err(ApiError::internal(format!("worker start failed: {err}")));
  }

  {
    let mut streams = state.streams().write().await;
    if let Some(entry) = streams.get_mut(&config.id) {
      entry.state = StreamState::Running;
      entry.last_error = None;
    }
  }

  state
    .start_lease_renewal(
      config.id.clone(),
      record.lease_id.clone(),
      lease_req.ttl_secs,
    )
    .await;

  info!(stream_id = %config.id, lease = %record.lease_id, "stream start accepted");

  Ok(Json(StreamStartResponse {
    accepted: true,
    lease_id: Some(record.lease_id),
    message: None,
  }))
}

async fn stop_stream(
  State(state): State<AppState>,
  Path(stream_id): Path<String>,
) -> Result<Json<StreamStopResponse>, ApiError> {
  let existing = {
    let streams = state.streams().read().await;
    streams.get(&stream_id).cloned()
  };

  let Some(info) = existing else {
    return Err(ApiError::not_found(format!(
      "stream '{}' not found",
      stream_id
    )));
  };

  state.cancel_lease_renewal(&stream_id).await;

  {
    let mut streams = state.streams().write().await;
    if let Some(entry) = streams.get_mut(&stream_id) {
      entry.state = StreamState::Stopping;
      entry.last_error = None;
    }
  }

  let worker = state.worker();
  if let Err(err) = worker.stop_stream(&stream_id).await {
    let mut streams = state.streams().write().await;
    if let Some(entry) = streams.get_mut(&stream_id) {
      entry.state = StreamState::Error;
      entry.last_error = Some(err.to_string());
    }
    return Err(ApiError::internal(format!("worker stop failed: {err}")));
  }

  if let Some(lease_id) = info.lease_id.clone() {
    let coordinator = state.coordinator();
    let release_req = LeaseReleaseRequest {
      lease_id: lease_id.clone(),
    };
    let release_resp = coordinator.release(&release_req).await?;

    {
      let mut streams = state.streams().write().await;
      streams.remove(&stream_id);
    }

    let message = if release_resp.released {
      None
    } else {
      Some("lease already released or expired".into())
    };

    info!(stream_id = %stream_id, lease = %lease_id, released = release_resp.released, "stream stop requested");

    Ok(Json(StreamStopResponse {
      stopped: true,
      message,
    }))
  } else {
    state.cancel_lease_renewal(&stream_id).await;

    {
      let mut streams = state.streams().write().await;
      streams.remove(&stream_id);
    }

    Ok(Json(StreamStopResponse {
      stopped: true,
      message: Some("stream had no active lease; removed local state".into()),
    }))
  }
}

async fn list_recordings(State(state): State<AppState>) -> Result<Json<Vec<RecordingInfo>>, ApiError> {
  let recordings = state.recordings().read().await;
  let list = recordings.values().cloned().collect();
  Ok(Json(list))
}

async fn start_recording(
  State(state): State<AppState>,
  Json(payload): Json<RecordingStartRequest>,
) -> Result<Json<RecordingStartResponse>, ApiError> {
  if payload.config.id.trim().is_empty() {
    return Err(ApiError::bad_request("recording id required"));
  }
  if payload.config.source_stream_id.is_none() && payload.config.source_uri.is_none() {
    return Err(ApiError::bad_request("source_stream_id or source_uri required"));
  }

  {
    let recordings = state.recordings().read().await;
    if let Some(existing) = recordings.get(&payload.config.id) {
      if existing.state.is_active() {
        return Ok(Json(RecordingStartResponse {
          accepted: false,
          lease_id: existing.lease_id.clone(),
          message: Some("recording already active".into()),
        }));
      }
    }
  }

  let ttl = payload.lease_ttl_secs.unwrap_or(30).max(5);
  let lease_req = LeaseAcquireRequest {
    resource_id: payload.config.id.clone(),
    holder_id: state.node_id().to_string(),
    kind: LeaseKind::Recorder,
    ttl_secs: ttl,
  };

  let coordinator = state.coordinator();
  let lease_resp = coordinator.acquire(&lease_req).await?;

  if !lease_resp.granted {
    return Ok(Json(RecordingStartResponse {
      accepted: false,
      lease_id: lease_resp.record.map(|r| r.lease_id),
      message: Some("resource already leased".into()),
    }));
  }

  let record = lease_resp
    .record
    .ok_or_else(|| ApiError::internal("coordinator granted lease without record"))?;

  let recording_info = RecordingInfo {
    config: payload.config.clone(),
    state: RecordingState::Starting,
    lease_id: Some(record.lease_id.clone()),
    storage_path: None,
    last_error: None,
    started_at: None,
    stopped_at: None,
  };

  {
    let mut recordings = state.recordings().write().await;
    recordings.insert(payload.config.id.clone(), recording_info);
  }

  let recorder = state.recorder();
  let recorder_resp = recorder.start_recording(&payload).await;

  match recorder_resp {
    Ok(resp) if !resp.accepted => {
      {
        let mut recordings = state.recordings().write().await;
        if let Some(entry) = recordings.get_mut(&payload.config.id) {
          entry.state = RecordingState::Error;
          entry.last_error = resp.message.clone();
        }
      }
      let coordinator = state.coordinator();
      let _ = coordinator
        .release(&LeaseReleaseRequest {
          lease_id: record.lease_id.clone(),
        })
        .await;
      return Ok(Json(RecordingStartResponse {
        accepted: false,
        lease_id: Some(record.lease_id),
        message: resp.message,
      }));
    }
    Err(err) => {
      {
        let mut recordings = state.recordings().write().await;
        if let Some(entry) = recordings.get_mut(&payload.config.id) {
          entry.state = RecordingState::Error;
          entry.last_error = Some(err.to_string());
        }
      }
      let coordinator = state.coordinator();
      let _ = coordinator
        .release(&LeaseReleaseRequest {
          lease_id: record.lease_id.clone(),
        })
        .await;
      return Err(ApiError::internal(format!("recorder start failed: {err}")));
    }
    Ok(_) => {}
  }

  {
    let mut recordings = state.recordings().write().await;
    if let Some(entry) = recordings.get_mut(&payload.config.id) {
      entry.state = RecordingState::Recording;
      entry.last_error = None;
    }
  }

  state
    .start_lease_renewal(
      payload.config.id.clone(),
      record.lease_id.clone(),
      lease_req.ttl_secs,
    )
    .await;

  info!(recording_id = %payload.config.id, lease = %record.lease_id, "recording start accepted");

  Ok(Json(RecordingStartResponse {
    accepted: true,
    lease_id: Some(record.lease_id),
    message: None,
  }))
}

async fn stop_recording(
  State(state): State<AppState>,
  Path(recording_id): Path<String>,
) -> Result<Json<RecordingStopResponse>, ApiError> {
  let existing = {
    let recordings = state.recordings().read().await;
    recordings.get(&recording_id).cloned()
  };

  let Some(info) = existing else {
    return Err(ApiError::not_found(format!(
      "recording '{}' not found",
      recording_id
    )));
  };

  state.cancel_lease_renewal(&recording_id).await;

  {
    let mut recordings = state.recordings().write().await;
    if let Some(entry) = recordings.get_mut(&recording_id) {
      entry.state = RecordingState::Stopping;
      entry.last_error = None;
    }
  }

  let recorder = state.recorder();
  let stop_req = RecordingStopRequest {
    id: recording_id.clone(),
  };
  if let Err(err) = recorder.stop_recording(&stop_req).await {
    let mut recordings = state.recordings().write().await;
    if let Some(entry) = recordings.get_mut(&recording_id) {
      entry.state = RecordingState::Error;
      entry.last_error = Some(err.to_string());
    }
    return Err(ApiError::internal(format!("recorder stop failed: {err}")));
  }

  if let Some(lease_id) = info.lease_id.clone() {
    let coordinator = state.coordinator();
    let release_req = LeaseReleaseRequest {
      lease_id: lease_id.clone(),
    };
    let release_resp = coordinator.release(&release_req).await?;

    {
      let mut recordings = state.recordings().write().await;
      recordings.remove(&recording_id);
    }

    let message = if release_resp.released {
      None
    } else {
      Some("lease already released or expired".into())
    };

    info!(recording_id = %recording_id, lease = %lease_id, released = release_resp.released, "recording stop requested");

    Ok(Json(RecordingStopResponse {
      stopped: true,
      message,
    }))
  } else {
    state.cancel_lease_renewal(&recording_id).await;

    {
      let mut recordings = state.recordings().write().await;
      recordings.remove(&recording_id);
    }

    Ok(Json(RecordingStopResponse {
      stopped: true,
      message: Some("recording had no active lease; removed local state".into()),
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{config::GatewayConfig, coordinator::CoordinatorClient, worker::{RecorderClient, WorkerClient}};
  use anyhow::{Result, anyhow};
  use axum::{
    body::Body,
    http::{Request, StatusCode},
  };
  use common::{
    leases::{
      LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest,
      LeaseReleaseResponse, LeaseRenewRequest, LeaseRenewResponse,
    },
    streams::{StreamConfig, StreamInfo, StreamState},
  };
  use reqwest::Url;
  use serde_json::json;
  use std::{net::SocketAddr, sync::Arc, time::Duration};
  use tokio::sync::Mutex;
  use tower::ServiceExt;

  struct StubCoordinator {
    acquire_responses: Mutex<Vec<LeaseAcquireResponse>>,
    release_responses: Mutex<Vec<LeaseReleaseResponse>>,
    renew_responses: Mutex<Vec<Result<LeaseRenewResponse>>>,
    acquire_calls: Mutex<Vec<LeaseAcquireRequest>>,
    release_calls: Mutex<Vec<LeaseReleaseRequest>>,
    renew_calls: Mutex<Vec<LeaseRenewRequest>>,
  }

  impl StubCoordinator {
    fn with_responses(
      acquire: Vec<LeaseAcquireResponse>,
      release: Vec<LeaseReleaseResponse>,
    ) -> Arc<Self> {
      Arc::new(Self {
        acquire_responses: Mutex::new(acquire),
        release_responses: Mutex::new(release),
        renew_responses: Mutex::new(vec![]),
        acquire_calls: Mutex::new(vec![]),
        release_calls: Mutex::new(vec![]),
        renew_calls: Mutex::new(vec![]),
      })
    }

    async fn push_renew_response(
      self: &Arc<Self>,
      response: Result<LeaseRenewResponse, &'static str>,
    ) {
      let mut lock = self.renew_responses.lock().await;
      lock.push(match response {
        Ok(ok) => Ok(ok),
        Err(msg) => Err(anyhow!(msg)),
      });
    }
  }

  #[async_trait::async_trait]
  impl CoordinatorClient for StubCoordinator {
    async fn acquire(&self, request: &LeaseAcquireRequest) -> Result<LeaseAcquireResponse> {
      self.acquire_calls.lock().await.push(request.clone());
      let mut responses = self.acquire_responses.lock().await;
      let resp = responses
        .pop()
        .expect("no acquire response configured (pop from end)");
      Ok(resp)
    }

    async fn renew(&self, request: &LeaseRenewRequest) -> Result<LeaseRenewResponse> {
      self.renew_calls.lock().await.push(request.clone());
      let mut responses = self.renew_responses.lock().await;
      match responses.pop() {
        Some(Ok(resp)) => Ok(resp),
        Some(Err(err)) => Err(err),
        None => Ok(LeaseRenewResponse {
          renewed: true,
          record: None,
        }),
      }
    }

    async fn release(&self, request: &LeaseReleaseRequest) -> Result<LeaseReleaseResponse> {
      self.release_calls.lock().await.push(request.clone());
      let mut responses = self.release_responses.lock().await;
      let resp = responses
        .pop()
        .unwrap_or(LeaseReleaseResponse { released: true });
      Ok(resp)
    }
  }

  #[derive(Default)]
  struct StubWorker {
    start_calls: Mutex<Vec<StreamConfig>>,
    stop_calls: Mutex<Vec<String>>,
    fail_start: Mutex<bool>,
    fail_stop: Mutex<bool>,
  }

  impl StubWorker {
    fn new() -> Self {
      Self::default()
    }
  }

  #[async_trait::async_trait]
  impl WorkerClient for StubWorker {
    async fn start_stream(&self, config: &StreamConfig) -> Result<()> {
      self.start_calls.lock().await.push(config.clone());
      if *self.fail_start.lock().await {
        anyhow::bail!("worker start failed");
      }
      Ok(())
    }

    async fn stop_stream(&self, stream_id: &str) -> Result<()> {
      self.stop_calls.lock().await.push(stream_id.to_string());
      if *self.fail_stop.lock().await {
        anyhow::bail!("worker stop failed");
      }
      Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
      Ok(true)
    }
  }

  #[derive(Default)]
  struct StubRecorder;

  impl StubRecorder {
    fn new() -> Self {
      Self::default()
    }
  }

  #[async_trait::async_trait]
  impl RecorderClient for StubRecorder {
    async fn start_recording(&self, _request: &RecordingStartRequest) -> Result<RecordingStartResponse> {
      Ok(RecordingStartResponse {
        accepted: true,
        lease_id: None,
        message: None,
      })
    }

    async fn stop_recording(&self, _request: &RecordingStopRequest) -> Result<RecordingStopResponse> {
      Ok(RecordingStopResponse {
        stopped: true,
        message: None,
      })
    }

    async fn health_check(&self) -> Result<bool> {
      Ok(true)
    }
  }

  fn base_config() -> GatewayConfig {
    GatewayConfig {
      bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
      coordinator_base_url: Url::parse("http://127.0.0.1:8082").unwrap(),
      node_id: "test-node".into(),
      worker_base_url: Url::parse("http://127.0.0.1:8080").unwrap(),
      recorder_base_url: Url::parse("http://127.0.0.1:8083").unwrap(),
    }
  }

  #[tokio::test]
  async fn start_stream_accepts_and_records_state() {
    let lease_record = LeaseRecord {
      lease_id: "lease-123".into(),
      resource_id: "stream-1".into(),
      holder_id: "test-node".into(),
      kind: LeaseKind::Stream,
      expires_at_epoch_secs: 999999,
      version: 1,
    };
    let coordinator = StubCoordinator::with_responses(
      vec![LeaseAcquireResponse {
        granted: true,
        record: Some(lease_record.clone()),
      }],
      vec![LeaseReleaseResponse { released: true }],
    );
    let worker = Arc::new(StubWorker::new());
    let worker_client: Arc<dyn WorkerClient> = worker.clone();
    let recorder: Arc<dyn RecorderClient> = Arc::new(StubRecorder::new());
    let state = AppState::new(base_config(), coordinator.clone(), worker_client, recorder);
    let app = router(state.clone());

    let start_body = json!({
        "config": {
            "id": "stream-1",
            "camera_id": "cam-1",
            "uri": "rtsp://example",
            "codec": "h264",
            "container": "ts"
        }
    })
    .to_string();

    let resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/streams")
          .header("content-type", "application/json")
          .body(Body::from(start_body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let start_resp: StreamStartResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(start_resp.accepted);
    assert_eq!(start_resp.lease_id.as_deref(), Some("lease-123"));

    // ensure state shows the stream
    let list_resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("GET")
          .uri("/v1/streams")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    let bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let list: Vec<StreamInfo> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].config.id, "stream-1");
    assert!(list[0].state.is_active());

    let start_calls = worker.start_calls.lock().await.clone();
    assert_eq!(start_calls.len(), 1);
    assert_eq!(start_calls[0].id, "stream-1");

    state.cancel_lease_renewal("stream-1").await;

    // verify acquire called with expected resource id
    let calls = coordinator.acquire_calls.lock().await.clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].resource_id, "stream-1");
  }

  #[tokio::test]
  async fn start_stream_missing_uri_rejected() {
    let coordinator = StubCoordinator::with_responses(
      vec![LeaseAcquireResponse {
        granted: true,
        record: None,
      }],
      vec![],
    );
    let worker = Arc::new(StubWorker::new());
    let worker_client: Arc<dyn WorkerClient> = worker.clone();
    let recorder: Arc<dyn RecorderClient> = Arc::new(StubRecorder::new());
    let state = AppState::new(base_config(), coordinator, worker_client, recorder);
    let app = router(state);
    let body = json!({
        "config": {
            "id": "stream-1",
            "uri": ""
        }
    })
    .to_string();
    let resp = app
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/streams")
          .header("content-type", "application/json")
          .body(Body::from(body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(worker.start_calls.lock().await.is_empty());
  }

  #[tokio::test]
  async fn stop_stream_releases_and_removes() {
    let lease_record = LeaseRecord {
      lease_id: "lease-abc".into(),
      resource_id: "stream-1".into(),
      holder_id: "test-node".into(),
      kind: LeaseKind::Stream,
      expires_at_epoch_secs: 999999,
      version: 1,
    };
    let coordinator = StubCoordinator::with_responses(
      vec![LeaseAcquireResponse {
        granted: true,
        record: Some(lease_record.clone()),
      }],
      vec![LeaseReleaseResponse { released: true }],
    );
    let worker = Arc::new(StubWorker::new());
    let worker_client: Arc<dyn WorkerClient> = worker.clone();
    let recorder: Arc<dyn RecorderClient> = Arc::new(StubRecorder::new());
    let state = AppState::new(base_config(), coordinator.clone(), worker_client, recorder);
    {
      // seed state directly with a running stream
      let mut streams = state.streams().write().await;
      streams.insert(
        "stream-1".into(),
        StreamInfo {
          config: StreamConfig {
            id: "stream-1".into(),
            camera_id: Some("cam-1".into()),
            uri: "rtsp://example".into(),
            codec: Some("h264".into()),
            container: Some("ts".into()),
          },
          state: StreamState::Running,
          lease_id: Some("lease-abc".into()),
          last_error: None,
        },
      );
    }

    let app = router(state.clone());
    let resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("DELETE")
          .uri("/v1/streams/stream-1")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let stop_resp: StreamStopResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(stop_resp.stopped);

    let list_resp = app
      .oneshot(
        Request::builder()
          .method("GET")
          .uri("/v1/streams")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    let bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let list: Vec<StreamInfo> = serde_json::from_slice(&bytes).unwrap();
    assert!(list.is_empty());

    let releases = coordinator.release_calls.lock().await.clone();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].lease_id, "lease-abc");

    let stop_calls = worker.stop_calls.lock().await.clone();
    assert_eq!(stop_calls, vec!["stream-1".to_string()]);

    state.cancel_lease_renewal("stream-1").await;
  }

  #[tokio::test(start_paused = true)]
  async fn start_stream_worker_failure_rolls_back_lease() {
    let lease_record = LeaseRecord {
      lease_id: "lease-xyz".into(),
      resource_id: "stream-err".into(),
      holder_id: "test-node".into(),
      kind: LeaseKind::Stream,
      expires_at_epoch_secs: 999999,
      version: 1,
    };
    let coordinator = StubCoordinator::with_responses(
      vec![LeaseAcquireResponse {
        granted: true,
        record: Some(lease_record),
      }],
      vec![LeaseReleaseResponse { released: true }],
    );
    let worker = Arc::new(StubWorker::new());
    *worker.fail_start.lock().await = true;
    let recorder: Arc<dyn RecorderClient> = Arc::new(StubRecorder::new());
    let state = AppState::new(
      base_config(),
      coordinator.clone(),
      worker.clone() as Arc<dyn WorkerClient>,
      recorder,
    );
    let app = router(state.clone());

    let start_body = json!({
        "config": {
            "id": "stream-err",
            "uri": "rtsp://example",
            "codec": "h265",
            "container": "fmp4"
        }
    })
    .to_string();

    let resp = app
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/streams")
          .header("content-type", "application/json")
          .body(Body::from(start_body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let streams = state.streams().read().await;
    let entry = streams.get("stream-err").expect("stream exists");
    assert_eq!(entry.state, StreamState::Error);
    assert!(
      entry
        .last_error
        .as_ref()
        .unwrap()
        .contains("worker start failed")
    );
    drop(streams);

    let release_calls = coordinator.release_calls.lock().await.clone();
    assert_eq!(release_calls.len(), 1);
    assert_eq!(release_calls[0].lease_id, "lease-xyz");
  }

  #[tokio::test(start_paused = true)]
  async fn lease_renew_failure_sets_error_state() {
    let lease_record = LeaseRecord {
      lease_id: "lease-renew".into(),
      resource_id: "stream-renew".into(),
      holder_id: "test-node".into(),
      kind: LeaseKind::Stream,
      expires_at_epoch_secs: 999999,
      version: 1,
    };
    let coordinator = StubCoordinator::with_responses(
      vec![LeaseAcquireResponse {
        granted: true,
        record: Some(lease_record.clone()),
      }],
      vec![LeaseReleaseResponse { released: true }],
    );
    coordinator.push_renew_response(Err("renew failed")).await;
    let worker = Arc::new(StubWorker::new());
    let recorder: Arc<dyn RecorderClient> = Arc::new(StubRecorder::new());
    let state = AppState::new(
      base_config(),
      coordinator.clone(),
      worker.clone() as Arc<dyn WorkerClient>,
      recorder,
    );
    let app = router(state.clone());

    let start_body = json!({
        "config": {
            "id": "stream-renew",
            "uri": "rtsp://example",
            "codec": "h264",
            "container": "ts"
        },
        "lease_ttl_secs": 10
    })
    .to_string();

    let resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/streams")
          .header("content-type", "application/json")
          .body(Body::from(start_body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let mut observed_error = false;
    for _ in 0..3 {
      tokio::time::advance(Duration::from_secs(6)).await;
      tokio::task::yield_now().await;
      let streams = state.streams().read().await;
      if let Some(entry) = streams.get("stream-renew") {
        if entry.state == StreamState::Error {
          assert!(entry.last_error.as_ref().unwrap().contains("renew failed"));
          observed_error = true;
          drop(streams);
          break;
        }
      }
      drop(streams);
    }
    assert!(observed_error, "expected stream to enter error state after renew failure");

    let renew_calls = coordinator.renew_calls.lock().await.clone();
    assert!(!renew_calls.is_empty(), "expected renew to be called");

    state.cancel_lease_renewal("stream-renew").await;
  }
}
