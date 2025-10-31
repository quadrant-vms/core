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
