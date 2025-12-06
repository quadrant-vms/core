use crate::{cluster::ClusterStatus, error::ApiError, state::CoordinatorState};
use axum::{
  Json, Router,
  extract::{Query, State},
  routing::{get, post},
};
use common::leases::{
  LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest,
  LeaseReleaseResponse, LeaseRenewRequest, LeaseRenewResponse,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

pub fn router(state: CoordinatorState) -> Router {
  Router::new()
    .route("/healthz", get(healthz))
    .route("/readyz", get(readyz))
    .route("/v1/leases", get(list_leases))
    .route("/v1/leases/acquire", post(acquire_lease))
    .route("/v1/leases/renew", post(renew_lease))
    .route("/v1/leases/release", post(release_lease))
    .route("/cluster/status", get(cluster_status))
    .route("/cluster/vote", post(cluster_vote))
    .route("/cluster/heartbeat", post(cluster_heartbeat))
    .with_state(state)
}

async fn healthz() -> &'static str {
  "ok"
}

async fn readyz(State(state): State<CoordinatorState>) -> Result<&'static str, ApiError> {
  let store = state.store();
  match store.health_check().await {
    Ok(true) => Ok("ready"),
    Ok(false) => Err(ApiError::internal("lease store not ready")),
    Err(e) => Err(ApiError::internal(format!("health check failed: {}", e))),
  }
}

#[derive(Debug, Deserialize)]
struct ListLeasesQuery {
  kind: Option<String>,
}

async fn list_leases(
  State(state): State<CoordinatorState>,
  Query(query): Query<ListLeasesQuery>,
) -> Result<Json<Vec<LeaseRecord>>, ApiError> {
  let maybe_kind = if let Some(kind_str) = query.kind {
    if kind_str.is_empty() {
      None
    } else {
      Some(
        kind_str
          .parse::<LeaseKind>()
          .map_err(|_| ApiError::bad_request(format!("unknown lease kind '{}'", kind_str)))?,
      )
    }
  } else {
    None
  };

  let store = state.store();
  let records = store.list(maybe_kind).await?;
  Ok(Json(records))
}

/// Forward a request to the leader if this node is a follower
async fn forward_to_leader<T: Serialize, R: serde::de::DeserializeOwned>(
  state: &CoordinatorState,
  path: &str,
  payload: &T,
) -> Result<R, ApiError> {
  let cluster = state
    .cluster()
    .ok_or_else(|| ApiError::internal("clustering not enabled"))?;

  if cluster.is_leader().await {
    return Err(ApiError::internal("should not forward, this node is the leader"));
  }

  let leader_addr = cluster
    .leader_addr()
    .await
    .ok_or_else(|| ApiError::internal("no leader available"))?;

  let url = format!("http://{}{}", leader_addr, path);

  debug!(url = %url, "forwarding request to leader");

  let client = reqwest::Client::new();
  let response = client
    .post(&url)
    .json(payload)
    .send()
    .await
    .map_err(|e| ApiError::internal(format!("failed to forward request: {}", e)))?;

  if !response.status().is_success() {
    return Err(ApiError::internal(format!(
      "leader returned error: {}",
      response.status()
    )));
  }

  let result = response
    .json::<R>()
    .await
    .map_err(|e| ApiError::internal(format!("failed to parse leader response: {}", e)))?;

  Ok(result)
}

async fn acquire_lease(
  State(state): State<CoordinatorState>,
  Json(request): Json<LeaseAcquireRequest>,
) -> Result<Json<LeaseAcquireResponse>, ApiError> {
  // If clustering is enabled and we're a follower, forward to leader
  if let Some(cluster) = state.cluster() {
    if !cluster.is_leader().await {
      let resp = forward_to_leader(&state, "/v1/leases/acquire", &request).await?;
      return Ok(Json(resp));
    }
  }

  let store = state.store();
  let resp = store.acquire(request).await?;
  Ok(Json(resp))
}

async fn renew_lease(
  State(state): State<CoordinatorState>,
  Json(request): Json<LeaseRenewRequest>,
) -> Result<Json<LeaseRenewResponse>, ApiError> {
  // If clustering is enabled and we're a follower, forward to leader
  if let Some(cluster) = state.cluster() {
    if !cluster.is_leader().await {
      let resp = forward_to_leader(&state, "/v1/leases/renew", &request).await?;
      return Ok(Json(resp));
    }
  }

  let store = state.store();
  let resp = store.renew(request).await?;
  Ok(Json(resp))
}

async fn release_lease(
  State(state): State<CoordinatorState>,
  Json(request): Json<LeaseReleaseRequest>,
) -> Result<Json<LeaseReleaseResponse>, ApiError> {
  // If clustering is enabled and we're a follower, forward to leader
  if let Some(cluster) = state.cluster() {
    if !cluster.is_leader().await {
      let resp = forward_to_leader(&state, "/v1/leases/release", &request).await?;
      return Ok(Json(resp));
    }
  }

  let store = state.store();
  let resp = store.release(request).await?;
  Ok(Json(resp))
}

async fn cluster_status(
  State(state): State<CoordinatorState>,
) -> Result<Json<ClusterStatus>, ApiError> {
  let cluster = state
    .cluster()
    .ok_or_else(|| ApiError::bad_request("clustering not enabled"))?;
  let status = cluster.status().await;
  Ok(Json(status))
}

#[derive(Debug, Deserialize)]
struct VoteRequest {
  candidate_id: String,
  term: u64,
}

#[derive(Debug, Serialize)]
struct VoteResponse {
  vote_granted: bool,
}

async fn cluster_vote(
  State(state): State<CoordinatorState>,
  Json(request): Json<VoteRequest>,
) -> Result<Json<VoteResponse>, ApiError> {
  let cluster = state
    .cluster()
    .ok_or_else(|| ApiError::bad_request("clustering not enabled"))?;
  let vote_granted = cluster
    .handle_vote_request(request.candidate_id, request.term)
    .await;
  Ok(Json(VoteResponse { vote_granted }))
}

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
  leader_id: String,
  leader_addr: String,
  term: u64,
}

async fn cluster_heartbeat(
  State(state): State<CoordinatorState>,
  Json(request): Json<HeartbeatRequest>,
) -> Result<&'static str, ApiError> {
  let cluster = state
    .cluster()
    .ok_or_else(|| ApiError::bad_request("clustering not enabled"))?;
  cluster
    .handle_heartbeat(request.leader_id, request.leader_addr, request.term)
    .await;
  Ok("ok")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    config::{CoordinatorConfig, LeaseStoreType},
    state::CoordinatorState,
    store::MemoryLeaseStore,
  };
  use axum::{
    body::Body,
    http::{Request, StatusCode},
  };
  use serde_json::json;
  use std::{net::SocketAddr, sync::Arc};
  use tower::ServiceExt;

  fn test_state() -> CoordinatorState {
    let config = CoordinatorConfig {
      bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
      default_ttl_secs: 10,
      max_ttl_secs: 60,
      store_type: LeaseStoreType::Memory,
      database_url: None,
      cluster_enabled: false,
      node_id: None,
      peer_addrs: vec![],
      election_timeout_ms: 5000,
      heartbeat_interval_ms: 1000,
    };
    let store = Arc::new(MemoryLeaseStore::new(10, 60));
    CoordinatorState::new(config, store)
  }

  #[tokio::test]
  async fn acquire_then_list() {
    let app = router(test_state());
    let acquire_req = Request::builder()
      .method("POST")
      .uri("/v1/leases/acquire")
      .header("content-type", "application/json")
      .body(Body::from(
        json!({
            "resource_id": "cam1",
            "holder_id": "node-a",
            "kind": "stream",
            "ttl_secs": 15
        })
        .to_string(),
      ))
      .unwrap();

    let resp = app.clone().oneshot(acquire_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let list_req = Request::builder()
      .method("GET")
      .uri("/v1/leases")
      .body(Body::empty())
      .unwrap();
    let resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let leases: Vec<LeaseRecord> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(leases.len(), 1);
    assert_eq!(leases[0].resource_id, "cam1");
  }

  #[tokio::test]
  async fn release_clears_lease() {
    let app = router(test_state());
    let acquire_body = json!({
        "resource_id": "cam1",
        "holder_id": "node-a",
        "kind": "stream"
    })
    .to_string();
    let resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/leases/acquire")
          .header("content-type", "application/json")
          .body(Body::from(acquire_body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let acquire_resp: LeaseAcquireResponse = serde_json::from_slice(&bytes).unwrap();
    let lease_id = acquire_resp.record.unwrap().lease_id;

    let release_body = json!({
        "lease_id": lease_id
    })
    .to_string();
    let release_resp = app
      .clone()
      .oneshot(
        Request::builder()
          .method("POST")
          .uri("/v1/leases/release")
          .header("content-type", "application/json")
          .body(Body::from(release_body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(release_resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(release_resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let release: LeaseReleaseResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(release.released);

    let list_resp = app
      .oneshot(
        Request::builder()
          .method("GET")
          .uri("/v1/leases")
          .body(Body::empty())
          .unwrap(),
      )
      .await
      .unwrap();
    let bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
      .await
      .unwrap();
    let leases: Vec<LeaseRecord> = serde_json::from_slice(&bytes).unwrap();
    assert!(leases.is_empty());
  }
}
