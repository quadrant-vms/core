use crate::{error::ApiError, state::AppState};
use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Json, Router,
};
use common::{
    leases::{LeaseAcquireRequest, LeaseKind, LeaseReleaseRequest},
    streams::{
        StreamInfo, StreamStartRequest, StreamStartResponse, StreamState, StreamStopResponse,
    },
};
use tracing::info;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/streams", get(list_streams).post(start_stream))
        .route("/v1/streams/:id", delete(stop_stream))
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
        return Err(ApiError::not_found(format!("stream '{}' not found", stream_id)));
    };

    if let Some(lease_id) = info.lease_id.clone() {
        let coordinator = state.coordinator();
        let release_req = LeaseReleaseRequest { lease_id: lease_id.clone() };
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
        // No active lease tracked; just remove local state.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::GatewayConfig, coordinator::CoordinatorClient};
    use anyhow::Result;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use common::{
        leases::{LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest, LeaseReleaseResponse},
        streams::{StreamConfig, StreamInfo, StreamState},
    };
    use reqwest::Url;
    use serde_json::json;
    use std::{net::SocketAddr, sync::Arc};
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    struct StubCoordinator {
        acquire_responses: Mutex<Vec<LeaseAcquireResponse>>,
        release_responses: Mutex<Vec<LeaseReleaseResponse>>,
        acquire_calls: Mutex<Vec<LeaseAcquireRequest>>,
        release_calls: Mutex<Vec<LeaseReleaseRequest>>,
    }

    impl StubCoordinator {
        fn with_responses(acquire: Vec<LeaseAcquireResponse>, release: Vec<LeaseReleaseResponse>) -> Arc<Self> {
            Arc::new(Self {
                acquire_responses: Mutex::new(acquire),
                release_responses: Mutex::new(release),
                acquire_calls: Mutex::new(vec![]),
                release_calls: Mutex::new(vec![]),
            })
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

        async fn release(&self, request: &LeaseReleaseRequest) -> Result<LeaseReleaseResponse> {
            self.release_calls.lock().await.push(request.clone());
            let mut responses = self.release_responses.lock().await;
            let resp = responses
                .pop()
                .unwrap_or(LeaseReleaseResponse { released: true });
            Ok(resp)
        }
    }

    fn base_config() -> GatewayConfig {
        GatewayConfig {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            coordinator_base_url: Url::parse("http://127.0.0.1:8082").unwrap(),
            node_id: "test-node".into(),
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
        let state = AppState::new(base_config(), coordinator.clone());
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
        let state = AppState::new(base_config(), coordinator);
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
        let state = AppState::new(base_config(), coordinator.clone());
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
    }
}
